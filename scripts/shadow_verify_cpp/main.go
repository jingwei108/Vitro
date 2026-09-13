//go:build windows

// shadow_verify_cpp —— C++ Shadow Verification 的 Go 迁移试点（D5，裁定文档 §13.5）。
//
// 与 Python 版（scripts/shadow_verify_cpp.py）的口径逐项对齐：
//   - 用例来源：内嵌 CPP_CASES + native/tests/cases/cpp/*.cpp（同名时目录版为准）；
//   - Clang 侧：.shadow_cpp_tmp/ 自管工作目录、-std=c++14、编译失败重试 3 次、
//     编译 30s / 运行 5s 超时；
//   - Vitro 侧：capi 直调 + E-P1-5 结构化输出通道（禁文本清洗）+ ABI/产物新鲜度 fail fast；
//   - 判定：clang_compile_fail / compile_gap / runtime_gap / output_gap / match，
//     category=="gap" 为预期差异，非预期差异 > 0 时退出码 1；
//   - 报告：native/tests/shadow_verification/reports/cpp_shadow_report.json（字段同名）。
//
// 与 Python 版的差异（有意的，均记录在案）：
//   - Clang 侧完全并发（16 workers，§13.1 实测 78 个 clang++ 编译 6.0x 加速）；
//     Vitro 侧串行——引擎 DLL 的会话级线程安全性未验证，试点不做此假设
//     （clang++ 子进程是独立进程，天然并发安全，且是耗时大头）；
//   - 启动自检（selfCheck）：对 compare 注入必然违反的输入，断言必须变红，
//     否则退出码 2 拒绝跑（J9：判定型脚本必须有"注入 → 必红"证据）；
//   - Go 字符串原生 UTF-8，无编码样板；os/exec 出 []byte，无隐式编码转换。
//
// 用法：go run ./scripts/shadow_verify_cpp
package main

import (
	"vitro/scripts/internal/capi"

	"bytes"
	"context"
	"encoding/json"
	"fmt"
	"os"
	"os/exec"
	"path/filepath"
	"runtime"
	"sort"
	"strings"
	"sync"
	"time"
	"unsafe"
)

// ---------------------------------------------------------------- 路径与常量

var (
	projectRoot = capi.ProjectRoot()
	nativeDir   = filepath.Join(projectRoot, "native")
	dllPath     = filepath.Join(nativeDir, "target", "release", "vitro_native.dll")
	tmpDir      = filepath.Join(projectRoot, ".shadow_cpp_tmp")
	reportPath  = filepath.Join(nativeDir, "tests", "shadow_verification", "reports", "cpp_shadow_report.json")
	casesDir    = filepath.Join(nativeDir, "tests", "cases", "cpp")

	clangPath  = "clang++"
	workerN    = 16 // §13.1：78 个 clang++ 编译 jobs=16 实测最优（6.0x）
	clangRetry = 3  // CI runner 上 clang 偶发瞬时失败（2026-09-12 实测）
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
}

type shadowCase struct {
	name     string
	source   string
	category string
}

type shadowDiff struct {
	CaseName         string    `json:"case_name"`
	ExpectedCategory string    `json:"expected_category"`
	ClangResult      runResult `json:"clang_result"`
	VitroResult       runResult `json:"vitro_result"`
	DiffType         string    `json:"diff_type"`
}

// ---------------------------------------------------------------- 内嵌用例

