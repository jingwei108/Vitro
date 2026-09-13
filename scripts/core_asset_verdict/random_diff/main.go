//go:build windows

// random_diff —— 随机程序差分测试的 Go 迁移（D5 第三站：探针集，裁定文档 §13.5）。
//
// 三路对照（独立于 662 用例作者的通道）：
//
//	随机生成器（本脚本）──→ C 源码 ──→ clang  ──→ stdout_clang
//	        │                  └──────→ Vitro   ──→ stdout_vitro（纯程序 stdout 通道）
//	        └──→ 语义模型（求值器）──→ expected
//
// 判定：model ≠ clang → model_clang_mismatch（生成器自检，不算引擎缺陷）；
//
//	clang ≠ vitro → clang_vitro_mismatch（引擎与 C 标准的差异，最小复现存 .findings/）；
//	否则 agree。默认 10 族 × 100 例 = 1000 例，种子固定可复现。
//
// 与 Python 版（scripts/core_asset_verdict/random_diff.py）的对账口径：
//   - **RNG 逐比特复刻**：MT19937 + CPython 的字符串 seeding（sha512 → init_by_array）
//     与 getrandbits/_randbelow/randint/choice/random 全链路，同 seed 生成**同一用例集合**；
//   - 生成器与语义模型的 RNG 调用顺序与 Python 版逐行对齐；
//   - 双轨对账：逐用例 verdict 与 expected 必须一致（expected 是纯函数）；
//   - 已知差异（有意）：① runner 实现不同（Python 版依赖 shadow_verify.py，Go 版自带
//     等价 runner），clang/vitro 动态输出以 verdict 对账；② 本脚本首次有成功基线
//     （Python 版 2026-09-12 实测 1000/1000 agree / 50.7s，此前从无产物）。
//
// 用法：go run ./scripts/core_asset_verdict/random_diff [--per-family 100] [--seed 20260912] [--jobs 6]
package main

