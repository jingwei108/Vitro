//go:build windows

// shadow_verify —— C 影子验证主驱动（防线 1，唯一硬门禁）的 Go 版本（D5 最后一站）。
//
// 与 Python 版（native/tests/shadow_verification/shadow_verify.py）的口径逐项对齐：
//   - 用例来源：baseline / gap / template / knr / leetcode 五目录 sorted(glob *.c)，
//     @category 提取（ASCII 限定防中文标点吞并）、剔除 "// @" 注释行、同名 .in 注入 stdin；
//   - Clang 侧：文件用例在原目录编译（#include 解析）、-Wno-implicit-function-declaration、
//     编译 30s / 运行 5s 超时、worker 隔离运行目录 + VFS 预设文件（test.txt / numbers.txt）、
//     结果中的运行目录路径归一化为 <rundir>；
//   - Cide 侧：capi 直调（compile_unit+compile_all / set_input_mode+set_input）+
//     E-P1-5 结构化输出通道（禁文本清洗）+ ABI 缺失 / 产物版本串不含 HEAD 时 fail fast；
//   - 判定（主驱动口径，与 C++ 版不同）：都编译失败=match；clang OK+cide 编译失败=compile_gap；
//     都编译 OK 且都运行失败=match；仅 cide 运行失败=known_issue（KNOWN_FAILURE_CASES）/runtime_gap；
//     stdout 不一致=known_issue（category 含 "bug" 或 KNOWN_FAILURE_CASES）/output_gap；
//     仅 clang 编译失败=cide_better；
//   - 门禁：非预期差异（compile_gap / runtime_gap / output_gap）>0 → exit 1；
//     Clang 预检失败 / 产物不新鲜 → exit 2；DLL 缺失 → exit 1；
//   - 报告三件套：Markdown（带时间戳 + latest）、JSON（带时间戳 + latest）、
//     kr_leetcode_report.json；--limit 调试样本不覆盖任何 latest。
//
// 与 Python 版的差异（有意的，均记录在案）：
//   - 并发形态：Clang 侧并发（--jobs，0=自动 min(CPU,8)），Cide 侧互斥锁串行——
//     DLL 并发调用 → 堆损坏是实测结论（AGENTS.md D5 进度），单进程内不做线程安全假设；
//     Python 版靠多进程绕开该限制，Go 单进程 + 互斥是等价的安全形态；
//   - Clang 结果缓存为 Go 自有 schema（"go1"）：key 用 Go 结构体序列化 + sha256，
//     与 Python key（json.dumps sort_keys）天然不同名、互不干扰、可共存于同一 .clang_cache/；
//   - stdin 口径修正：Python text=True 在 Windows 上把 stdin 的 \n 翻译成 \r\n 再喂 Clang
//     （而 Cide 收 \n，"同一份字节"名不副实）；Go 两侧喂同一份 .in 内容
//     （经 universal-newlines 归一为 \n，与 Python Path.read_text 的读取语义一致）；
//   - Clang 编译失败重试 3 次（C++ 版第一站实证 CI runner 瞬时故障）；
//   - 不再内置硬编码 SHADOW_CASES fallback：目录加载失败/为空 → fail loud（exit 2）。
//     Python 的 fallback 是文件加载机制落地前的历史遗留，共 ~300 行永不执行的死重；
//   - 启动自检（selfCheck，J9）：对 analyzeDiff / classifyCompileError 注入必然违反的输入，
//     断言必须变红，否则退出码 2 拒绝跑（判定函数坏了的"全绿"比没有门禁更坏）。
//
// 用法：go run ./scripts/shadow_verify [--jobs N] [--refresh-clang] [--rebuild]
//
//	[--limit N] [--report path.md] [--json path.json]
package main

import (
	"cide/scripts/internal/capi"

	"bytes"
	"context"
	"crypto/sha256"
	"encoding/hex"
	"encoding/json"
	"flag"
	"fmt"
	"math"
	"os"
	"os/exec"
	"path/filepath"
	"regexp"
	"runtime"
	"sort"
	"strings"
	"sync"
	"time"
	"unsafe"
)

// ---------------------------------------------------------------- 路径与常量

var (
	projectRoot   = capi.ProjectRoot()
	nativeDir     = filepath.Join(projectRoot, "native")
	dllPath       = filepath.Join(nativeDir, "target", "release", "cide_native.dll")
	scriptDir     = filepath.Join(nativeDir, "tests", "shadow_verification")
	clangCacheDir = filepath.Join(scriptDir, ".clang_cache")
	runRoot       = filepath.Join(scriptDir, ".shadow_tmp")
	reportsDir    = filepath.Join(scriptDir, "reports")

	clangPath = "clang"

	// Go 自有缓存 schema：key 组成变化时改值（等价全量失效）。与 Python 的整数
	// schema=1 有意不同值，防止任何跨驱动的缓存误用。
	clangCacheSchema    = "go1"
	clangCompileTimeout = 30 * time.Second
	clangRunTimeout     = 5 * time.Second
	// CI runner 上 clang 偶发瞬时失败（D5 第一站实测，shadow_verify_cpp.go 同款）
	clangRetry = 3

	// VFS 预设文件：Cide 在 setup_vm / inject_preset_files 中注入同样内容，
	// 这里写物理文件让 Clang 侧看到一致的文件环境。
	presetFiles = map[string]string{
		"test.txt":    "hello\nworld\n",
		"numbers.txt": "1 2 3 4 5\n",
	}

	// 已知失败用例（与 E2E 防线 cide_e2e.rs 的 KNOWN_TEMPLATE_FAILURES 常量对齐，
	// 根因分析见 native/tests/E2E_FAILURES.md）。这些模板在 Cide VM 的边界检查下
	// 触发陷阱而 Clang 静默 UB，属于已记录的教学差异，门禁不视为回归。
	// ⚠️ 防线 5 双向监控约定：若这些用例在 E2E 防线转绿，需同步移除此处条目。
	knownFailureCases = map[string]bool{
		"bTree_default": true, // E2E_FAILURES.md：未插入元素时访问 NULL 指针区域
		"spfa_default":  true, // E2E_FAILURES.md：队列大小 MAXV(5) 不足导致越界
	}
)

// ---------------------------------------------------------------- 数据结构

type runResult struct {
	Compiler       string  `json:"compiler"`
	CompileSuccess bool    `json:"compile_success"`
	CompileError   string  `json:"compile_error"`
	RunSuccess     bool    `json:"run_success"`
	RunError       string  `json:"run_error"`
	Stdout         string  `json:"stdout"`
	Stderr         string  `json:"stderr"`
	ExitCode       int     `json:"exit_code"`
	DurationMs     float64 `json:"duration_ms"`
	// 该结果来自 Clang 结果缓存（未真实执行 Clang）；仅用于报告标注
	Cached bool `json:"cached,omitempty"`
	// 运行环境异常（进程启动失败 / 超时被 kill），非被测程序的确定性行为：
	// 值得重试，且不得写入 Clang 结果缓存（宁重算，不用不可信 Golden）。
	// Python 版把超时异常同样落缓存（固化风险尚未踩到），此处为有意加固。
	abnormal bool `json:"-"`
}

type shadowCase struct {
	name     string
	source   string // 清洗后源码（剔除 "// @" 行；与 Cide compile_unit 收到的一致）
	category string // 预期分类，如 "double"、"arch_diff_bug"
	srcDir   string // baseline / gap / template / knr / leetcode
	path     string // 源文件绝对路径（#include 解析、缓存 origin）
	stdin    string // 同名 .in 内容（universal-newlines 归一后；空串 = 无输入）
}

// normalizeNewlines 等价 Python 文本模式读取的 universal newlines：
// \r\n → \n、孤立 \r → \n。用例源码 / .in 内容统一经此归一，
// 保证 Cide 与缓存材料看到的字节与 Python 驱动一致。
func normalizeNewlines(s string) string {
	if !strings.ContainsRune(s, '\r') {
		return s
	}
	s = strings.ReplaceAll(s, "\r\n", "\n")
	return strings.ReplaceAll(s, "\r", "\n")
}

