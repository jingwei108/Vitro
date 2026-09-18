package main

// 预编译 runtime_libc 为嵌入产物。
//
// 开发流程：
//  1. 修改 native/runtime_libc/src/*.c
//  2. go run ./scripts/precompile_bytecode_libc
//  3. git add native/crates/vitro_vm/src/bytecode_libc_data.json
//     native/crates/vitro_runtime/src/bytecode_libc_index.rs
//  4. git commit
//
// CI 检查：go run ./scripts/precompile_bytecode_libc --check
//
// `--check` 以 `source_digest`（源文件内容 SHA-256）判定产物是否与
// native/runtime_libc/{src,vitro}/ 同步，**不依赖文件 mtime**——CI 干净
// 检出时 git 不保留 mtime 且按路径顺序写文件，mtime 判定必然误报
// （2026-09-11 修复，Python 版历史事故记载）。
//
// D5 后续批次第二站（2026-09-18）：precompile_bytecode_libc.py → Go。
// 双轨对账口径（切换 CI 前提）：digest 逐字节一致（CRLF→LF 归一 + 路径
// 排序 + schema 前缀）、重生成产物与 Python 版逐字节一致（JSON sort_keys
// 递归键序 = Go map marshal 键序；数字以 json.Number 原样保真；HTML 转义
// 关闭对齐 ensure_ascii=False）、--check 双侧同判定。J9 证红：篡改任一
// runtime_libc 源文件 → --check 必红（digest 失配 / exit 1）。
//
// 事故档案（fd0c1cf）：产物曾在更名提交中被文本替换而非重生成，冻结的
// source_digest 使 --check 必红——本脚本重新生成产物时必须真实调用
// vitro_cli export，禁止任何"就地文本替换"捷径。

import (
	"bytes"
	"crypto/sha256"
	"encoding/hex"
	"encoding/json"
	"flag"
	"fmt"
	"os"
	"os/exec"
	"path/filepath"
	"sort"
	"strconv"
	"strings"

	"vitro/scripts/internal/capi"
)

var (
	root       = capi.ProjectRoot()
	nativeDir  = filepath.Join(root, "native")
	srcDirs    = []string{filepath.Join(nativeDir, "runtime_libc", "src"), filepath.Join(nativeDir, "runtime_libc", "vitro")}
	outputJSON = filepath.Join(nativeDir, "crates", "vitro_vm", "src", "bytecode_libc_data.json")
	outputRS   = filepath.Join(nativeDir, "crates", "vitro_runtime", "src", "bytecode_libc_index.rs")
	layoutJSON = filepath.Join(nativeDir, "crates", "vitro_cpp_frontend", "src", "builtin_layout_data.json")
	vitroDir   = filepath.Join(nativeDir, "runtime_libc", "vitro")
)

// digestSchema 摘要 schema 版本：源文件集合或摘要算法变化时递增，
// 使旧产物无需比较内容即可判定为过期。
const digestSchema = 1

// sourceFiles 返回 runtime_libc 下参与预编译的源文件（按完整路径排序）。
// includeHeaders=true 用于内容摘要（头文件变化同样需要重新生成）；
// 传给 vitro_cli export 时为 false（只传 .c/.cpp）。
func sourceFiles(includeHeaders bool) []string {
	exts := map[string]bool{".c": true, ".cpp": true}
	if includeHeaders {
		exts[".h"] = true
	}
	var files []string
	for _, dir := range srcDirs {
		ents, err := os.ReadDir(dir)
		if err != nil {
			continue
		}
		for _, e := range ents {
			if e.IsDir() {
				continue
			}
			if exts[filepath.Ext(e.Name())] {
				files = append(files, filepath.Join(dir, e.Name()))
			}
		}
	}
	sort.Strings(files)
	return files
}