// 与 Python 版逐字节一致（内嵌副本不参与维护——目录版才是真相，同名时目录版胜出）。
var cppCases = []shadowCase{
	{"cpp_class_field", `
#include <stdio.h>
class Point {
public:
    int x;
    Point() { x = 0; }
};
int main() {
    Point p;
    p.x = 42;
    printf("%d\n", p.x);
    return 0;
}
`, "baseline"},
	{"cpp_new_delete", `
#include <stdio.h>
class Box {
public:
    int v;
    Box() { v = 0; }
};
int main() {
    Box* b = new Box();
    b->v = 7;
    printf("%d\n", b->v);
    delete b;
    return 0;
}
`, "baseline"},
	{"cpp_virtual_call", `
#include <stdio.h>
class Base {
public:
    virtual int foo() { return 1; }
};
class Derived : public Base {
public:
    int foo() { return 2; }
};
int main() {
    Base* b = new Derived();
    printf("%d\n", b->foo());
    delete b;
    return 0;
}
`, "baseline"},
	{"cpp_template_class", `
#include <stdio.h>
template<class T>
class Box {
public:
    T v;
    Box() { v = 0; }
};
int main() {
    Box<int> b;
    b.v = 99;
    printf("%d\n", b.v);
    return 0;
}
`, "baseline"},
	{"cpp_lambda_capture", `
#include <stdio.h>
int main() {
    int x = 5;
    auto f = [x](int y) { return x + y; };
    printf("%d\n", f(3));
    return 0;
}
`, "baseline"},
	{"cpp_reference_param", `
#include <stdio.h>
void inc(int& x) { x = x + 1; }
int main() {
    int a = 5;
    inc(a);
    printf("%d\n", a);
    return 0;
}
`, "baseline"},
	{"cpp_range_for", `
#include <stdio.h>
int main() {
    int arr[] = {1, 2, 3};
    int sum = 0;
    for (int x : arr) sum = sum + x;
    printf("%d\n", sum);
    return 0;
}
`, "baseline"},
	{"cpp_raii_dtor", `
#include <stdio.h>
int g = 0;
class A {
public:
    int id;
    A() { id = 0; }
    void init(int i) { id = i; }
    ~A() { g = g * 10 + id; }
};
void foo() {
    A a;
    a.init(1);
}
int main() {
    foo();
    printf("%d\n", g);
    return 0;
}
`, "baseline"},
	{"cpp_new_array", `
#include <stdio.h>
int g = 0;
class A {
public:
    A() { g++; }
    ~A() { g--; }
};
int main() {
    A* arr = new A[3];
    printf("%d\n", g);
    delete[] arr;
    printf("%d\n", g);
    return 0;
}
`, "baseline"},
	{"cpp_nested_class_new", `
#include <stdio.h>
template<class T>
class list {
    struct Node {
        T data;
        Node* next;
    };
    Node* head;
public:
    list() : head((Node*)0) {}
    void push(T x) {
        Node* n = new Node;
        n->data = x;
        n->next = head;
        head = n;
    }
    T get(int i) {
        Node* p = head;
        while (i-- > 0 && p != (Node*)0) p = p->next;
        if (p == (Node*)0) return 0;
        return p->data;
    }
    ~list() {
        Node* p = head;
        while (p != (Node*)0) {
            Node* n = p->next;
            delete p;
            p = n;
        }
    }
};
int main() {
    list<int> l;
    l.push(10);
    l.push(20);
    printf("%d\n", l.get(0));
    printf("%d\n", l.get(1));
    return 0;
}
`, "baseline"},
	{"cpp_ctor_overload", `
#include <stdio.h>
class Box {
public:
    int x;
    Box() { x = 0; }
    Box(int v) { x = v; }
};
int main() {
    Box* a = new Box();
    Box* b = new Box(42);
    printf("%d %d\n", a->x, b->x);
    delete a;
    delete b;
    return 0;
}
`, "gap"},
	{"cpp_rvalue_ref", `
#include <stdio.h>
int foo() { return 42; }
int main() {
    int&& r = foo();
    printf("%d\n", r);
    return 0;
}
`, "baseline"},
	{"cpp_const_ref_rvalue", `
#include <stdio.h>
int main() {
    const int& r = 5;
    printf("%d\n", r);
    return 0;
}
`, "baseline"},
	{"cpp_auto_ref", `
#include <stdio.h>
int main() {
    int x = 10;
    auto& r = x;
    r = r + 1;
    printf("%d\n", x);
    return 0;
}
`, "baseline"},
	{"cpp_lambda_multi_capture", `
#include <stdio.h>
int main() {
    int a = 1, b = 2;
    auto f = [a, &b](int x) { return x + a + b; };
    b = 5;
    printf("%d\n", f(10));
    return 0;
}
`, "baseline"},
	{"cpp_lambda_ref_capture", `
#include <stdio.h>
int main() {
    int x = 5;
    auto f = [&x]() { x = x + 1; };
    f();
    printf("%d\n", x);
    return 0;
}
`, "baseline"},
	{"cpp_member_out_of_line", `
#include <stdio.h>
class Bar {
public:
    int x;
    void set(int v);
};
void Bar::set(int v) { x = v; }
int main() {
    Bar b;
    b.set(7);
    printf("%d\n", b.x);
    return 0;
}
`, "gap"},
	{"cpp_auto_new_int", `
#include <stdio.h>
int main() {
    auto p = new int(99);
    printf("%d\n", *p);
    delete p;
    return 0;
}
`, "baseline"},
	{"cpp_range_for_ref_modify", `
#include <stdio.h>
int main() {
    int arr[] = {1, 2, 3};
    for (auto& x : arr) x = x * 2;
    printf("%d %d %d\n", arr[0], arr[1], arr[2]);
    return 0;
}
`, "baseline"},
	{"cpp_template_struct", `
#include <stdio.h>
template<class T> struct Pair { T a, b; };
int main() {
    Pair<int> p;
    p.a = 1;
    p.b = 2;
    printf("%d %d\n", p.a, p.b);
    return 0;
}
`, "gap"},
	{"cpp_struct_tag_alias", `
#include <stdio.h>
struct Node { int x; };
int main() {
    Node n;
    n.x = 42;
    printf("%d\n", n.x);
    return 0;
}
`, "baseline"},
	{"cpp_new_int_array", `
#include <stdio.h>
int main() {
    int* p = new int[3];
    p[0] = 1;
    p[1] = 2;
    p[2] = 3;
    printf("%d %d %d\n", p[0], p[1], p[2]);
    delete[] p;
    return 0;
}
`, "baseline"},
}