// ---------------------------------------------------------------- 启动自检（J9）

// selfCheck 对 analyzeDiff / classifyCompileError 注入必然违反的输入，断言判定必须变红。
// 自检不过 → 退出码 2 拒绝运行：判定函数坏了的"全绿"比没有门禁更坏。
func selfCheck() {
	clangOK := runResult{Compiler: "clang", CompileSuccess: true, RunSuccess: true, Stdout: "1\n2\n", ExitCode: 0}
	cideOK := runResult{Compiler: "cide", CompileSuccess: true, RunSuccess: true, Stdout: "1\n2\n", ExitCode: 0}
	c := func(name, category string) shadowCase {
		return shadowCase{name: name, category: category}
	}
	checks := []struct {
		name  string
		cs    shadowCase
		clang runResult
		cide  runResult
		want  string
	}{
		{"两侧一致 → match", c("x", "baseline"), clangOK, cideOK, "match"},
		{"cide 编译失败 → compile_gap", c("x", "baseline"), clangOK, runResult{Compiler: "cide", CompileSuccess: false}, "compile_gap"},
		// 语义雷区：主驱动口径——两侧都编译失败是 match（用例本身可能有问题），
		// 不是 C++ 版的 clang_compile_fail；若变成 compile_gap 会制造全量假红。
		{"都编译失败 → match", c("x", "baseline"), runResult{Compiler: "clang", CompileSuccess: false}, runResult{Compiler: "cide", CompileSuccess: false}, "match"},
		{"clang 编译失败而 cide 通过 → cide_better", c("x", "baseline"), runResult{Compiler: "clang", CompileSuccess: false}, cideOK, "cide_better"},
		{"都运行失败 → match", c("x", "baseline"), runResult{Compiler: "clang", CompileSuccess: true, RunSuccess: false, RunError: "boom"}, runResult{Compiler: "cide", CompileSuccess: true, RunSuccess: false, RunError: "boom"}, "match"},
		{"仅 cide 运行失败 → runtime_gap", c("x", "baseline"), clangOK, runResult{Compiler: "cide", CompileSuccess: true, RunSuccess: false, RunError: "trap"}, "runtime_gap"},
		// KNOWN_FAILURE_CASES 豁免：已记录的教学差异不统计为 runtime_gap
		{"已知失败用例运行失败 → known_issue", c("bTree_default", "baseline"), clangOK, runResult{Compiler: "cide", CompileSuccess: true, RunSuccess: false, RunError: "trap"}, "known_issue"},
		{"输出不同 → output_gap", c("x", "baseline"), clangOK, runResult{Compiler: "cide", CompileSuccess: true, RunSuccess: true, Stdout: "1\n3\n"}, "output_gap"},
		// category 含 "bug" 豁免：已记录的架构差异不统计为 output_gap
		{"输出不同但 category 含 bug → known_issue", c("x", "arch_diff_bug"), clangOK, runResult{Compiler: "cide", CompileSuccess: true, RunSuccess: true, Stdout: "1\n3\n"}, "known_issue"},
		{"输出不同但用例已知 → known_issue", c("spfa_default", "baseline"), clangOK, runResult{Compiler: "cide", CompileSuccess: true, RunSuccess: true, Stdout: "1\n3\n"}, "known_issue"},
		// 语义雷区：这两条若变红，说明 strip / CRLF 归一口径被破坏——
		// Python 版靠 .strip() + universal newlines 保持 match，Go 版必须同语义。
		{"尾部空白差异仍 → match", c("x", "baseline"), clangOK, runResult{Compiler: "cide", CompileSuccess: true, RunSuccess: true, Stdout: "1\n2\n   \n"}, "match"},
		{"CRLF/LF 差异仍 → match", c("x", "baseline"), clangOK, runResult{Compiler: "cide", CompileSuccess: true, RunSuccess: true, Stdout: "1\r\n2\r\n"}, "match"},
	}
	for _, ck := range checks {
		if got := analyzeDiff(ck.cs, ck.clang, ck.cide); got != ck.want {
			capi.Fatal("启动自检失败：%s：期望 %s，实际 %s（判定口径已破坏，拒绝运行）", ck.name, ck.want, got)
		}
	}
	// classifyCompileError：expected_category 优先 + 关键词 fallback
	if got := classifyCompileError("error: unknown type name 'double'", "file_io"); got != "file_io" {
		capi.Fatal("启动自检失败：expected_category 应优先（期望 file_io，实际 %s）", got)
	}
	if got := classifyCompileError("E1001: double not supported", "baseline"); got != "double" {
		capi.Fatal("启动自检失败：关键词 fallback 失效（期望 double，实际 %s）", got)
	}
	if got := classifyCompileError("", "baseline"); got != "unknown" {
		capi.Fatal("启动自检失败：空错误应归类 unknown（实际 %s）", got)
	}
	fmt.Printf("启动自检：analyzeDiff %d 条 + classifyCompileError 3 条断言全部通过\n", len(checks))
}

// ---------------------------------------------------------------- Clang 侧

func clangCompileArgs(cFile, exeFile string) []string {
	args := []string{cFile, "-o", exeFile, "-Wno-implicit-function-declaration"}
	if runtime.GOOS != "windows" {
		args = append(args, "-lm")
	}
	return args
}

// clangCmdSignature Clang 编译命令的稳定签名（缓存 key 用，不含随机/临时路径）。
func clangCmdSignature() []string {
	return append([]string{clangPath, "<src>", "-o", "<exe>"}, clangCompileArgs("<src>", "<exe>")[1:]...)
}

// runWithClang 编译失败或环境异常（启动失败/超时）时重试（CI runner 瞬时故障，
// D5 第一站实证），确定性行为（程序自身 exit != 0）不重试。
func runWithClang(cs shadowCase, runDir string) runResult {
	var result runResult
	for attempt := 0; attempt < clangRetry; attempt++ {
		result = runWithClangOnce(cs, runDir)
		if result.CompileSuccess && !result.abnormal {
			return result
		}
		time.Sleep(time.Duration(500*(attempt+1)) * time.Millisecond)
	}
	return result
}