import (
	"vitro/scripts/internal/capi"
	"vitro/scripts/internal/probeutil"
	"vitro/scripts/internal/pyrandom"

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

var (
	here     = probeutil.VerdictDir()
	work     = filepath.Join(here, ".randomdiff")
	findings = filepath.Join(here, ".findings")
	report   = filepath.Join(here, "random_diff.json")
	dllPath  = filepath.Join(capi.ProjectRoot(), "native", "target", "release", "vitro_native.dll")
)

// ───────────────────────── CPython random 复刻（MT19937） ─────────────────────────
//
// 目标：random.Random(seed_str) 的输出序列与 CPython 逐比特一致。
// seeding 口径（CPython _randommodule.c random_seed）：
//   n = int.from_bytes(str_utf8 + sha512(str_utf8), 'big')
//   key = n 的 32bit word（LSB word 在前），去掉高位全零 word，至少 1 个
//   init_by_array(key)

// ───────────────────────── 语义模型工具（对齐 C int 语义） ─────────────────────────

func s32(v int64) int64 {
	v &= 0xFFFFFFFF
	if v >= 0x80000000 {
		v -= 0x100000000
	}
	return v
}

func u32(v uint64) uint64 {
	return v & 0xFFFFFFFF
}

// ───────────────────────── 族 1：int 表达式 ─────────────────────────

var intBinops = []string{"+", "-", "*", "/", "%", "&", "|", "^", "<<", ">>"}

type env struct {
	names []string // 插入序（对齐 Python dict 迭代序）
	vals  map[string]int64
}

func newEnv() *env { return &env{vals: map[string]int64{}} }

func (e *env) set(name string, v int64) {
	if _, has := e.vals[name]; !has {
		e.names = append(e.names, name)
	}
	e.vals[name] = v
}

func (e *env) get(name string) int64 { return e.vals[name] }

// evalInt 对齐 C（= Python eval_int 的截断语义）：Go 的整除/取余本身就向零截断
func evalInt(op string, lv, rv int64) (int64, bool) {
	switch op {
	case "+":
		return lv + rv, true
	case "-":
		return lv - rv, true
	case "*":
		return lv * rv, true
	case "/":
		if rv == 0 {
			return 0, false
		}
		return lv / rv, true
	case "%":
		if rv == 0 {
			return 0, false
		}
		return lv % rv, true
	case "&":
		return lv & rv, true
	case "|":
		return lv | rv, true
	case "^":
		return lv ^ rv, true
	case "<<":
		return lv << uint(rv), true
	case ">>":
		return lv >> uint(rv), true
	}
	capi.Fatal("evalInt 未知运算符 %s", op)
	return 0, false
}

// renderInt 返回 (c_code, value)；值域受限（|v| ≤ 500_000）避免 UB。
// RNG 调用顺序与 Python render_int 逐行对齐。
func renderInt(r *pyrandom.Random, e *env, depth int) (string, int64) {
	if depth <= 0 {
		if len(e.names) > 0 && r.Random() < 0.6 {
			n := e.names[r.Choice(len(e.names))]
			return n, e.get(n)
		}
		k := r.Randint(0, 20)
		return fmt.Sprint(k), int64(k)
	}
	op := intBinops[r.Choice(len(intBinops))]
	lc, lv := renderInt(r, e, depth-1)
	var rc string
	var rv int64
	switch op {
	case "<<", ">>":
		k := r.Randint(0, 3)
		rc, rv = fmt.Sprint(k), int64(k)
	case "*":
		k := r.Randint(0, 5)
		rc, rv = fmt.Sprint(k), int64(k)
	case "/", "%":
		k := r.Randint(1, 9)
		rc, rv = fmt.Sprint(k), int64(k)
	default:
		rc, rv = renderInt(r, e, depth-1)
	}
	v, ok := evalInt(op, lv, rv)
	if !ok || v > 500_000 || v < -500_000 {
		k := r.Randint(0, 20)
		return fmt.Sprint(k), int64(k)
	}
	return fmt.Sprintf("(%s %s %s)", lc, op, rc), v
}

func famInt(r *pyrandom.Random) (string, string, bool) {
	// build_int_program：200 次尝试，RNG 状态跨尝试延续
	for attempt := 0; attempt < 200; attempt++ {
		e := newEnv()
		var lines []string
		ok := true
		nvars := r.Randint(2, 4)
		for i := 0; i < nvars; i++ {
			val := r.Randint(0, 30)
			e.set(fmt.Sprint("v", i), int64(val))
			lines = append(lines, fmt.Sprintf("    int v%d = %d;", i, val))
		}
		nstmt := r.Randint(2, 5)
		for i := 0; i < nstmt; i++ {
			tgt := e.names[r.Choice(len(e.names))]
			code, val := renderInt(r, e, 2)
			if val > 2_000_000 || val < -2_000_000 {
				ok = false
				break
			}
			e.set(tgt, val)
			lines = append(lines, fmt.Sprintf("    %s = %s;", tgt, code))
		}
		if !ok {
			continue
		}
		var prints, expected strings.Builder
		for _, v := range e.names {
			prints.WriteString(fmt.Sprintf("    printf(\"%%d\\n\", %s);\n", v))
			expected.WriteString(fmt.Sprint(e.get(v), "\n"))
		}
		src := "#include <stdio.h>\nint main() {\n" + strings.Join(lines, "\n") + "\n" + prints.String() + "    return 0;\n}\n"
		return src, expected.String(), true
	}
	return "", "", false
}

// ───────────────────────── 族 2：unsigned 算术 ─────────────────────────

func famUnsigned(r *pyrandom.Random) (string, string, bool) {
	e := newEnv()
	var lines []string
	nvars := r.Randint(2, 4)
	for i := 0; i < nvars; i++ {
		val := r.Randint(0, 4_000_000_000)
		e.set(fmt.Sprint("u", i), int64(uint64(val)))
		lines = append(lines, fmt.Sprintf("    unsigned int u%d = %du;", i, val))
	}
	nstmt := r.Randint(2, 4)
	for i := 0; i < nstmt; i++ {
		tgt := e.names[r.Choice(len(e.names))]
		aName := e.names[r.Choice(len(e.names))]
		a := uint64(e.get(aName))
		op := intBinops[r.Choice(len(intBinops))]
		var b uint64
		var rhs string
		switch op {
		case "<<", ">>":
			k := r.Randint(0, 31)
			b, rhs = uint64(k), fmt.Sprint(k)
		case "/", "%":
			k := r.Randint(1, 65535)
			b, rhs = uint64(k), fmt.Sprint(k)
		case "*":
			k := r.Randint(0, 1_000_000)
			b, rhs = uint64(k), fmt.Sprintf("%du", k)
		default:
			bName := e.names[r.Choice(len(e.names))]
			b = uint64(e.get(bName))
			rhs = bName
		}
		var v uint64
		switch op {
		case "+":
			v = u32(a + b)
		case "-":
			v = u32(a - b)
		case "*":
			v = u32(a * b)
		case "/":
			v = u32(a / b)
		case "%":
			v = u32(a % b)
		case "&":
			v = u32(a & b)
		case "|":
			v = u32(a | b)
		case "^":
			v = u32(a ^ b)
		case "<<":
			v = u32(a << b)
		case ">>":
			v = u32(a >> b)
		}
		e.set(tgt, int64(v))
		lines = append(lines, fmt.Sprintf("    %s = %s %s %s;", tgt, aName, op, rhs))
	}
	var prints, expected strings.Builder
	for _, v := range e.names {
		prints.WriteString(fmt.Sprintf("    printf(\"%%u\\n\", %s);\n", v))
		expected.WriteString(fmt.Sprint(uint64(e.get(v)), "\n"))
	}
	src := "#include <stdio.h>\nint main() {\n" + strings.Join(lines, "\n") + "\n" + prints.String() + "    return 0;\n}\n"
	return src, expected.String(), true
}

// ───────────────────────── 族 3：数组/指针循环 ─────────────────────────

func famArray(r *pyrandom.Random) (string, string, bool) {
	k := r.Randint(2, 8)
	init := make([]int, k)
	mult := make([]int, k)
	for i := 0; i < k; i++ {
		init[i] = r.Randint(-20, 20)
	}
	for i := 0; i < k; i++ {
		mult[i] = r.Randint(-3, 3)
	}
	var parts []string
	setParts := make([]string, k)
	for i := 0; i < k; i++ {
		setParts[i] = fmt.Sprintf("a[%d] = %d;", i, init[i])
	}
	parts = append(parts,
		"#include <stdio.h>",
		"int main() {",
		fmt.Sprintf("    int a[%d];", k),
		"    "+strings.Join(setParts, " "),
		"    int s = 0;",
		fmt.Sprintf("    for (int i = 0; i < %d; i++) { s += a[i] * %d; }", k, mult[0]),
		"    int t = 0;",
		"    int* p = a;",
	)
	lastMult := mult[k-1]
	if lastMult == 0 {
		lastMult = 1
	}
	parts = append(parts,
		fmt.Sprintf("    for (int i = 0; i < %d; i++) { t += *(p + i) * %d; }", k, lastMult),
		"    printf(\"%d\\n\", s);",
		"    printf(\"%d\\n\", t);",
		"    return 0;",
		"}",
	)
	s, t := 0, 0
	for i := 0; i < k; i++ {
		s += init[i] * mult[0]
	}
	for i := 0; i < k; i++ {
		t += init[i] * lastMult
	}
	return strings.Join(parts, "\n") + "\n", fmt.Sprintf("%d\n%d\n", s, t), true
}

// ───────────────────────── 族 4：控制流 ─────────────────────────

func famControl(r *pyrandom.Random) (string, string, bool) {
	x0 := r.Randint(-10, 10)
	m := r.Randint(3, 12)
	stepAdd := r.Randint(-4, 4)
	stepSub := r.Randint(-4, 4)
	lim := r.Randint(-20, 20)
	brk := r.Randint(0, m)
	x := x0
	for i := 0; i < m; i++ {
		if i > brk {
			break
		}
		if i%3 == 0 {
			x += stepAdd
			continue
		}
		x -= stepSub
	}
	src := fmt.Sprintf(`#include <stdio.h>
int main() {
    int x = %d;
    for (int i = 0; i < %d; i++) {
        if (i > %d) break;
        if (i %% 3 == 0) { x += %d; continue; }
        x -= %d;
    }
    printf("%%d\n", x);
    printf("%%d\n", x > %d ? 1 : 0);
    return 0;
}
`, x0, m, brk, stepAdd, stepSub, lim)
	cmp := 0
	if x > lim {
		cmp = 1
	}
	return src, fmt.Sprintf("%d\n%d\n", x, cmp), true
}

// ───────────────────────── 族 5：函数调用 + 递归 ─────────────────────────

func famFuncs(r *pyrandom.Random) (string, string, bool) {
	a1 := r.Randint(1, 30)
	b1 := r.Randint(1, 30)
	a2 := r.Randint(1, 30)
	b2 := r.Randint(1, 30)
	n := r.Randint(1, 7)
	src := fmt.Sprintf(`#include <stdio.h>
int f1(int a, int b) { return a * %d + b - %d; }
int f2(int a, int b) { return a + b * %d %% (%d + 1); }
int rec(int n) { if (n <= 1) return 1; return n + rec(n - 1); }
int main() {
    printf("%%d\n", f1(%d, %d));
    printf("%%d\n", f2(%d, %d));
    printf("%%d\n", rec(%d));
    printf("%%d\n", f1(f2(%d, %d), rec(%d)));
    return 0;
}
`, a1, b1, a2, b2, a1, b1, a2, b2, n, a1, b1, n)
	v1 := a1*a1 + b1 - b1
	v2 := a2 + b2*a2%(b2+1)
	v3 := 0
	for i := 1; i <= n; i++ {
		v3 += i
	}
	if n <= 1 {
		v3 = 1
	}
	w := a1 + b1*a2%(b2+1)
	v4 := w*a1 + v3 - b1
	return src, fmt.Sprintf("%d\n%d\n%d\n%d\n", v1, v2, v3, v4), true
}

// ───────────────────────── 族 6：struct 与 struct 指针 ─────────────────────────

func famStruct(r *pyrandom.Random) (string, string, bool) {
	k := 4
	xs := make([]int, k)
	ys := make([]int, k)
	for i := 0; i < k; i++ {
		xs[i] = r.Randint(-15, 15)
		ys[i] = r.Randint(-15, 15)
	}
	rowParts := make([]string, k)
	for i := 0; i < k; i++ {
		rowParts[i] = fmt.Sprintf("a[%d].x = %d; a[%d].y = %d;", i, xs[i], i, ys[i])
	}
	rows := strings.Join(rowParts, " ")
	src := fmt.Sprintf(`#include <stdio.h>
struct S { int x; int y; };
int main() {
    struct S a[%d];
    %s
    int s = 0;
    for (int i = 0; i < %d; i++) { s += a[i].x * a[i].y; }
    struct S* p = a;
    int t = 0;
    for (int i = 0; i < %d; i++) { t += p->x - p->y; p++; }
    printf("%%d\n", s);
    printf("%%d\n", t);
    return 0;
}
`, k, rows, k, k)
	s, t := 0, 0
	for i := 0; i < k; i++ {
		s += xs[i] * ys[i]
		t += xs[i] - ys[i]
	}
	return src, fmt.Sprintf("%d\n%d\n", s, t), true
}

// ───────────────────────── 族 7：char 数组 / 字符串 ─────────────────────────

var words = []string{"hello", "abcde", "xyz12", "World", "qwert"}

func famChar(r *pyrandom.Random) (string, string, bool) {
	w := words[r.Choice(len(words))]
	n := len(w)
	src := fmt.Sprintf(`#include <stdio.h>
#include <string.h>
int main() {
    char s[%d] = "%s";
    int sum = 0;
    for (int i = 0; i < %d; i++) { sum += s[i]; }
    printf("%%d\n", sum);
    printf("%%d\n", (int)strlen(s));
    char d[%d];
    for (int i = 0; i < %d; i++) { d[i] = s[%d - i]; }
    d[%d] = 0;
    printf("%%s\n", d);
    return 0;
}
`, n+1, w, n, n+1, n, n-1, n)
	sum := 0
	rb := make([]byte, n)
	for i := 0; i < n; i++ {
		sum += int(w[i])
		rb[i] = w[n-1-i]
	}
	return src, fmt.Sprintf("%d\n%d\n%s\n", sum, n, string(rb)), true
}

// ───────────────────────── 族 8：malloc 链表 ─────────────────────────

func famMallocList(r *pyrandom.Random) (string, string, bool) {
	k := r.Randint(2, 6)
	vals := make([]int, k)
	for i := 0; i < k; i++ {
		vals[i] = r.Randint(-9, 9)
	}
	pushParts := make([]string, k)
	for i := 0; i < k; i++ {
		pushParts[i] = fmt.Sprintf("    struct Node* n%d = (struct Node*)malloc(sizeof(struct Node)); n%d->v = %d; n%d->next = head; head = n%d;", i, i, vals[i], i, i)
	}
	pushes := strings.Join(pushParts, "\n")
	src := fmt.Sprintf(`#include <stdio.h>
#include <stdlib.h>
struct Node { int v; struct Node* next; };
int main() {
    struct Node* head = 0;
%s
    int s = 0;
    for (struct Node* p = head; p != 0; p = p->next) { s += p->v; }
    printf("%%d\n", s);
    struct Node* p = head;
    while (p != 0) { struct Node* nx = p->next; free(p); p = nx; }
    printf("freed\n");
    return 0;
}
`, pushes)
	sum := 0
	for _, v := range vals {
		sum += v
	}
	return src, fmt.Sprintf("%d\nfreed\n", sum), true
}

// ───────────────────────── 族 9：二维数组 + 嵌套循环 ─────────────────────────

func famNested(r *pyrandom.Random) (string, string, bool) {
	rr := r.Randint(2, 4)
	c := r.Randint(2, 4)
	src := fmt.Sprintf(`#include <stdio.h>
int main() {
    int a[%d][%d];
    int s = 0;
    for (int i = 0; i < %d; i++) {
        for (int j = 0; j < %d; j++) {
            a[i][j] = i * %d + j;
            if (j == %d) continue;
            s += a[i][j];
        }
    }
    int t = 0;
    for (int i = 0; i < %d; i++) {
        for (int j = 0; j < %d; j++) {
            if (a[i][j] > 5) break;
            t += a[i][j];
        }
    }
    printf("%%d\n", s);
    printf("%%d\n", t);
    return 0;
}
`, rr, c, rr, c, c, c-1, rr, c)
	s := 0
	for i := 0; i < rr; i++ {
		for j := 0; j < c; j++ {
			if j != c-1 {
				s += i*c + j
			}
		}
	}
	t := 0
	for i := 0; i < rr; i++ {
		for j := 0; j < c; j++ {
			if i*c+j > 5 {
				break
			}
			t += i*c + j
		}
	}
	return src, fmt.Sprintf("%d\n%d\n", s, t), true
}

// ───────────────────────── 族 10：switch / do-while / 位运算 ─────────────────────────

func famSwitch(r *pyrandom.Random) (string, string, bool) {
	n := r.Randint(3, 8)
	src := fmt.Sprintf(`#include <stdio.h>
int main() {
    int s = 0;
    for (int i = 0; i < %d; i++) {
        switch (i %% 4) {
            case 0: s += 1; break;
            case 1: s += i; break;
            case 2: s -= 2;
            default: s += 3; break;
        }
    }
    int j = 0;
    do { s ^= (1 << (j %% 5)); j++; } while (j < %d);
    printf("%%d\n", s);
    return 0;
}
`, n, n)
	s := 0
	for i := 0; i < n; i++ {
		switch i % 4 {
		case 0:
			s += 1
		case 1:
			s += i
		case 2:
			s -= 2
			s += 3
		default:
			s += 3
		}
	}
	for j := 0; j < n; j++ {
		s ^= 1 << uint(j%5)
	}
	return src, fmt.Sprintf("%d\n", s), true
}

type family struct {
	name string
	fn   func(*pyrandom.Random) (string, string, bool)
}

var families = []family{
	{"int_expr", famInt},
	{"unsigned", famUnsigned},
	{"array_ptr", famArray},
	{"control", famControl},
	{"funcs", famFuncs},
	{"struct", famStruct},
	{"char_str", famChar},
	{"malloc_list", famMallocList},
	{"nested_loop", famNested},
	{"switch_bits", famSwitch},
}

// ───────────────────────── runner（等价 shadow_verify.py 的 run_with_clang / run_with_vitro） ─────────────────────────

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

// makeClangHeader 对齐 shadow_verify.make_clang_header：始终注入 stdio.h，
// 仅当源码调用 atof/atoi/atol/exit 且未自定义时注入前向声明。
func makeClangHeader(source string) string {
	header := "#include <stdio.h>\n"
	var decls []string
	if strings.Contains(source, "atof(") && !strings.Contains(source, "double atof") {
		decls = append(decls, "double atof(const char *nptr);")
	}
	if strings.Contains(source, "atoi(") && !strings.Contains(source, "int atoi") {
		decls = append(decls, "int atoi(const char *nptr);")
	}
	if strings.Contains(source, "atol(") && !strings.Contains(source, "long atol") {
		decls = append(decls, "long atol(const char *nptr);")
	}
	if strings.Contains(source, "exit(") && !strings.Contains(source, "void exit") {
		decls = append(decls, "void exit(int status);")
	}
	for _, d := range decls {
		header += d + "\n"
	}
	return header
}

func runWithClang(fam string, idx int, source string) runResult {
	start := time.Now()
	workDir := filepath.Join(work, fmt.Sprintf("%s_%d", fam, idx))
	if err := os.MkdirAll(workDir, 0o755); err != nil {
		return runResult{Compiler: "clang", CompileError: err.Error(), ExitCode: -1,
			DurationMs: ms(time.Since(start))}
	}
	exeFile := filepath.Join(workDir, "test.exe")
	cFile := filepath.Join(workDir, "test.c")
	if err := os.WriteFile(cFile, []byte(makeClangHeader(source)+source), 0o644); err != nil {
		return runResult{Compiler: "clang", CompileError: err.Error(), ExitCode: -1,
			DurationMs: ms(time.Since(start))}
	}

	ctx, cancel := context.WithTimeout(context.Background(), 30*time.Second)
	defer cancel()
	cmd := exec.CommandContext(ctx, "clang", cFile, "-o", exeFile, "-Wno-implicit-function-declaration")
	var errBuf bytes.Buffer
	cmd.Stderr = &errBuf
	cmd.Stdout = &errBuf
	if err := cmd.Run(); err != nil {
		msg := err.Error()
		if errBuf.Len() > 0 {
			msg = errBuf.String()
		}
		code := -1
		if cmd.ProcessState != nil {
			code = cmd.ProcessState.ExitCode()
		}
		return runResult{Compiler: "clang", CompileSuccess: false, CompileError: msg,
			Stderr: errBuf.String(), ExitCode: code, DurationMs: ms(time.Since(start))}
	}

	runCtx, runCancel := context.WithTimeout(context.Background(), 5*time.Second)
	defer runCancel()
	runCmd := exec.CommandContext(runCtx, exeFile)
	runCmd.Dir = workDir
	var runOut, runErr bytes.Buffer
	runCmd.Stdout = &runOut
	runCmd.Stderr = &runErr
	runErrErr := runCmd.Run()
	code := -1
	if runCmd.ProcessState != nil {
		code = runCmd.ProcessState.ExitCode()
	}
	// 对齐 Python subprocess text=True 的 universal newlines：Windows CRT 的
	// stdout 文本模式会把 \n 写成 \r\n，Python 侧读回时已翻译为 \n——不归一
	// 会把所有用例打成假 mismatch（实测 2026-09-12）。
	if runErrErr != nil {
		return runResult{Compiler: "clang", CompileSuccess: true,
			RunSuccess: false, RunError: crlfToLf(runErr.String()),
			Stdout: crlfToLf(runOut.String()), Stderr: crlfToLf(runErr.String()), ExitCode: code,
			DurationMs: ms(time.Since(start))}
	}
	return runResult{Compiler: "clang", CompileSuccess: true,
		RunSuccess: code == 0, RunError: ifStr(crlfToLf(runErr.String()), code != 0),
		Stdout: crlfToLf(runOut.String()), Stderr: crlfToLf(runErr.String()), ExitCode: code,
		DurationMs: ms(time.Since(start))}
}

func crlfToLf(s string) string {
	return strings.ReplaceAll(s, "\r\n", "\n")
}

func ms(d time.Duration) float64 { return float64(d.Milliseconds()) }

func ifStr(s string, cond bool) string {
	if cond {
		return s
	}
	return ""
}

// vitro DLL 绑定（进程内加载一次；session 由每例独立 create/destroy）
type vitroAPI struct {
	d      *capi.DLL
	handle uintptr
}

var (
	vitroOnce   sync.Once
	vitroShared *vitroAPI
)

// vitroMu：Vitro 调用必须串行。**实证**（2026-09-12）：与 clang 并发同池跑 DLL 时
// 进程以 0xc0000374（STATUS_HEAP_CORRUPTION）崩死——引擎 DLL 存在非线程安全的
// 内部状态，多线程并发调用不安全。Python 版此前未崩只是 6 线程下 vitro 调用
// 碰撞率低（clang 编译占去大部分时长），并非线程安全的证据。
// 代价：vitro 串行 ~45s 成为瓶颈，总耗时与 Python 版持平（~50s）——本探针不在
// CI 热路径上，迁移价值在编码安全与可审计性，非性能。
var vitroMu sync.Mutex

func loadVitro() *vitroAPI {
	vitroOnce.Do(func() {
		d := capi.Load(dllPath) // 符号绑定 + 产物新鲜度门禁（fail fast）
		h, _, _ := d.SessionCreate.Call()
		if h == 0 {
			capi.Fatal("vitro_session_create 返回 NULL")
		}
		vitroShared = &vitroAPI{d: d, handle: h}
	})
	return vitroShared
}

func runWithVitro(source string) runResult {
	vitroMu.Lock()
	defer vitroMu.Unlock()
	start := time.Now()
	a := loadVitro()

	srcB := capi.CBytes(source)
	// shadow_verify.run_with_vitro 无 filename 路径：vitro_compile(session, source)
	ret, _, _ := a.d.Compile.Call(a.handle, uintptr(unsafe.Pointer(&srcB[0])))
	runtime.KeepAlive(srcB)
	if int32(ret) != 0 {
		msg := a.d.CompileErrorsExact(a.handle)
		if msg == "" {
			msg = "Unknown compile error"
		}
		return runResult{Compiler: "vitro", CompileSuccess: false, CompileError: msg,
			Stderr: msg, ExitCode: int(int32(ret)), DurationMs: ms(time.Since(start))}
	}

	a.d.SetInputMode.Call(a.handle, 1)
	runRet, _, _ := a.d.Run.Call(a.handle)
	stdout := strings.TrimSpace(capi.ReadChannel(a.handle, a.d.ProgOutLen, a.d.ProgOut))
	runtimeErr := a.d.RuntimeErr(a.handle)

	return runResult{Compiler: "vitro", CompileSuccess: true,
		RunSuccess: int32(runRet) == 0 && runtimeErr == "",
		RunError:   runtimeErr, Stdout: stdout, Stderr: runtimeErr,
		ExitCode: int(int32(runRet)), DurationMs: ms(time.Since(start))}
}

// ───────────────────────── 判定与主流程 ─────────────────────────

type diffRecord struct {
	Family       string `json:"family"`
	Index        int    `json:"index"`
	Verdict      string `json:"verdict"`
	Expected     string `json:"expected"`
	Clang        string `json:"clang"`
	Vitro         string `json:"vitro"`
	ClangCompile bool   `json:"clang_compile"`
	VitroCompile  bool   `json:"vitro_compile"`
	VitroErr      string `json:"vitro_err"`
	Saved        string `json:"saved,omitempty"`
}

func truncate(s string, n int) string {
	r := []rune(s)
	if len(r) > n {
		return string(r[:n])
	}
	return s
}

func runOne(item struct {
	fam           string
	idx           int
	src, expected string
}) diffRecord {
	clang := runWithClang(item.fam, item.idx, item.src)
	vitro := runWithVitro(item.src)
	gClang := strings.TrimSpace(clang.Stdout)
	gVitro := strings.TrimSpace(vitro.Stdout)
	exp := strings.TrimSpace(item.expected)
	verdict := "agree"
	if gClang != exp {
		verdict = "model_clang_mismatch"
	} else if gClang != gVitro {
		verdict = "clang_vitro_mismatch"
	}
	rec := diffRecord{
		Family: item.fam, Index: item.idx, Verdict: verdict,
		Expected: truncate(exp, 200), Clang: truncate(gClang, 200), Vitro: truncate(gVitro, 200),
		ClangCompile: clang.CompileSuccess, VitroCompile: vitro.CompileSuccess,
		VitroErr: truncate(vitro.CompileError, 200),
	}
	if verdict != "agree" {
		os.MkdirAll(findings, 0o755)
		p := filepath.Join(findings, fmt.Sprintf("%s_%d_%s.c", item.fam, item.idx, verdict))
		os.WriteFile(p, []byte(fmt.Sprintf("%s\n/* expected=%q\n   clang=%q\n   vitro=%q */\n",
			item.src, exp, gClang, gVitro)), 0o644)
		if rel, err := filepath.Rel(filepath.Dir(filepath.Dir(here)), p); err == nil {
			rec.Saved = filepath.ToSlash(rel)
		}
	}
	return rec
}

func selfTest() {
	// 金标：CPython 3.14 实测（seed="20260912:int_expr"）
	r := pyrandom.NewByString("20260912:int_expr")
	got := []float64{r.Random(), r.Random(), r.Random()}
	want := []float64{0.12077530804515435, 0.13829212145236314, 0.3025312629379405}
	for i := range want {
		if got[i] != want[i] {
			capi.Fatal("selftest：random()[%d] = %v，期望 %v（RNG 复刻破坏，用例集合将与 Python 版不一致）", i, got[i], want[i])
		}
	}
	r2 := pyrandom.NewByString("20260912:int_expr")
	for i, w := range []int{7, 28, 8, 21, 19} {
		if g := r2.Randint(0, 40); g != w {
			capi.Fatal("selftest：randint(0,40)[%d] = %d，期望 %d", i, g, w)
		}
	}
	r3 := pyrandom.NewByString("20260912:int_expr")
	if g := r3.Randint(2, 4); g != 2 {
		capi.Fatal("selftest：randint(2,4) = %d，期望 2", g)
	}
	if g := r3.Choice(len(words)); words[g] != "World" {
		capi.Fatal("selftest：choice(WORDS) = %s，期望 World", words[g])
	}
	if g := r3.Randint(0, 4_000_000_000); g != 593960143 {
		capi.Fatal("selftest：randint(0,4e9) = %d，期望 593960143", g)
	}
	// 语义模型：负数除法/取模向零截断（C 语义）
	if v, _ := evalInt("/", -7, 2); v != -3 {
		capi.Fatal("selftest：-7/2 = %v，期望 -3（C 截断除）", v)
	}
	if v, _ := evalInt("%", -7, 2); v != -1 {
		capi.Fatal("selftest：-7%%2 = %v，期望 -1（C 余号随被除数）", v)
	}
	fmt.Println("selftest：RNG 逐比特复刻与语义模型断言全部通过（金标 = CPython 3.14 实测）")
}

func main() {
	per, seed, jobs := 100, 20260912, 6
	args := os.Args[1:]
	for i := 0; i+1 < len(args); i++ {
		switch args[i] {
		case "--per-family":
			fmt.Sscan(args[i+1], &per)
		case "--seed":
			fmt.Sscan(args[i+1], &seed)
		case "--jobs":
			fmt.Sscan(args[i+1], &jobs)
		}
	}
	start := time.Now()
	selfTest()
	fmt.Printf("per_family=%d seed=%d jobs=%d\n", per, seed, jobs)

	// 生成（与 Python 版同序：FAMILIES 定义序 → 种子 f"{seed}:{fam}"）
	type item struct {
		fam      string
		idx      int
		src      string
		expected string
	}
	var items []item
	for _, f := range families {
		r := pyrandom.NewByString(fmt.Sprintf("%d:%s", seed, f.name))
		made, attempt := 0, 0
		for made < per && attempt < per*5 {
			attempt++
			src, expected, ok := f.fn(r)
			if !ok {
				continue
			}
			items = append(items, item{f.name, made, src, expected})
			made++
		}
	}
	fmt.Printf("生成 %d 例（%d 族），开始三路差分…\n", len(items), len(families))

	if err := os.MkdirAll(work, 0o755); err != nil {
		capi.Fatal("无法创建工作目录 %s: %v", work, err)
	}

	results := make([]diffRecord, len(items))
	sem := make(chan struct{}, jobs)
	var wg sync.WaitGroup
	for i, it := range items {
		wg.Add(1)
		go func(i int, it item) {
			defer wg.Done()
			sem <- struct{}{}
			defer func() { <-sem }()
			results[i] = runOne(it)
		}(i, it)
	}
	wg.Wait()

	cnt := map[string]int{}
	for _, r := range results {
		cnt[r.Verdict]++
	}
	fmt.Printf("\n总样本 %d\n", len(results))
	// 按 Python Counter.most_common 语义：计数降序
	type kv struct {
		k string
		v int
	}
	var kvs []kv
	for k, v := range cnt {
		kvs = append(kvs, kv{k, v})
	}
	sort.Slice(kvs, func(i, j int) bool { return kvs[i].v > kvs[j].v })
	for _, x := range kvs {
		fmt.Printf("  %s: %d\n", x.k, x.v)
	}
	for _, f := range families {
		sub := map[string]int{}
		for _, r := range results {
			if r.Family == f.name {
				sub[r.Verdict]++
			}
		}
		parts := make([]string, 0, len(sub))
		keys := make([]string, 0, len(sub))
		for k := range sub {
			keys = append(keys, k)
		}
		sort.Strings(keys)
		for _, k := range keys {
			parts = append(parts, fmt.Sprintf("%q: %d", k, sub[k]))
		}
		fmt.Printf("  [%-10s] {%s}\n", f.name, strings.Join(parts, ", "))
	}
	var bad []diffRecord
	for _, r := range results {
		if r.Verdict != "agree" {
			bad = append(bad, r)
		}
	}
	for i, r := range bad {
		if i >= 25 {
			break
		}
		fmt.Printf("    ! %s/%d: %s\n", r.Family, r.Index, r.Verdict)
		fmt.Printf("      expected=%q\n", truncate(r.Expected, 80))
		fmt.Printf("      clang   =%q\n", truncate(r.Clang, 80))
		fmt.Printf("      vitro    =%q\n", truncate(r.Vitro, 80))
	}

	f, err := os.Create(report)
	if err != nil {
		capi.Fatal("无法写报告 %s: %v", report, err)
	}
	enc := json.NewEncoder(f)
	enc.SetEscapeHTML(false)
	enc.SetIndent("", " ")
	if err := enc.Encode(results); err != nil {
		f.Close()
		capi.Fatal("报告序列化失败: %v", err)
	}
	f.Close()
	fmt.Printf("\nJSON 已写出: %s\n最小复现目录: %s\n耗时: %.1fs\n", report, findings, time.Since(start).Seconds())

	// 门禁：任何非 agree 都不静默放行（与 Python 版 return 0 不同——本版有牙）
	if len(bad) > 0 {
		os.Exit(1)
	}
}