// ---------------------------------------------------------------- 启动自检（J9）

// selfCheck 对 compareResults 注入必然违反的输入，断言判定必须变红。
// 自检不过 → 退出码 2 拒绝运行：判定函数坏了的"全绿"比没有门禁更坏。
func selfCheck() {
	clangOK := runResult{Compiler: "clang++", CompileSuccess: true, RunSuccess: true, Stdout: "1\n2\n", ExitCode: 0}
	vitroOK := runResult{Compiler: "vitro", CompileSuccess: true, RunSuccess: true, Stdout: "1\n2\n", ExitCode: 0}
	checks := []struct {
		name  string
		clang runResult
		vitro  runResult
		want  string
	}{
		{"两侧一致 → match", clangOK, vitroOK, "match"},
		{"clang 编译失败 → clang_compile_fail", runResult{Compiler: "clang++", CompileSuccess: false}, vitroOK, "clang_compile_fail"},
		{"vitro 编译失败 → compile_gap", clangOK, runResult{Compiler: "vitro", CompileSuccess: false}, "compile_gap"},
		{"一侧运行失败 → runtime_gap", clangOK, runResult{Compiler: "vitro", CompileSuccess: true, RunSuccess: false, RunError: "trap"}, "runtime_gap"},
		{"输出不同 → output_gap", clangOK, runResult{Compiler: "vitro", CompileSuccess: true, RunSuccess: true, Stdout: "1\n3\n"}, "output_gap"},
		// 语义雷区：这三条若变红，说明 strip / CRLF 归一口径被破坏——
		// Python 版靠 .strip() + \r\n→\n 保持 match，Go 版必须同语义。
		{"尾部空白差异仍 → match", clangOK, runResult{Compiler: "vitro", CompileSuccess: true, RunSuccess: true, Stdout: "1\n2\n   \n"}, "match"},
		{"CRLF/LF 差异仍 → match", clangOK, runResult{Compiler: "vitro", CompileSuccess: true, RunSuccess: true, Stdout: "1\r\n2\r\n"}, "match"},
	}
	for _, c := range checks {
		if got := compareResults(c.clang, c.vitro); got != c.want {
			capi.Fatal("启动自检失败：%s：期望 %s，实际 %s（判定口径已破坏，拒绝运行）", c.name, c.want, got)
		}
	}
	fmt.Printf("启动自检：compare 口径 %d 条断言全部通过\n", len(checks))
}

// ---------------------------------------------------------------- Clang 侧

func runWithClang(name, source string) runResult {
	var result runResult
	for attempt := 0; attempt < clangRetry; attempt++ {
		result = runWithClangOnce(name, source)
		if result.CompileSuccess {
			return result
		}
		time.Sleep(time.Duration(500*(attempt+1)) * time.Millisecond)
	}
	return result
}