// runWithClangOnce 用 Clang 编译并运行单用例。
//
// runDir 是本 worker 槽位的隔离运行目录：编译产物（可执行文件）落在其中，
// 运行 cwd 设为该目录——用例 fopen 写出的文件与预设 test.txt / numbers.txt
// 都在其中，并行槽位之间互不干扰。文件用例直接在原目录编译（#include 可解析）。
func runWithClangOnce(cs shadowCase, runDir string) runResult {
	start := time.Now()
	// per-case 唯一产物名（对齐 C++ 版第一站的实证形态）：Windows 上 16 路并发
	// 快速覆盖+执行同名 test.exe 会触发映像加载竞态（实测确定性 0xC0000005）。
	exeFile := filepath.Join(runDir, "test_"+cs.name+".exe")
	if runtime.GOOS != "windows" {
		exeFile = filepath.Join(runDir, "test_"+cs.name)
	}

	// 文件用例通常自带 #include <stdio.h>，无需再补充头文件。
	// （Python 版对无路径的内联用例有 make_clang_header 分支；本驱动全部用例
	// 来自目录加载，该分支不再存在——文件缺失属仓库损坏，fail loud。）
	if _, err := os.Stat(cs.path); err != nil {
		return runResult{Compiler: "clang", CompileSuccess: false, CompileError: err.Error(), ExitCode: -1,
			DurationMs: msSince(start)}
	}

	// 编译（Windows MSVC 环境下不需要 -lm，Linux/Android 需要）
	compileCtx, compileCancel := context.WithTimeout(context.Background(), clangCompileTimeout)
	defer compileCancel()
	compileCmd := exec.CommandContext(compileCtx, clangPath, clangCompileArgs(cs.path, exeFile)...)
	var compileOut, compileErrBuf bytes.Buffer
	compileCmd.Stdout = &compileOut
	compileCmd.Stderr = &compileErrBuf
	compileErr := compileCmd.Run()
	if compileErr != nil {
		// 对齐 Python：compile_error 取 clang 的 stderr；起进程失败时取 err 文本。
		msg := compileErr.Error()
		if compileErrBuf.Len() > 0 {
			msg = compileErrBuf.String()
		}
		code := 1
		if compileCmd.ProcessState != nil {
			code = compileCmd.ProcessState.ExitCode()
		}
		// 超时被 kill / 进程未启动 = 环境异常（Defender 首扫、runner 瞬态），可重试不落缓存
		abnormal := compileCtx.Err() != nil || compileCmd.ProcessState == nil
		return runResult{Compiler: "clang", CompileSuccess: false, CompileError: msg,
			Stderr: compileErrBuf.String(), ExitCode: code, DurationMs: msSince(start), abnormal: abnormal}
	}

	// 运行（cwd = 隔离运行目录；stdin 与 Cide 侧喂同一份字节）
	runCtx, runCancel := context.WithTimeout(context.Background(), clangRunTimeout)
	defer runCancel()
	runCmd := exec.CommandContext(runCtx, exeFile)
	runCmd.Dir = runDir
	var runOut, runErrBuf bytes.Buffer
	runCmd.Stdout = &runOut
	runCmd.Stderr = &runErrBuf
	if cs.stdin != "" {
		runCmd.Stdin = strings.NewReader(cs.stdin)
	}
	runErr := runCmd.Run()
	runCode := -1
	if runCmd.ProcessState != nil {
		runCode = runCmd.ProcessState.ExitCode()
	}
	if runErr != nil {
		// 两条路径的语义映射（与 Python subprocess.run 对照，run 不带 check）：
		//   - 程序自身非零退出：Go 返回 *ExitError，Python **正常返回** CompletedProcess
		//     —— 输出保留，run_success=(exit==0)、run_error=stderr。e1_func_identifier
		//     的 `return helper()`（exit 1）走的就是这条，属确定性行为不是异常；
		//   - 超时被 kill / 进程启动失败 / 异常终止码（0xC0000005 等映像级崩溃，
		//     正常教学程序 exit 只落在 0~0xFFF）：对齐 except 分支——丢弃部分输出、
		//     run_error 取异常文本、标记 abnormal（可重试不落缓存）。
		if runCtx.Err() != nil || runCmd.ProcessState == nil || runCode < 0 || runCode > 0xFFF {
			return runResult{Compiler: "clang", CompileSuccess: true,
				RunSuccess: false, RunError: runErr.Error(), ExitCode: runCode,
				DurationMs: msSince(start), abnormal: true}
		}
		return runResult{Compiler: "clang", CompileSuccess: true,
			RunSuccess: runCode == 0, RunError: errStringIf(runErrBuf.String(), runCode != 0),
			Stdout: runOut.String(), Stderr: runErrBuf.String(), ExitCode: runCode,
			DurationMs: msSince(start)}
	}
	return runResult{Compiler: "clang", CompileSuccess: true,
		RunSuccess: runCode == 0, RunError: errStringIf(runErrBuf.String(), runCode != 0),
		Stdout: runOut.String(), Stderr: runErrBuf.String(), ExitCode: runCode,
		DurationMs: msSince(start)}
}

func errStringIf(s string, cond bool) string {
	if cond {
		return s
	}
	return ""
}

func msSince(start time.Time) float64 {
	return float64(time.Since(start).Milliseconds())
}

// normalizeRunDir 把结果文本中的 worker 私有路径归一化，保证缓存与报告跨槽位确定性。
func normalizeRunDir(result *runResult, runDir string) {
	for _, field := range []*string{&result.CompileError, &result.RunError, &result.Stdout, &result.Stderr} {
		if *field != "" && strings.Contains(*field, runDir) {
			*field = strings.ReplaceAll(*field, runDir, "<rundir>")
		}
	}
}

// ---------------------------------------------------------------- Clang 结果缓存（Go 自有 schema）

// cacheMaterial 缓存 key 的组成材料：源码 + stdin + clang 版本 + 参数 + 超时 +
// 预设文件 + 同目录头文件。任何一项变化都会让旧缓存自然失效。
type cacheMaterial struct {
	Schema       string            `json:"schema"`
	Platform     string            `json:"platform"`
	ClangVersion string            `json:"clang_version"`
	CompileCmd   []string          `json:"compile_cmd"`
	CompileTO    int               `json:"compile_timeout_ms"`
	RunTO        int               `json:"run_timeout_ms"`
	Origin       string            `json:"origin"`
	Source       string            `json:"source"`
	Stdin        string            `json:"stdin"`
	PresetFiles  map[string]string `json:"preset_files"`
	IncludeFiles map[string]string `json:"include_files"`
}

// caseOrigin 用例来源标识：仓库内相对路径（跨 checkout 稳定，缓存可跨机器共享）。
func caseOrigin(cs shadowCase) string {
	rel, err := filepath.Rel(nativeDir, cs.path)
	if err != nil {
		return filepath.Base(cs.path)
	}
	return filepath.ToSlash(rel)
}

func clangCacheKey(cs shadowCase, clangVersion string) string {
	sourceText, err := os.ReadFile(cs.path)
	if err != nil {
		capi.Fatal("缓存材料读取用例失败 %s: %v", cs.path, err)
	}
	includeFiles := map[string]string{}
	if entries, err := os.ReadDir(filepath.Dir(cs.path)); err == nil {
		var headers []string
		for _, e := range entries {
			if !e.IsDir() && strings.HasSuffix(e.Name(), ".h") {
				headers = append(headers, e.Name())
			}
		}
		sort.Strings(headers)
		for _, h := range headers {
			data, err := os.ReadFile(filepath.Join(filepath.Dir(cs.path), h))
			if err != nil {
				capi.Fatal("缓存材料读取头文件失败 %s: %v", h, err)
			}
			sum := sha256.Sum256(data)
			includeFiles[h] = hex.EncodeToString(sum[:])
		}
	}
	presets := map[string]string{}
	for name, data := range presetFiles {
		sum := sha256.Sum256([]byte(data))
		presets[name] = hex.EncodeToString(sum[:])
	}
	material := cacheMaterial{
		Schema:       clangCacheSchema,
		Platform:     runtime.GOOS,
		ClangVersion: clangVersion,
		CompileCmd:   clangCmdSignature(),
		CompileTO:    int(clangCompileTimeout.Milliseconds()),
		RunTO:        int(clangRunTimeout.Milliseconds()),
		Origin:       caseOrigin(cs),
		Source:       normalizeNewlines(string(sourceText)),
		Stdin:        cs.stdin,
		PresetFiles:  presets,
		IncludeFiles: includeFiles,
	}
	var buf bytes.Buffer
	enc := json.NewEncoder(&buf)
	enc.SetEscapeHTML(false)
	if err := enc.Encode(material); err != nil {
		capi.Fatal("缓存材料序列化失败: %v", err)
	}
	sum := sha256.Sum256(buf.Bytes())
	return hex.EncodeToString(sum[:])
}

type cachePayload struct {
	Schema  string    `json:"schema"`
	Key     string    `json:"key"`
	Created string    `json:"created"`
	Result  runResult `json:"result"`
}

// clangCacheLoad 读缓存；缺失/损坏/schema 不符一律视为未命中（宁重算，不用不可信 Golden）。
func clangCacheLoad(key string) *runResult {
	data, err := os.ReadFile(filepath.Join(clangCacheDir, key+".json"))
	if err != nil {
		return nil
	}
	var payload cachePayload
	if err := json.Unmarshal(data, &payload); err != nil {
		return nil
	}
	if payload.Schema != clangCacheSchema {
		return nil
	}
	r := payload.Result
	r.DurationMs = 0
	r.Cached = true
	return &r
}