// computeSourceDigest 计算源文件内容摘要（SHA-256）。
// 只取决于源文件的相对路径与内容，与 mtime 无关——CI 干净检出与本地
// 增量开发下判定一致。行尾归一化（CRLF→LF）：同一份逻辑内容不应因
// core.autocrlf / 平台差异被判成"产物过期"。
func computeSourceDigest() string {
	h := sha256.New()
	fmt.Fprintf(h, "schema=%d\n", digestSchema)
	for _, path := range sourceFiles(true) {
		rel, _ := filepath.Rel(root, path)
		h.Write([]byte(filepath.ToSlash(rel)))
		h.Write([]byte{0})
		data, err := os.ReadFile(path)
		if err != nil {
			capi.Fatal("读源文件失败 %s: %v", path, err)
		}
		h.Write(bytes.ReplaceAll(data, []byte("\r\n"), []byte("\n")))
		h.Write([]byte{0})
	}
	return "sha256:" + hex.EncodeToString(h.Sum(nil))
}

func findVitroCLI() string {
	for _, rel := range []string{
		filepath.Join("target", "release", "vitro_cli.exe"),
		filepath.Join("target", "release", "vitro_cli"),
		filepath.Join("target", "debug", "vitro_cli.exe"),
		filepath.Join("target", "debug", "vitro_cli"),
	} {
		p := filepath.Join(nativeDir, rel)
		if _, err := os.Stat(p); err == nil {
			return p
		}
	}
	return ""
}

func buildVitroCLI() string {
	fmt.Println("Building vitro_cli...")
	cmd := exec.Command("cargo", "build", "--release", "--bin", "vitro_cli")
	cmd.Dir = nativeDir
	cmd.Stdout, cmd.Stderr = os.Stdout, os.Stderr
	if err := cmd.Run(); err != nil {
		capi.Fatal("cargo build 失败: %v", err)
	}
	exe := findVitroCLI()
	if exe == "" {
		capi.Fatal("vitro_cli build succeeded but executable not found")
	}
	fmt.Printf("  -> %s\n", exe)
	return exe
}

// precompile 调用 vitro_cli export 预编译 runtime_libc，随后做固定索引重定位。
func precompile(exe string) map[string]any {
	// Stage 2b: .cpp files now contain full C++ implementations,
	// and legacy .c container implementations are being removed.
	sourcePaths := sourceFiles(false)

	fmt.Printf("Precompiling %d files:\n", len(sourcePaths))
	for _, p := range sourcePaths {
		fmt.Printf("  - %s\n", filepath.Base(p))
	}

	args := append([]string{exe, "export"}, sourcePaths...)
	args = append(args, "--builtin-libc", "-o", outputJSON)
	cmd := exec.Command(args[0], args[1:]...)
	cmd.Stdout, cmd.Stderr = os.Stdout, os.Stderr
	if err := cmd.Run(); err != nil {
		capi.Fatal("vitro_cli export 失败: %v", err)
	}

	raw, err := os.ReadFile(outputJSON)
	if err != nil {
		capi.Fatal("读产物失败: %v", err)
	}
	// UseNumber：数字以原文保真（避免 float 精度重排），重写出零漂移。
	dec := json.NewDecoder(bytes.NewReader(raw))
	dec.UseNumber()
	var data map[string]any
	if err := dec.Decode(&data); err != nil {
		capi.Fatal("产物不是合法 JSON: %v", err)
	}

	// 将 code 中的 Call/CallPtr operand 从原始索引重定位为固定索引，
	// 避免 setup_vm 同时注册原始索引和固定索引时发生冲突。
	rawFuncIndex := map[string]json.Number{}
	for k, v := range asObj(data["func_index"]) {
		if n, ok := v.(json.Number); ok {
			rawFuncIndex[k] = n
		}
	}
	type pair struct {
		name string
		raw  int64
	}
	var sortedRaw []pair
	for name, n := range rawFuncIndex {
		v, err := n.Int64()
		if err != nil {
			capi.Fatal("func_index 值不是整数: %s=%s", name, n.String())
		}
		sortedRaw = append(sortedRaw, pair{name, v})
	}
	sort.Slice(sortedRaw, func(i, j int) bool { return sortedRaw[i].raw < sortedRaw[j].raw })
	rawToFixed := map[int64]int64{}
	for i, p := range sortedRaw {
		rawToFixed[p.raw] = 1000 + int64(i)
	}

	codeArr, _ := data["code"].([]any)
	for _, inst := range codeArr {
		m := asObj(inst)
		switch strOf(m["op"]) {
		case "Call", "CallPtr":
			if n, ok := m["operand"].(json.Number); ok {
				if v, err := n.Int64(); err == nil {
					if fixed, ok := rawToFixed[v]; ok {
						m["operand"] = json.Number(strconv.FormatInt(fixed, 10))
					}
				}
			}
		}
	}

	fixedIndex := map[string]any{}
	for name, n := range rawFuncIndex {
		v, _ := n.Int64()
		fixedIndex[name] = json.Number(strconv.FormatInt(rawToFixed[v], 10))
	}
	data["func_index"] = fixedIndex

	// 记录源文件内容摘要：供 --check 在 CI 干净检出下做稳定判定（与 mtime 无关）
	data["source_digest"] = computeSourceDigest()

	validatePrecompiled(data)

	fmt.Printf("  code_len: %v\n", asObj(data)["code_len"])
	fmt.Printf("  func_count: %d\n", len(fixedIndex))
	fmt.Printf("  globals_size: %v\n", asObj(data)["globals_size"])
	return data
}