func runWithClangOnce(name, source string) runResult {
	start := time.Now()
	cppFile := filepath.Join(tmpDir, "test_"+name+".cpp")
	exeFile := filepath.Join(tmpDir, "test_"+name+".exe")
	if err := os.WriteFile(cppFile, []byte(source), 0o644); err != nil {
		return runResult{Compiler: "clang++", CompileError: err.Error(), ExitCode: -1,
			DurationMs: float64(time.Since(start).Milliseconds())}
	}

	// CommandContext：超时自动 kill 子进程（对应 Python subprocess timeout=30）。
	compileCtx, compileCancel := context.WithTimeout(context.Background(), 30*time.Second)
	defer compileCancel()
	compileCmd := exec.CommandContext(compileCtx, clangPath, cppFile, "-o", exeFile, "-std=c++14")
	var compileStdout, compileStderr bytes.Buffer
	compileCmd.Stdout = &compileStdout
	compileCmd.Stderr = &compileStderr
	compileErr := compileCmd.Run()
	if compileErr != nil {
		// 对齐 Python：compile_error 取 clang 的 stderr；起进程失败时取 err 文本。
		msg := compileErr.Error()
		if compileStderr.Len() > 0 {
			msg = compileStderr.String()
		}
		code := 1
		if compileCmd.ProcessState != nil {
			code = compileCmd.ProcessState.ExitCode()
		}
		return runResult{Compiler: "clang++", CompileSuccess: false, CompileError: msg,
			Stderr: compileStderr.String(), ExitCode: code,
			DurationMs: float64(time.Since(start).Milliseconds())}
	}

	runCtx, runCancel := context.WithTimeout(context.Background(), 5*time.Second)
	defer runCancel()
	runCmd := exec.CommandContext(runCtx, exeFile)
	var runOut, runErrBuf bytes.Buffer
	runCmd.Stdout = &runOut
	runCmd.Stderr = &runErrBuf
	runErr := runCmd.Run()
	runCode := -1
	if runCmd.ProcessState != nil {
		runCode = runCmd.ProcessState.ExitCode()
	}
	if runErr != nil {
		// 启动失败或超时被 kill：Python 版同口径——run_success=false、run_error 取 stderr
		return runResult{Compiler: "clang++", CompileSuccess: true,
			RunSuccess: false, RunError: runErrBuf.String(),
			Stdout: runOut.String(), Stderr: runErrBuf.String(), ExitCode: runCode,
			DurationMs: float64(time.Since(start).Milliseconds())}
	}
	return runResult{Compiler: "clang++", CompileSuccess: true,
		RunSuccess: runCode == 0, RunError: errStringIf(runErrBuf.String(), runCode != 0),
		Stdout: runOut.String(), Stderr: runErrBuf.String(), ExitCode: runCode,
		DurationMs: float64(time.Since(start).Milliseconds())}
}

func errStringIf(s string, cond bool) string {
	if cond {
		return s
	}
	return ""
}

// ---------------------------------------------------------------- Vitro 侧

// vitroMu：引擎 DLL 会话级线程安全性未验证，Vitro 侧全程串行（Clang 侧才是耗时大头）。
var vitroMu sync.Mutex

func runWithVitro(d *capi.DLL, source string) runResult {
	vitroMu.Lock()
	defer vitroMu.Unlock()

	start := time.Now()
	handle, _, _ := d.SessionCreate.Call()
	if handle == 0 {
		return runResult{Compiler: "vitro", CompileError: "session create failed", ExitCode: -1,
			DurationMs: float64(time.Since(start).Milliseconds())}
	}
	defer d.SessionDestroy.Call(handle)

	nameB := capi.CBytes("main.cpp")
	srcB := capi.CBytes(source)
	d.CompileUnit.Call(handle, uintptr(unsafe.Pointer(&nameB[0])), uintptr(unsafe.Pointer(&srcB[0])))
	runtime.KeepAlive(nameB)
	runtime.KeepAlive(srcB)

	compileRet, _, _ := d.CompileAll.Call(handle)
	if int32(compileRet) != 0 {
		errMsg := d.CompileErrorsExact(handle)
		if errMsg == "" {
			errMsg = "Unknown compile error"
		}
		return runResult{Compiler: "vitro", CompileSuccess: false, CompileError: errMsg,
			Stderr: errMsg, ExitCode: int(int32(compileRet)),
			DurationMs: float64(time.Since(start).Milliseconds())}
	}

	runRet, _, _ := d.Run.Call(handle)
	// E-P1-5：直接读纯程序 stdout 通道（引擎附注走 note 通道），禁止文本清洗。
	stdoutStr := strings.TrimSpace(capi.ReadChannel(handle, d.ProgOutLen, d.ProgOut))
	errPtr, _, _ := d.RuntimeError.Call(handle)
	runtimeErr := capi.PtrToGoString(errPtr)

	return runResult{Compiler: "vitro", CompileSuccess: true,
		RunSuccess: int32(runRet) == 0 && runtimeErr == "",
		RunError:   runtimeErr, Stdout: stdoutStr, Stderr: runtimeErr,
		ExitCode:   int(int32(runRet)),
		DurationMs: float64(time.Since(start).Milliseconds())}
}