// clangCacheStore 原子落盘：先写临时文件再 rename，避免并行槽位读到半截 JSON。
func clangCacheStore(key string, result runResult) {
	if err := os.MkdirAll(clangCacheDir, 0o755); err != nil {
		return // 缓存写失败不致命：只影响下次命中率
	}
	payload := cachePayload{
		Schema:  clangCacheSchema,
		Key:     key,
		Created: time.Now().Format("2006-01-02 15:04:05"),
		Result:  result,
	}
	data, err := json.Marshal(payload)
	if err != nil {
		return
	}
	tmp := filepath.Join(clangCacheDir, fmt.Sprintf(".%s.%d.tmp", key, os.Getpid()))
	if err := os.WriteFile(tmp, data, 0o644); err != nil {
		return
	}
	_ = os.Rename(tmp, filepath.Join(clangCacheDir, key+".json"))
}

// ---------------------------------------------------------------- Cide 侧

// cideMu：引擎 DLL 会话级线程安全性已实证为不安全（并发 → 堆损坏，见 AGENTS.md
// D5 进度），Cide 侧全程互斥串行；Clang 侧是耗时大头，由 goroutine 池并发。
var cideMu sync.Mutex

// runWithCide 通过 C API 调用 Cide 编译并运行（文件用例走 compile_unit+compile_all）。
func runWithCide(d *capi.DLL, cs shadowCase) runResult {
	cideMu.Lock()
	defer cideMu.Unlock()

	start := time.Now()
	handle, _, _ := d.SessionCreate.Call()
	if handle == 0 {
		return runResult{Compiler: "cide", CompileSuccess: false, CompileError: "session create failed", ExitCode: -1,
			DurationMs: msSince(start)}
	}
	defer d.SessionDestroy.Call(handle)

	var compileRet uintptr
	if cs.path != "" {
		pathB := capi.CBytes(cs.path)
		srcB := capi.CBytes(cs.source)
		compileRet, _, _ = d.CompileUnit.Call(handle,
			uintptr(unsafe.Pointer(&pathB[0])), uintptr(unsafe.Pointer(&srcB[0])))
		runtime.KeepAlive(pathB)
		runtime.KeepAlive(srcB)
		if compileRet == 0 {
			compileRet, _, _ = d.CompileAll.Call(handle)
		}
	} else {
		srcB := capi.CBytes(cs.source)
		compileRet, _, _ = d.Compile.Call(handle, uintptr(unsafe.Pointer(&srcB[0])))
		runtime.KeepAlive(srcB)
	}
	if int32(compileRet) != 0 {
		errMsg := d.CompileErrorsExact(handle)
		if errMsg == "" {
			errMsg = "Unknown compile error"
		}
		return runResult{Compiler: "cide", CompileSuccess: false, CompileError: errMsg,
			Stderr: errMsg, ExitCode: int(int32(compileRet)), DurationMs: msSince(start)}
	}

	d.SetInputMode.Call(handle, 1)
	if cs.stdin != "" {
		// 与 Clang 侧喂同一份字节，保证两侧可比
		inB := capi.CBytes(cs.stdin)
		d.SetInput.Call(handle, uintptr(unsafe.Pointer(&inB[0])))
		runtime.KeepAlive(inB)
	}
	runRet, _, _ := d.Run.Call(handle)

	// E-P1-5：直接读纯程序 stdout 通道（引擎附注走 note 通道），禁止文本清洗。
	stdoutStr := strings.TrimSpace(capi.ReadChannel(handle, d.ProgOutLen, d.ProgOut))
	runtimeErr := d.RuntimeErr(handle)

	return runResult{Compiler: "cide", CompileSuccess: true,
		RunSuccess: int32(runRet) == 0 && runtimeErr == "",
		RunError:   runtimeErr, Stdout: stdoutStr, Stderr: runtimeErr,
		ExitCode:   int(int32(runRet)),
		DurationMs: msSince(start)}
}

// ---------------------------------------------------------------- 判定

// analyzeDiff 分析 Clang 和 Cide 的差异（主驱动判定树，与 Python 版逐分支对齐）。
func analyzeDiff(cs shadowCase, clang, cide runResult) string {
	if clang.CompileSuccess && !cide.CompileSuccess {
		return "compile_gap"
	}
	if clang.CompileSuccess && cide.CompileSuccess {
		if !clang.RunSuccess && !cide.RunSuccess {
			return "match" // 都失败
		}
		if clang.RunSuccess && !cide.RunSuccess {
			// 已记录的模板运行失败（E2E_FAILURES.md 有根因）不算回归
			if knownFailureCases[cs.name] {
				return "known_issue"
			}
			return "runtime_gap"
		}
		if capi.Normalize(clang.Stdout) != capi.Normalize(cide.Stdout) {
			// 已知问题（预期行为差异）不统计为 output_gap
			if strings.Contains(cs.category, "bug") || knownFailureCases[cs.name] {
				return "known_issue"
			}
			return "output_gap"
		}
		return "match"
	}
	if !clang.CompileSuccess && !cide.CompileSuccess {
		return "match" // 都编译失败（可能是用例本身有问题）
	}
	return "cide_better" // Cide 通过但 Clang 失败（罕见）
}

// compileErrorPatterns 缺失特性分类关键词（有序，先到先得；与 Python 版同序同词）。
var compileErrorPatterns = []struct {
	category string
	keywords []string
}{
	{"double", []string{"double"}},
	{"function_pointer", []string{"function pointer", "expected identifier"}},
	{"file_io", []string{"fopen", "fclose", "fread", "fwrite", "fprintf", "stdin", "stdout"}},
	{"preprocessor", []string{"#include", "#ifdef", "#ifndef", "#pragma"}},
	{"union", []string{"union"}},
	{"bitfield", []string{"bitfield"}},
	{"goto", []string{"goto"}},
	{"switch_fallthrough", []string{"fallthrough"}},
	{"inline_asm", []string{"asm", "__asm__"}},
	{"complex_number", []string{"complex", "_Complex"}},
	{"long_long", []string{"long long"}},
	{"variadic_macro", []string{"...", "__VA_ARGS__"}},
	{"typeof", []string{"typeof"}},
	{"static_assert", []string{"static_assert"}},
	{"designated_initializer", []string{"designated"}},
	{"variable_length_array", []string{"vla", "variable length"}},
	{"missing_header", []string{"stdio.h", "stdlib.h", "string.h", "math.h"}},
	{"const_string", []string{"char*", "const"}},
}

// classifyCompileError 根据 Cide 编译错误消息分类缺失特性。
// 优先使用用例本身的 expected_category（如果已知且不是 baseline），再用错误信息
// 关键词作为 fallback 分类。
func classifyCompileError(errorMsg, expectedCategory string) string {
	if expectedCategory != "" && expectedCategory != "baseline" && expectedCategory != "unknown" {
		return expectedCategory
	}
	errLower := strings.ToLower(errorMsg)
	for _, p := range compileErrorPatterns {
		for _, kw := range p.keywords {
			if strings.Contains(errLower, kw) {
				return p.category
			}
		}
	}
	return "unknown"
}

// ---------------------------------------------------------------- 用例加载

var categoryRe = regexp.MustCompile(`@category:\s*([A-Za-z0-9_\-]+)`)