func asObj(v any) map[string]any {
	m, _ := v.(map[string]any)
	return m
}

func strOf(v any) string {
	s, _ := v.(string)
	return s
}

// validatePrecompiled 校验预编译产物符合 Stage 2b 要求：
// 1. runtime_libc/vitro/ 下不应存在旧 .c 实现。
// 2. builtin_layout 中注册的方法必须在产物中可用。
// 3. 不应残留旧 C 风格函数名（如 vitro_vec_init_int）。
func validatePrecompiled(data map[string]any) {
	var legacyC []string
	if ents, err := os.ReadDir(vitroDir); err == nil {
		for _, e := range ents {
			if !e.IsDir() && strings.HasSuffix(e.Name(), ".c") {
				legacyC = append(legacyC, e.Name())
			}
		}
	}
	if len(legacyC) > 0 {
		capi.Fatal("发现遗留 C 容器实现: %v. Stage 2b 要求 runtime_libc/vitro/ 只保留 .cpp 实现.", legacyC)
	}

	rawLayout, err := os.ReadFile(layoutJSON)
	if err != nil {
		capi.Fatal("布局文件不存在: %s", layoutJSON)
	}
	var layout struct {
		Classes   map[string]map[string]any `json:"classes"`
		MethodMap map[string]map[string]any `json:"method_map"`
	}
	if err := json.Unmarshal(rawLayout, &layout); err != nil {
		capi.Fatal("布局 JSON 解析失败: %v", err)
	}

	funcIndex := asObj(data["func_index"])
	var missing []string
	for vitroName := range layout.Classes {
		for method, mangled := range layout.MethodMap[vitroName] {
			if _, ok := funcIndex[strOf(mangled)]; !ok {
				missing = append(missing, fmt.Sprintf("%s.%s -> %s", vitroName, method, strOf(mangled)))
			}
		}
	}
	if len(missing) > 0 {
		sort.Strings(missing)
		capi.Fatal("以下内置容器方法未在预编译产物中找到:\n  %s", strings.Join(missing, "\n  "))
	}

	// 旧 C 风格函数名黑名单（Stage 2b 之前的命名）
	oldPrefixes := []string{
		"vitro_vec_init_", "vitro_vec_push_", "vitro_vec_pop_", "vitro_vec_get_",
		"vitro_vec_size_", "vitro_vec_destroy_", "vitro_vec_clear_",
		"vitro_list_init_", "vitro_list_push_", "vitro_list_pop_", "vitro_list_get_",
		"vitro_list_size_", "vitro_list_destroy_", "vitro_list_clear_",
		"vitro_string_init", "vitro_string_push_", "vitro_string_pop_",
		"vitro_string_get_", "vitro_string_size", "vitro_string_destroy",
		"vitro_string_clear", "vitro_string_c_str",
	}
	var stale []string
	for name := range funcIndex {
		for _, p := range oldPrefixes {
			if strings.HasPrefix(name, p) {
				stale = append(stale, name)
				break
			}
		}
	}
	if len(stale) > 0 {
		sort.Strings(stale)
		capi.Fatal("预编译产物中残留旧 C 风格函数名:\n  %s", strings.Join(stale, "\n  "))
	}

	fmt.Println("  validation: ok")
}