// ---------------------------------------------------------------- 判定

func compareResults(clang, vitro runResult) string {
	if !clang.CompileSuccess {
		return "clang_compile_fail"
	}
	if !vitro.CompileSuccess {
		return "compile_gap"
	}
	if !clang.RunSuccess || !vitro.RunSuccess {
		if clang.RunSuccess != vitro.RunSuccess {
			return "runtime_gap"
		}
	}
	clangOut := capi.Normalize(clang.Stdout)
	vitroOut := capi.Normalize(vitro.Stdout)
	if clangOut != vitroOut {
		return "output_gap"
	}
	return "match"
}

// ---------------------------------------------------------------- 用例加载

func loadDirectoryCases() []shadowCase {
	entries, err := os.ReadDir(casesDir)
	if err != nil {
		return nil // 目录不存在：与 Python 版同口径，空集
	}
	names := make([]string, 0, len(entries))
	for _, e := range entries {
		if !e.IsDir() && strings.HasSuffix(e.Name(), ".cpp") {
			names = append(names, e.Name())
		}
	}
	sort.Strings(names) // 确定性：sorted(glob)，并发分片必须可复现

	cases := make([]shadowCase, 0, len(names))
	for _, fn := range names {
		raw, err := os.ReadFile(filepath.Join(casesDir, fn))
		if err != nil {
			capi.Fatal("读取用例失败 %s: %v", fn, err)
		}
		source := string(raw)
		category := "e2e_regression"
		trimmed := strings.TrimLeft(source, " \t\n\r\v\f")
		if firstLine := firstLineOf(trimmed); strings.HasPrefix(firstLine, "// category:") {
			category = strings.TrimSpace(strings.SplitN(firstLine, ":", 2)[1])
		}
		cases = append(cases, shadowCase{
			name:     strings.TrimSuffix(fn, ".cpp"),
			source:   source,
			category: category,
		})
	}
	return cases
}

func firstLineOf(s string) string {
	if s == "" {
		return ""
	}
	if i := strings.IndexAny(s, "\r\n"); i >= 0 {
		return s[:i]
	}
	return s
}

func allCases() ([]shadowCase, int) {
	dirCases := loadDirectoryCases()
	dirNames := make(map[string]bool, len(dirCases))
	for _, c := range dirCases {
		dirNames[c.name] = true
	}
	cases := make([]shadowCase, 0, len(cppCases)+len(dirCases))
	for _, c := range cppCases {
		if !dirNames[c.name] {
			cases = append(cases, c)
		}
	}
	return append(cases, dirCases...), len(dirCases)
}

// ---------------------------------------------------------------- 主流程