// loadCaseFiles 从五个目录的 .c 文件加载用例。
//
// sorted(glob)：目录遍历顺序跨进程/平台不保证一致，必须排序——并行槽位若各自
// 得到不同顺序，按索引对账就会错配（2026-09-11 实测踩到）。
// 目录间按声明顺序串联，--limit 取前 N 的语义依赖此顺序。
func loadCaseFiles() []shadowCase {
	type caseDir struct {
		rel    string
		srcDir string
	}
	dirs := []caseDir{
		{filepath.Join("tests", "cases", "baseline"), "baseline"},
		{filepath.Join("tests", "cases", "gap"), "gap"},
		{filepath.Join("tests", "cases_template_generated"), "template"},
		{filepath.Join("tests", "cases", "knr"), "knr"},
		{filepath.Join("tests", "cases", "leetcode"), "leetcode"},
	}
	var cases []shadowCase
	for _, d := range dirs {
		rootPath := filepath.Join(nativeDir, d.rel)
		entries, err := os.ReadDir(rootPath)
		if err != nil {
			continue // 目录不存在：与 Python 版同口径，跳过
		}
		var names []string
		for _, e := range entries {
			if !e.IsDir() && strings.HasSuffix(e.Name(), ".c") {
				names = append(names, e.Name())
			}
		}
		// 大小写不敏感排序（casefold 平局比原串）：Python pathlib 的 __lt__ 在
		// Windows 上按大小写规范化字符串比较，实测即此序；Linux 上它是码点序
		// （平台相关缺陷）。Go 统一取 casefold——跨平台顺序稳定，--limit 语义
		// 不随 runner 系统漂移。
		sort.Slice(names, func(i, j int) bool {
			li, lj := strings.ToLower(names[i]), strings.ToLower(names[j])
			if li != lj {
				return li < lj
			}
			return names[i] < names[j]
		})
		for _, fn := range names {
			path := filepath.Join(rootPath, fn)
			raw, err := os.ReadFile(path)
			if err != nil {
				capi.Fatal("读取用例失败 %s: %v", path, err)
			}
			source := normalizeNewlines(string(raw))
			// 提取 @category 注释。占位符限定 ASCII：`\S` 类宽松匹配会跨越 C 注释
			// 里的中文标点，把整段注释吞进 category（实测污染用例名）。
			category := "baseline"
			if m := categoryRe.FindStringSubmatch(source); m != nil {
				category = m[1]
			}
			// 移除注释标记，保留纯源码（剔除判定用 strip 后前缀，输出保留原行）
			var cleanLines []string
			for _, line := range strings.Split(source, "\n") {
				if strings.HasPrefix(strings.TrimSpace(line), "// @") {
					continue
				}
				cleanLines = append(cleanLines, line)
			}
			cleanSource := strings.Join(cleanLines, "\n")
			inPath := strings.TrimSuffix(path, ".c") + ".in"
			stdinText := ""
			if data, err := os.ReadFile(inPath); err == nil {
				stdinText = normalizeNewlines(string(data))
			}
			cases = append(cases, shadowCase{
				name:     strings.TrimSuffix(fn, ".c"),
				source:   cleanSource,
				category: category,
				srcDir:   d.srcDir,
				path:     path,
				stdin:    stdinText,
			})
		}
	}
	if len(cases) == 0 {
		capi.Fatal("用例目录加载为空（native/tests/cases/** 与 cases_template_generated 均无 .c）。\n" +
			"防线 1 不允许在空用例集上给出\"门禁通过\"（空转假绿）。")
	}
	return cases
}

// ---------------------------------------------------------------- 并行执行

type caseOutcome struct {
	index int
	cs    shadowCase
	clang runResult
	cide  runResult
}

// executeCases 执行全部用例，返回按用例序重排的结果（并行下同样确定性）。
//
// 两段流水线（有意与 Python 的"每 worker 串行跑完整用例"不同，性能实证见
// D5 第一站）：
//   - Wave 1：Clang 侧满并发（耗时大头；clang 子进程是独立进程，天然并发安全）；
//   - Wave 2：Cide 侧单线程串行——引擎 DLL 会话级线程安全性已实证为不安全
//     （并发 → 堆损坏，见 AGENTS.md D5 进度）。实测 Cide 单例 ~1.5ms，
//     串行段对总时长影响可忽略，而 Wave 1 的并发度不再被互斥锁稀释。
//
// 每个 Clang 槽位一个隔离运行目录 go_run_{slot}，预设文件写入其中。
func executeCases(cases []shadowCase, jobs int, refreshClang bool, clangVersion string) []caseOutcome {
	total := len(cases)
	outcomes := make([]caseOutcome, total)

	slots := make(chan int, jobs)
	for i := 0; i < jobs; i++ {
		slots <- i
	}
	printMu := &sync.Mutex{}
	completed := 0

	// Wave 1：Clang（走缓存）
	var wg sync.WaitGroup
	for i, cs := range cases {
		wg.Add(1)
		go func(i int, cs shadowCase) {
			defer wg.Done()
			slot := <-slots
			defer func() { slots <- slot }()

			runDir := filepath.Join(runRoot, fmt.Sprintf("go_run_%d", slot))
			ensureRunDir(runDir)

			key := clangCacheKey(cs, clangVersion)
			var clangRes runResult
			if refreshClang {
				clangRes = runWithClang(cs, runDir)
			} else if cached := clangCacheLoad(key); cached != nil {
				clangRes = *cached
			} else {
				clangRes = runWithClang(cs, runDir)
			}
			if !clangRes.Cached {
				normalizeRunDir(&clangRes, runDir)
				if !clangRes.abnormal { // 瞬态环境异常不落缓存，下次重算
					clangCacheStore(key, clangRes)
				}
			}
			outcomes[i] = caseOutcome{index: i, cs: cs, clang: clangRes}

			if jobs > 1 {
				printMu.Lock()
				completed++
				tag := ""
				if clangRes.Cached {
					tag = " (clang 缓存)"
				}
				fmt.Printf("  [%d/%d] %s: clang=%s%s\n", completed, total, cs.name, okOf(clangRes.CompileSuccess), tag)
				printMu.Unlock()
			}
		}(i, cs)
	}
	wg.Wait()

	// Wave 2：Cide（串行，互斥由 runWithCide 内部锁保证）
	for i, cs := range cases {
		outcomes[i].cide = runWithCide(dll, cs)
	}
	return outcomes
}

// ensureRunDir 建槽位隔离运行目录并写 VFS 预设文件（幂等）。
func ensureRunDir(runDir string) {
	if err := os.MkdirAll(runDir, 0o755); err != nil {
		capi.Fatal("无法创建运行目录 %s: %v", runDir, err)
	}
	// 排序遍历：确定性纪律（写文件顺序虽无语义，不留随机序）
	var names []string
	for name := range presetFiles {
		names = append(names, name)
	}
	sort.Strings(names)
	for _, name := range names {
		if err := os.WriteFile(filepath.Join(runDir, name), []byte(presetFiles[name]), 0o644); err != nil {
			capi.Fatal("无法写预设文件 %s: %v", name, err)
		}
	}
}

// ---------------------------------------------------------------- release DLL 陈旧检测

type stalenessInfo struct {
	mtime time.Time
	path  string
}

// dllStaleness 检测 release DLL 是否比引擎源码旧；陈旧则返回最新源码信息。
// 背景（2026-09-11 实际发生）：Shadow 用 target/release DLL，而日常开发跑的是
// debug 构建——改完引擎直接跑 Shadow，跑的其实是改动前的旧引擎。
func dllStaleness() (stalenessInfo, bool) {
	fi, err := os.Stat(dllPath)
	if err != nil {
		return stalenessInfo{}, false
	}
	dllMtime := fi.ModTime()
	var newest time.Time
	var newestPath string
	for _, path := range sourceFilesForFreshness() {
		st, err := os.Stat(path)
		if err != nil {
			continue
		}
		if st.ModTime().After(newest) {
			newest = st.ModTime()
			newestPath = path
		}
	}
	// 容差 2s：避免文件系统时间戳粒度造成的假阳性
	if newestPath != "" && newest.After(dllMtime.Add(2*time.Second)) {
		return stalenessInfo{mtime: newest, path: newestPath}, true
	}
	return stalenessInfo{}, false
}