// generateIndexRS 生成 bytecode_libc_index.rs（与 Python 版逐行同构；
// 头注指向现役 Go 命令）。
func generateIndexRS(data map[string]any) string {
	funcIndex := asObj(data["func_index"])
	baseIndex := int64(1000)
	globalsReserved := int64(1024)
	if len(funcIndex) > 0 {
		baseIndex = int64(1) << 62
		for _, v := range funcIndex {
			if n, ok := v.(json.Number); ok {
				if i, err := n.Int64(); err == nil && i < baseIndex {
					baseIndex = i
				}
			}
		}
	}
	if n, ok := asObj(data)["globals_size"].(json.Number); ok {
		if g, err := n.Int64(); err == nil {
			reserved := ((g + 1023) / 1024) * 1024
			if reserved > globalsReserved {
				globalsReserved = reserved
			}
		}
	}
	codeLen := strOfAny(asObj(data)["code_len"])
	funcCount := len(funcIndex)

	type nameIdx struct {
		name string
		idx  int64
	}
	var sortedFuncs []nameIdx
	for name, v := range funcIndex {
		if n, ok := v.(json.Number); ok {
			if i, err := n.Int64(); err == nil {
				sortedFuncs = append(sortedFuncs, nameIdx{name, i})
			}
		}
	}
	sort.Slice(sortedFuncs, func(i, j int) bool { return sortedFuncs[i].idx < sortedFuncs[j].idx })

	var b strings.Builder
	w := func(format string, args ...any) { fmt.Fprintf(&b, format+"\n", args...) }
	w("// AUTO-GENERATED by scripts/precompile_bytecode_libc (Go, 2026-09-18 迁移自 precompile_bytecode_libc.py)")
	w("// DO NOT EDIT MANUALLY")
	w("//")
	w("// To regenerate:")
	w("//   go run ./scripts/precompile_bytecode_libc")
	w("")
	w("pub const BYTECODE_LIBC_CODE_LEN: usize = %s;", codeLen)
	w("pub const BYTECODE_LIBC_BASE_INDEX: i32 = %d;", baseIndex)
	w("pub const BYTECODE_LIBC_GLOBALS_RESERVED: u32 = %d;", globalsReserved)
	w("pub const BYTECODE_LIBC_FUNC_COUNT: usize = %d;", funcCount)
	w("")
	w("/// Bytecode Libc 中所有可用的函数名（供索引查询使用）。")
	w("/// 注意：并非所有函数都默认走 Bytecode 路径，")
	w("/// 实际路径由 `host_func_id::BYTECODE_LIBC_PURE_FUNCS` 控制。")
	w("pub const BYTECODE_LIBC_ALL_FUNCS: &[&str] = &[")
	for _, p := range sortedFuncs {
		w("    \"%s\",", p.name)
	}
	w("];")
	w("")
	w("/// 将函数名解析为 Bytecode Libc 固定索引。")
	w("pub fn bytecode_libc_index(name: &str) -> Option<i32> {")
	w("    match name {")
	for _, p := range sortedFuncs {
		w("        \"%s\" => Some(%d),", p.name, p.idx)
	}
	w("        _ => None,")
	w("    }")
	w("}")
	w("")
	w("/// 判断函数是否在 Bytecode Libc 预编译产物中存在。")
	w("pub fn is_bytecode_libc(name: &str) -> bool {")
	w("    bytecode_libc_index(name).is_some()")
	w("}")
	w("")
	return b.String()
}