func main() {
	start := time.Now()
	selfCheck()

	if err := os.MkdirAll(tmpDir, 0o755); err != nil {
		capi.Fatal("无法创建工作目录 %s: %v", tmpDir, err)
	}
	// 清掉上一次运行的产物，避免读到陈旧可执行文件（删除失败不致命）。
	stale, _ := filepath.Glob(filepath.Join(tmpDir, "test*"))
	for _, p := range stale {
		os.Remove(p)
	}

	d := capi.Load(dllPath)

	cases, dirCount := allCases()
	inlineN := len(cases) - dirCount
	fmt.Printf("C++ Shadow Verification（Go 试点）：共 %d 用例（内嵌 %d + 目录 %d），Clang 并发 %d 路\n",
		len(cases), inlineN, dirCount, workerN)

	results := make([]shadowDiff, len(cases))
	printMu := &sync.Mutex{}
	sem := make(chan struct{}, workerN)
	var wg sync.WaitGroup

	for i, c := range cases {
		wg.Add(1)
		go func(i int, c shadowCase) {
			defer wg.Done()
			sem <- struct{}{}
			defer func() { <-sem }()

			clangRes := runWithClang(c.name, c.source)
			vitroRes := runWithVitro(d, c.source)
			diffType := compareResults(clangRes, vitroRes)
			results[i] = shadowDiff{
				CaseName:         c.name,
				ExpectedCategory: c.category,
				ClangResult:      clangRes,
				VitroResult:       vitroRes,
				DiffType:         diffType,
			}
			printMu.Lock()
			if diffType == "match" {
				fmt.Printf("  ✅ MATCH  %s\n", c.name)
			} else {
				fmt.Printf("  ❌ %-17s %s\n", strings.ToUpper(diffType), c.name)
			}
			printMu.Unlock()
		}(i, c)
	}
	wg.Wait()

	total := len(results)
	match := 0
	compileGap, runtimeGap, outputGap, clangFail := 0, 0, 0, 0
	for _, r := range results {
		switch r.DiffType {
		case "match":
			match++
		case "compile_gap":
			compileGap++
		case "runtime_gap":
			runtimeGap++
		case "output_gap":
			outputGap++
		case "clang_compile_fail":
			clangFail++
		}
	}

	var expectedGaps, unexpectedGaps []shadowDiff
	for _, r := range results {
		if r.DiffType == "match" {
			continue
		}
		if r.ExpectedCategory == "gap" {
			expectedGaps = append(expectedGaps, r)
		} else {
			unexpectedGaps = append(unexpectedGaps, r)
		}
	}

	fmt.Println()
	fmt.Println("============================================================")
	fmt.Println("C++ Shadow Verification 报告")
	fmt.Println("============================================================")
	fmt.Printf("总用例: %d\n", total)
	fmt.Printf("  ✅ 一致: %d\n", match)
	fmt.Printf("  ❌ 编译差异: %d\n", compileGap)
	fmt.Printf("  ❌ 运行时差异: %d\n", runtimeGap)
	fmt.Printf("  ❌ 输出差异: %d\n", outputGap)
	fmt.Printf("  ⚠️  Clang++ 编译失败: %d\n", clangFail)
	fmt.Printf("  📌 预期差异 (gap): %d\n", len(expectedGaps))
	fmt.Printf("  🚨 非预期差异: %d\n", len(unexpectedGaps))
	fmt.Printf("  ⏱ 耗时: %.1fs\n", time.Since(start).Seconds())

	if len(expectedGaps) > 0 {
		fmt.Println("\n预期差异用例（已记录的 Vitro 限制）：")
		for _, r := range expectedGaps {
			fmt.Printf("  - %s: %s\n", r.CaseName, r.DiffType)
		}
	}
	if len(unexpectedGaps) > 0 {
		fmt.Println("\n非预期差异用例（需要调查）：")
		for _, r := range unexpectedGaps {
			fmt.Printf("  - %s: %s\n", r.CaseName, r.DiffType)
		}
	}

	if err := os.MkdirAll(filepath.Dir(reportPath), 0o755); err != nil {
		capi.Fatal("无法创建报告目录: %v", err)
	}
	f, err := os.Create(reportPath)
	if err != nil {
		capi.Fatal("无法写报告 %s: %v", reportPath, err)
	}
	enc := json.NewEncoder(f)
	enc.SetEscapeHTML(false) // 对齐 Python ensure_ascii=False：不转义 <、>、&
	enc.SetIndent("", "  ")
	if err := enc.Encode(results); err != nil {
		f.Close()
		capi.Fatal("报告序列化失败: %v", err)
	}
	if err := f.Close(); err != nil {
		capi.Fatal("报告写入失败: %v", err)
	}
	fmt.Printf("\n报告已保存: %s\n", reportPath)

	// 只有非预期差异才导致失败（与 Python 版同口径）
	if len(unexpectedGaps) > 0 {
		os.Exit(1)
	}
}