// sourceFilesForFreshness 引擎源码清单：native/src/** + native/crates/** + 两处 Cargo.toml。
func sourceFilesForFreshness() []string {
	var files []string
	files = append(files, filepath.Join(nativeDir, "Cargo.toml"))
	if _, err := os.Stat(filepath.Join(nativeDir, "build.rs")); err == nil {
		files = append(files, filepath.Join(nativeDir, "build.rs"))
	}
	for _, base := range []string{filepath.Join(nativeDir, "src"), filepath.Join(nativeDir, "crates")} {
		filepath.WalkDir(base, func(path string, d os.DirEntry, err error) error {
			if err != nil {
				return nil
			}
			if d.IsDir() {
				if d.Name() == "target" {
					return filepath.SkipDir
				}
				return nil
			}
			if strings.HasSuffix(path, ".rs") || filepath.Base(path) == "Cargo.toml" {
				files = append(files, path)
			}
			return nil
		})
	}
	sort.Strings(files) // 确定性（仅影响 newest 平局判定）
	return files
}

func warnStaleDLL(dllMtime, sourceMtime time.Time, sourcePath string) {
	rel, err := filepath.Rel(projectRoot, sourcePath)
	if err != nil {
		rel = sourcePath
	}
	bar := strings.Repeat("!", 66)
	fmt.Println("\n" + bar)
	fmt.Println("⚠️  release DLL 比引擎源码旧 —— 本次 Shadow 跑的是**旧引擎**：")
	fmt.Printf("    DLL 构建时间 : %s\n", dllMtime.Format("2006-01-02 15:04:05"))
	fmt.Printf("    最新源码修改 : %s  %s\n", sourceMtime.Format("2006-01-02 15:04:05"), rel)
	fmt.Println("    结论不代表当前源码。加 --rebuild 自动重建，或手动执行：")
	fmt.Println("      cd native && cargo build --release")
	fmt.Println(bar + "\n")
}

func rebuildReleaseDLL() {
	fmt.Println("\n[--rebuild] release DLL 陈旧，重建引擎：cargo build --release")
	cmd := exec.Command("cargo", "build", "--release")
	cmd.Dir = nativeDir
	cmd.Stdout = os.Stdout
	cmd.Stderr = os.Stderr
	if err := cmd.Run(); err != nil {
		fmt.Println("错误: cargo build --release 失败，终止（结论不可信，不产出报告）。")
		os.Exit(1)
	}
}

// ---------------------------------------------------------------- 报告

type detailEntry struct {
	Case             string `json:"case"`
	Expected         string `json:"expected"`
	DiffType         string `json:"diff_type"`
	CideCompileError string `json:"cide_compile_error"`
}

type srcStat struct {
	Total      int `json:"total"`
	Match      int `json:"match"`
	CompileGap int `json:"compile_gap"`
	RuntimeGap int `json:"runtime_gap"`
	OutputGap  int `json:"output_gap"`
}

type diffInfo struct {
	cs    shadowCase
	diff  string
	clang runResult
	cide  runResult
}

type runConfig struct {
	jobs       int
	elapsedSec float64
	refresh    bool
	hits       int
	misses     int
}

// generateReport 生成分类统计 Markdown 报告（与 Python 版逐行同构）。
func generateReport(diffs []diffInfo, outputPath string, cfg *runConfig) {
	var compileGaps, runtimeGaps, outputGaps, matches []diffInfo
	for _, d := range diffs {
		switch d.diff {
		case "compile_gap":
			compileGaps = append(compileGaps, d)
		case "runtime_gap":
			runtimeGaps = append(runtimeGaps, d)
		case "output_gap":
			outputGaps = append(outputGaps, d)
		case "match":
			matches = append(matches, d)
		}
	}

	// 编译缺口按分类统计
	categoryCounts := map[string]int{}
	var categoryOrder []string
	categoryCases := map[string][]string{}
	for _, d := range compileGaps {
		cat := classifyCompileError(d.cide.CompileError, d.cs.category)
		if _, seen := categoryCounts[cat]; !seen {
			categoryOrder = append(categoryOrder, cat)
		}
		categoryCounts[cat]++
		categoryCases[cat] = append(categoryCases[cat], d.cs.name)
	}
	sort.SliceStable(categoryOrder, func(i, j int) bool {
		return categoryCounts[categoryOrder[i]] > categoryCounts[categoryOrder[j]]
	})

	// 按来源目录统计（knr / leetcode / baseline / template / gap / builtin）
	srcStats := map[string]*srcStat{}
	var srcOrder []string
	for _, d := range diffs {
		src := d.cs.srcDir
		if src == "" {
			src = "other"
		}
		if _, seen := srcStats[src]; !seen {
			srcStats[src] = &srcStat{}
			srcOrder = append(srcOrder, src)
		}
		s := srcStats[src]
		s.Total++
		switch d.diff {
		case "match":
			s.Match++
		case "compile_gap":
			s.CompileGap++
		case "runtime_gap":
			s.RuntimeGap++
		case "output_gap":
			s.OutputGap++
		}
	}
	sort.Strings(srcOrder)

	total := len(diffs)
	lines := []string{
		"# Cide 影子验证报告",
		"",
		fmt.Sprintf("生成时间: %s", time.Now().Format("2006-01-02 15:04:05")),
		fmt.Sprintf("总用例数: %d", total),
		fmt.Sprintf("完全匹配: %d (%d%%)", len(matches), len(matches)*100/total),
		fmt.Sprintf("编译缺口: %d (%d%%)", len(compileGaps), len(compileGaps)*100/total),
		fmt.Sprintf("运行时缺口: %d", len(runtimeGaps)),
		fmt.Sprintf("输出差异: %d", len(outputGaps)),
	}
	if cfg != nil {
		cacheLine := fmt.Sprintf("执行配置: jobs=%d, 耗时=%gs, Clang 缓存命中=%d/%d",
			cfg.jobs, cfg.elapsedSec, cfg.hits, cfg.hits+cfg.misses)
		if cfg.refresh {
			cacheLine += "（--refresh-clang 全量重算）"
		}
		lines = append(lines, cacheLine)
	}
	lines = append(lines,
		"",
		"## 按来源目录统计",
		"",
		"| 来源 | 总数 | 匹配 | 编译缺口 | 运行时缺口 | 输出差异 |",
		"|------|------|------|----------|------------|----------|",
	)
	for _, src := range srcOrder {
		s := srcStats[src]
		lines = append(lines, fmt.Sprintf("| %s | %d | %d | %d | %d | %d |",
			src, s.Total, s.Match, s.CompileGap, s.RuntimeGap, s.OutputGap))
	}

	lines = append(lines, "", "## 缺失特性频率排序（编译缺口）", "")
	for _, cat := range categoryOrder {
		count := categoryCounts[cat]
		examples := categoryCases[cat]
		if len(examples) > 3 {
			examples = examples[:3]
		}
		lines = append(lines, fmt.Sprintf("- **%s**: %d 个用例 (%d%%) — 示例: %s",
			cat, count, count*100/total, strings.Join(examples, ", ")))
	}

	lines = append(lines, "", "## 详细差异", "")
	okTag := func(b bool) string {
		if b {
			return "OK"
		}
		return "FAIL"
	}
	for _, d := range diffs {
		if d.diff == "match" {
			continue
		}
		lines = append(lines, "", fmt.Sprintf("### %s [%s]", d.cs.name, d.diff))
		lines = append(lines, fmt.Sprintf("- 预期分类: %s", d.cs.category))
		lines = append(lines, fmt.Sprintf("- Clang: compile=%s, run=%s", okTag(d.clang.CompileSuccess), okTag(d.clang.RunSuccess)))
		lines = append(lines, fmt.Sprintf("- Cide: compile=%s, run=%s", okTag(d.cide.CompileSuccess), okTag(d.cide.RunSuccess)))
		if !d.cide.CompileSuccess {
			lines = append(lines, fmt.Sprintf("- Cide 编译错误: %s", capi.TruncateRunes(d.cide.CompileError, 200)))
		} else if strings.TrimSpace(d.clang.Stdout) != strings.TrimSpace(d.cide.Stdout) {
			lines = append(lines, fmt.Sprintf("- Clang stdout: %s", capi.TruncateRunes(strings.TrimSpace(d.clang.Stdout), 200)))
			lines = append(lines, fmt.Sprintf("- Cide stdout: %s", capi.TruncateRunes(strings.TrimSpace(d.cide.Stdout), 200)))
		}
	}

	if err := os.MkdirAll(filepath.Dir(outputPath), 0o755); err != nil {
		capi.Fatal("无法创建报告目录: %v", err)
	}
	if err := os.WriteFile(outputPath, []byte(strings.Join(lines, "\n")), 0o644); err != nil {
		capi.Fatal("无法写报告 %s: %v", outputPath, err)
	}
	fmt.Printf("报告已生成: %s\n", outputPath)
}