func strOfAny(v any) string {
	if n, ok := v.(json.Number); ok {
		return n.String()
	}
	return fmt.Sprintf("%v", v)
}

// writeOutputs 写入 JSON 与 Rust 常量文件。
// sort_keys：产物由 Rust 侧 HashMap 派生，其序列化键序随进程随机种子变化；
// 按键排序保证产物可重现（2026-09-11 定位）。Go 的 encoding/json 对 map
// marshal 天然按键序——与 Python sort_keys=True 等价；EscapeHTML 关闭对齐
// ensure_ascii=False（`<>&` 不转义）。写出为 CRLF 行尾（对齐 Python text
// mode 写出行为；git autocrlf 归一后入库 LF，两者 diff 一致）。
func writeOutputs(data map[string]any) {
	var buf bytes.Buffer
	enc := json.NewEncoder(&buf)
	enc.SetEscapeHTML(false)
	enc.SetIndent("", "  ")
	if err := enc.Encode(data); err != nil {
		capi.Fatal("JSON 序列化失败: %v", err)
	}
	// Python json.dump 不写尾换行；Encoder.Encode 会追加——剥掉后再做行尾展开。
	out := strings.TrimSuffix(buf.String(), "\n")
	if err := os.WriteFile(outputJSON, []byte(strings.ReplaceAll(out, "\n", "\r\n")), 0o644); err != nil {
		capi.Fatal("写产物失败: %v", err)
	}
	fmt.Printf("Written: %s\n", outputJSON)

	rs := generateIndexRS(data)
	if err := os.WriteFile(outputRS, []byte(strings.ReplaceAll(rs, "\n", "\r\n")), 0o644); err != nil {
		capi.Fatal("写产物失败: %v", err)
	}
	fmt.Printf("Written: %s\n", outputRS)
}

// checkUpToDate 检查预编译产物是否与 runtime_libc 源码同步（digest 判定）。
func checkUpToDate() bool {
	for _, path := range []string{outputJSON, outputRS} {
		if _, err := os.Stat(path); err != nil {
			rel, _ := filepath.Rel(root, path)
			fmt.Printf("  missing artifact: %s\n", filepath.ToSlash(rel))
			return false
		}
	}
	raw, err := os.ReadFile(outputJSON)
	if err != nil {
		fmt.Printf("  unreadable artifact: %v\n", err)
		return false
	}
	var probe map[string]any
	if err := json.Unmarshal(raw, &probe); err != nil {
		fmt.Printf("  unreadable artifact: %v\n", err)
		return false
	}
	recorded := strOf(probe["source_digest"])
	if recorded == "" {
		fmt.Println("  artifact has no source_digest field (generated by an older script) -> regenerate once")
		return false
	}
	current := computeSourceDigest()
	if recorded != current {
		fmt.Printf("  recorded digest: %s\n", recorded)
		fmt.Printf("  current  digest: %s\n", current)
		fmt.Printf("  source files: %d under native/runtime_libc/\n", len(sourceFiles(true)))
		return false
	}
	return true
}

func main() {
	check := flag.Bool("check", false, "检查预编译产物是否最新（CI 模式）")
	flag.Parse()

	if *check {
		if checkUpToDate() {
			fmt.Println("Precompiled artifacts are up-to-date.")
			return
		}
		fmt.Fprintln(os.Stderr, "ERROR: Precompiled artifacts are out-of-date. "+
			"Run: go run ./scripts/precompile_bytecode_libc")
		os.Exit(1)
	}

	// 清理旧产物，避免残留索引与旧函数名
	for _, f := range []string{outputJSON, outputRS} {
		if _, err := os.Stat(f); err == nil {
			if err := os.Remove(f); err != nil {
				capi.Fatal("清理旧产物失败: %v", err)
			}
			fmt.Printf("  removed: %s\n", f)
		}
	}

	exe := findVitroCLI()
	if exe == "" {
		exe = buildVitroCLI()
	}

	data := precompile(exe)
	writeOutputs(data)
	fmt.Println("Done.")
}