// writeJSONData 输出 JSON 数据（结构与 Python 版同字段，供 engineering_health.py 等下游读取）。
func buildJSONData(diffs []diffInfo, clangVersion string, cfg *runConfig) map[string]any {
	return map[string]any{
		"timestamp":     time.Now().Format("2006-01-02 15:04:05"),
		"clang_version": clangVersion,
		"config": map[string]any{
			"jobs":        cfg.jobs,
			"elapsed_sec": cfg.elapsedSec,
			"clang_cache": map[string]any{
				"refresh": cfg.refresh,
				"hits":    cfg.hits,
				"misses":  cfg.misses,
			},
		},
		"summary":            summaryOf(diffs),
		"category_frequency": categoryFrequencyOf(diffs),
		"details":            detailsOf(diffs),
	}
}

func writeJSONData(diffs []diffInfo, outputPath string, clangVersion string, cfg *runConfig) {
	if err := writeJSONFile(outputPath, buildJSONData(diffs, clangVersion, cfg)); err != nil {
		capi.Fatal("无法写 JSON %s: %v", outputPath, err)
	}
}

func summaryOf(diffs []diffInfo) map[string]int {
	// 与 Python 版同构：5 个键恒出现（含 0），下游健康度看板依赖固定结构
	s := map[string]int{
		"total":       len(diffs),
		"match":       0,
		"compile_gap": 0,
		"runtime_gap": 0,
		"output_gap":  0,
	}
	for _, d := range diffs {
		if _, tracked := s[d.diff]; tracked { // known_issue / cide_better 不入 summary（与 Python 同）
			s[d.diff]++
		}
	}
	return s
}

func categoryFrequencyOf(diffs []diffInfo) map[string]int {
	freq := map[string]int{}
	for _, d := range diffs {
		if d.diff == "compile_gap" {
			cat := classifyCompileError(d.cide.CompileError, d.cs.category)
			freq[cat]++
		}
	}
	return freq
}

func detailsOf(diffs []diffInfo) []detailEntry {
	out := make([]detailEntry, 0, len(diffs))
	for _, d := range diffs {
		errText := ""
		if !d.cide.CompileSuccess {
			errText = capi.TruncateRunes(d.cide.CompileError, 500)
		}
		out = append(out, detailEntry{
			Case:             d.cs.name,
			Expected:         d.cs.category,
			DiffType:         d.diff,
			CideCompileError: errText,
		})
	}
	return out
}

// writeJSONFile ensure_ascii=False 语义（不转义非 ASCII，也不转义 <、>、&）。
func writeJSONFile(path string, v any) error {
	if err := os.MkdirAll(filepath.Dir(path), 0o755); err != nil {
		return err
	}
	f, err := os.Create(path)
	if err != nil {
		return err
	}
	enc := json.NewEncoder(f)
	enc.SetEscapeHTML(false)
	enc.SetIndent("", "  ")
	if err := enc.Encode(v); err != nil {
		f.Close()
		return err
	}
	return f.Close()
}

// writeKrLeetCodeReport 生成 K&R + LeetCode 专项报告（CI artifact）。
func writeKrLeetCodeReport(diffs []diffInfo, outputPath string) {
	report := map[string]any{
		"timestamp": time.Now().Format("2006-01-02 15:04:05"),
		"summary":   map[string]any{},
		"gaps":      []any{},
	}
	summary := report["summary"].(map[string]any)
	var krDiffs []diffInfo
	for _, d := range diffs {
		if d.cs.srcDir == "knr" || d.cs.srcDir == "leetcode" {
			krDiffs = append(krDiffs, d)
		}
	}
	for _, src := range []string{"knr", "leetcode"} {
		var srcDiffs []diffInfo
		for _, d := range krDiffs {
			if d.cs.srcDir == src {
				srcDiffs = append(srcDiffs, d)
			}
		}
		summary[src] = summaryOf(srcDiffs)
	}
	gaps := report["gaps"].([]any)
	for _, d := range krDiffs {
		if d.diff == "match" {
			continue
		}
		errText := ""
		if !d.cide.CompileSuccess {
			errText = capi.TruncateRunes(d.cide.CompileError, 500)
		}
		gaps = append(gaps, map[string]any{
			"case":               d.cs.name,
			"src_dir":            d.cs.srcDir,
			"diff_type":          d.diff,
			"expected_category":  d.cs.category,
			"cide_compile_error": errText,
			"clang_stdout":       capi.TruncateRunes(strings.TrimSpace(d.clang.Stdout), 200),
			"cide_stdout":        capi.TruncateRunes(strings.TrimSpace(d.cide.Stdout), 200),
		})
	}
	report["gaps"] = gaps
	if err := writeJSONFile(outputPath, report); err != nil {
		capi.Fatal("无法写专项报告 %s: %v", outputPath, err)
	}
}

// ---------------------------------------------------------------- 主流程

// verifyClangAvailable E-P0-4：Clang 预检。
// 缺失时 fail fast（exit 2，区别于测试失败的 exit 1），防止所有用例的
// run_with_clang 抛异常后被 analyzeDiff 尾分支静默归类为 cide_better，
// 防线 1 退化为"全量通过"。返回版本串写入报告供审计。
func verifyClangAvailable() string {
	ctx, cancel := context.WithTimeout(context.Background(), 30*time.Second)
	defer cancel()
	cmd := exec.CommandContext(ctx, clangPath, "--version")
	var out, errBuf bytes.Buffer
	cmd.Stdout = &out
	cmd.Stderr = &errBuf
	if err := cmd.Run(); err != nil {
		fmt.Printf("错误: 无法执行 Clang (%s): %v\n", clangPath, err)
		fmt.Println("防线 1 依赖 Clang 生成 Golden，Clang 缺失时结果不可信。")
		fmt.Println("请安装 LLVM/Clang 并确认其在 PATH 中（CI 侧请检查 runner 镜像变更）。")
		os.Exit(2)
	}
	text := strings.TrimSpace(out.String())
	if text == "" {
		return "unknown"
	}
	version := strings.Split(text, "\n")[0]
	fmt.Printf("Clang 预检通过: %s\n", version)
	return version
}

func resolveJobs(jobs int) int {
	if jobs > 0 {
		return jobs
	}
	// 自动并发上限 16：D5 第一站实测 clang 16 路最优（CI runner 核数少时
	// 由 NumCPU 兜底，不影响 4 核 runner）。
	n := runtime.NumCPU()
	if n < 1 {
		n = 1
	}
	if n > 16 {
		n = 16
	}
	return n
}

func main() {
	reportFlag := flag.String("report", "", "指定 Markdown 报告输出路径；默认生成带时间戳的文件")
	jsonFlag := flag.String("json", "", "指定 JSON 数据输出路径；默认生成带时间戳的文件")
	jobsFlag := flag.Int("jobs", 0, "Clang 并发数；0=自动（min(CPU 核数, 16)），1=串行")
	refreshFlag := flag.Bool("refresh-clang", false, "忽略 Clang 结果缓存并全量重算（同时覆盖写回；CI 夜间防版本漂移用）")
	rebuildFlag := flag.Bool("rebuild", false, "检测到 release DLL 比引擎源码旧时，先执行 cargo build --release")
	limitFlag := flag.Int("limit", 0, "仅执行前 N 个用例（本地调试提速用；正式门禁/CI 请勿使用，且不更新 latest 报告）")
	flag.Usage = func() {
		fmt.Fprintf(os.Stderr, "Cide 影子验证框架（Go 版，D5 最后一站）\n用法: go run scripts/shadow_verify.go [选项]\n选项:\n")
		flag.PrintDefaults()
	}
	flag.Parse()

	bar60 := strings.Repeat("=", 60)
	fmt.Println(bar60)
	fmt.Println("Cide 影子验证框架")
	fmt.Println(bar60)

	// E-P0-4：Clang 预检，缺失 fail fast（防止全量用例被静默归类 cide_better）
	clangVersion := verifyClangAvailable()

	if _, err := os.Stat(dllPath); err != nil {
		fmt.Printf("错误: 找不到 Cide DLL: %s\n", dllPath)
		fmt.Println("请先运行: cd native && cargo build --release")
		os.Exit(1)
	}

	// release DLL 陈旧检测：Shadow 用 release DLL，而日常构建多为 debug——
	// 改完引擎不重建就会拿旧引擎跑门禁（2026-09-11 实际踩过）。
	staleInfo, stale := dllStaleness()
	if stale && *rebuildFlag {
		rebuildReleaseDLL()
		staleInfo, stale = dllStaleness()
	}
	if stale {
		dllFi, _ := os.Stat(dllPath)
		warnStaleDLL(dllFi.ModTime(), staleInfo.mtime, staleInfo.path)
	}

	// 产物新鲜度门禁（fail fast，exit 2）：版本串必须含当前 HEAD。
	// 与 mtime 陈旧检测互补：mtime 发现不了"提交推进了没重建"。必须在主进程校验。
	dll = capi.Load(dllPath)

	jobs := resolveJobs(*jobsFlag)
	cacheMode := "启用"
	if *refreshFlag {
		cacheMode = "强制重算（--refresh-clang）"
	}
	mode := "并行"
	if jobs <= 1 {
		mode = "串行"
	}
	fmt.Printf("配置: jobs=%d（%s），Clang 缓存=%s\n", jobs, mode, cacheMode)

	cases := loadCaseFiles()
	debugLimit := *limitFlag > 0
	if debugLimit {
		if *limitFlag < len(cases) {
			cases = cases[:*limitFlag]
		}
		fmt.Printf("⚠️  --limit %d：仅执行前 %d 个用例（调试模式，非完整门禁，不更新 latest）\n", *limitFlag, len(cases))
	}

	started := time.Now()
	outcomes := executeCases(cases, jobs, *refreshFlag, clangVersion)
	elapsed := time.Since(started).Seconds()

	var diffs []diffInfo
	cacheHits := 0
	for i, oc := range outcomes {
		if oc.clang.Cached {
			cacheHits++
		}
		fmt.Printf("\n[%d/%d] %s (%s)\n", i+1, len(cases), oc.cs.name, oc.cs.category)
		cacheTag := ""
		if oc.clang.Cached {
			cacheTag = " (clang 缓存)"
		}
		fmt.Printf("  Clang: compile=%s, run=%s%s\n", okOf(oc.clang.CompileSuccess), okOf(oc.clang.RunSuccess), cacheTag)
		fmt.Printf("  Cide:  compile=%s, run=%s\n", okOf(oc.cide.CompileSuccess), okOf(oc.cide.RunSuccess))

		diffType := analyzeDiff(oc.cs, oc.clang, oc.cide)
		diffs = append(diffs, diffInfo{cs: oc.cs, diff: diffType, clang: oc.clang, cide: oc.cide})

		switch diffType {
		case "compile_gap":
			cat := classifyCompileError(oc.cide.CompileError, oc.cs.category)
			fmt.Printf("  => 编译缺口 [%s]\n", cat)
		case "match":
			fmt.Println("  => 匹配 ✓")
		default:
			fmt.Printf("  => %s\n", diffType)
		}
	}

	cfg := runConfig{
		jobs:       jobs,
		elapsedSec: math.Round(elapsed*10) / 10,
		refresh:    *refreshFlag,
		hits:       cacheHits,
		misses:     len(outcomes) - cacheHits,
	}
	fmt.Printf("\n执行完成: %d 用例，耗时 %.1fs，Clang 缓存命中 %d/%d\n",
		len(outcomes), elapsed, cacheHits, len(outcomes))
	// 清理运行目录（失败不致命：残留目录会在下次槽位初始化重建）
	os.RemoveAll(runRoot)

	// 生成报告
	reportPath := *reportFlag
	if reportPath == "" {
		reportPath = filepath.Join(reportsDir, fmt.Sprintf("shadow_report_%s.md", time.Now().Format("20060102_150405")))
	}
	generateReport(diffs, reportPath, &cfg)

	// 同步更新 latest 文件，便于健康度看板等工具读取
	// （--limit 调试运行是残缺样本，不覆盖 latest，避免污染看板）
	if !debugLimit {
		generateReport(diffs, filepath.Join(reportsDir, "shadow_report_latest.md"), &cfg)
	}

	// 同时输出 JSON
	jsonPath := *jsonFlag
	if jsonPath == "" {
		jsonPath = filepath.Join(reportsDir, fmt.Sprintf("shadow_data_%s.json", time.Now().Format("20060102_150405")))
	}
	writeJSONData(diffs, jsonPath, clangVersion, &cfg)
	fmt.Printf("\nJSON 数据已保存: %s\n", jsonPath)

	// 同步更新 latest JSON（与带时间戳版本同一份数据，与 Python 版行为一致）
	if !debugLimit {
		if err := writeJSONFile(filepath.Join(reportsDir, "shadow_data_latest.json"),
			buildJSONData(diffs, clangVersion, &cfg)); err != nil {
			capi.Fatal("无法写 latest JSON: %v", err)
		}
	}

	// 生成 K&R + LeetCode 专项报告
	krPath := filepath.Join(reportsDir, "kr_leetcode_report.json")
	if debugLimit {
		// --limit 样本残缺，不覆盖 CI artifact 使用的专项报告
		fmt.Println("（--limit 调试运行：跳过 kr_leetcode_report.json 更新）")
	} else {
		writeKrLeetCodeReport(diffs, krPath)
		fmt.Printf("K&R + LeetCode 专项报告已保存: %s\n", krPath)
	}

	// E-P0-1：门禁退出码。此前 main() 无任何非零退出路径，防线 1 在 CI 中只是
	// "出报告的观测工具"，任何回归恒绿。
	//
	// 判定规则：
	//   - 非预期差异（compile_gap / runtime_gap / output_gap）→ exit 1
	//     （Clang 能跑而 Cide 不能，或输出不一致 = 回归）
	//   - match / known_issue（category 含 "bug" 的已记录问题）/ cide_better
	//     （Cide 教学扩展比 Clang 宽松）→ 不视为失败，与 AGENTS.md 统计口径一致
	var unexpected []diffInfo
	for _, d := range diffs {
		if d.diff == "compile_gap" || d.diff == "runtime_gap" || d.diff == "output_gap" {
			unexpected = append(unexpected, d)
		}
	}
	fmt.Println()
	fmt.Println(bar60)
	fmt.Println("Shadow 门禁汇总")
	fmt.Println(bar60)
	fmt.Printf("总用例: %d\n", len(diffs))
	for _, dt := range []string{"match", "known_issue", "cide_better", "compile_gap", "runtime_gap", "output_gap"} {
		n := 0
		for _, d := range diffs {
			if d.diff == dt {
				n++
			}
		}
		if n > 0 {
			fmt.Printf("  %s: %d\n", dt, n)
		}
	}
	if len(unexpected) > 0 {
		fmt.Printf("\n非预期差异 %d 例（需要调查，CI 将失败）：\n", len(unexpected))
		for _, d := range unexpected {
			fmt.Printf("  - %s: %s (category=%s)\n", d.cs.name, d.diff, d.cs.category)
		}
		os.Exit(1)
	}
	fmt.Println("\n无非预期差异，门禁通过。")
}

func okOf(b bool) string {
	if b {
		return "OK"
	}
	return "FAIL"
}

// dll 全局 DLL 实例（capi.Load 后供 executeCases 使用）。
var dll *capi.DLL
