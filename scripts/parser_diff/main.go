// parser_diff：S3 解析层差分驱动（E1–E4 + 病态 12 样本 + 活性断言）。
//
// 用法（仓库根）：
//
//	go run ./scripts/parser_diff <corpus_dir>            # E1/E2 全量对拍
//	go run ./scripts/parser_diff --pathological           # E3 病态 12 样本（同等拒绝）
//	go run ./scripts/parser_diff --legal-deep             # E4 合法深嵌套反向锚
//	go run ./scripts/parser_diff <corpus_dir> --selftest  # J9：注入差异先证红
//
// 对拍面：
//   - E1：AST dump JSON——Rust serve `ast.dump` 响应 vs MoonBit
//     `cmd/dump_ast` 输出，双侧经 scripts/canonicalize 归一后逐字节 diff
//   - E2：parse_errors 序列（code/line/column 保序；message 在 JSON 内
//     一并逐字节比——文案两侧照搬自同一源，漂移即红）
//   - 活性：MoonBit 侧 stall_count 必须为 0（勘察 6-3 的 GC 语言观测义务；
//     Rust 侧恒 0）
//
// fail loud：自检（serve 可用 / 两侧文件数一致 / canonicalize 可用）不过
// 直接拒绝给判定；差异全量列出后 exit 1。
package main

import (
	"bufio"
	"bytes"
	"encoding/json"
	"fmt"
	"os"
	"os/exec"
	"path/filepath"
	"regexp"
	"strconv"
)

func main() {
	if len(os.Args) < 2 {
		fmt.Fprintln(os.Stderr, "用法: go run ./scripts/parser_diff <corpus_dir|--pathological|--legal-deep> [--selftest]")
		os.Exit(2)
	}
	mode := os.Args[1]
	selftest := len(os.Args) > 2 && os.Args[2] == "--selftest"
	switch {
	case mode == "--pathological":
		os.Exit(runPathological(selftest))
	case mode == "--legal-deep":
		os.Exit(runLegalDeep(selftest))
	default:
		corpus, err := filepath.Abs(mode)
		if err != nil {
			fail("语料路径解析失败: %v", err)
		}
		os.Exit(runCorpus(corpus, selftest))
	}
}

// ---------------------------------------------------------------------------
// 语料对拍（E1/E2/活性）
// ---------------------------------------------------------------------------

func runCorpus(corpus string, selftest bool) int {
	files := listCFiles(corpus)
	if len(files) == 0 {
		fail("语料目录无 .c 文件: %s", corpus)
	}
	rustOuts := rustAstDumpBatch(files)
	if selftest {
		// J9 埋雷：篡改首个文件的 Rust 侧输出（ok 翻转），驱动必须红
		rustOuts[0] = []byte(`{"ok": false, "parse_error_count": 0, "ast": null, "parse_errors": [], "stall_count": 0}`)
		fmt.Println("parser_diff: selftest 已注入差异（Rust 侧样本 0 篡改）")
	}
	moonDir := moonDump(corpus)
	failures := 0
	for i, f := range files {
		stem := stemOf(f)
		moonRaw, err := os.ReadFile(filepath.Join(moonDir, stem+".c.ast.json"))
		if err != nil {
			// dump_ast 输出名 = <stem>.c.ast.json（basename 去 .ast 后缀前含 .c）
			moonRaw, err = os.ReadFile(filepath.Join(moonDir, stem+".ast.json"))
			if err != nil {
				fail("MoonBit 侧输出缺失: %s (%v)", stem, err)
			}
		}
		rustNorm := canonicalize(rustOuts[i])
		moonNorm := canonicalize(moonRaw)
		if !bytes.Equal(rustNorm, moonNorm) {
			failures++
			fmt.Printf("DIFF %s\n  rust: %s\n  moon: %s\n", filepath.Base(f),
				preview(rustNorm), preview(moonNorm))
			continue
		}
		// 活性断言：stall_count 必须为 0（归一化文本内直接查）
		if bytes.Contains(moonNorm, []byte(`"stall_count": 0`)) == false {
			failures++
			fmt.Printf("STALL %s：MoonBit 侧解析器出现零推进迭代\n", filepath.Base(f))
		}
	}
	if failures > 0 {
		fmt.Printf("parser_diff: FAIL——%d/%d 处差异（语料 %s）\n", failures, len(files), corpus)
		return 1
	}
	fmt.Printf("parser_diff: PASS——%d 个样本 AST+诊断序列归一后逐字节一致（语料 %s）\n", len(files), corpus)
	return 0
}

// ---------------------------------------------------------------------------
// E3 病态 12 样本（勘察 6-4 / 6-11 全部通道；两侧"同等拒绝"）
// ---------------------------------------------------------------------------

// pathologicalSamples：12 条（6-4 通道 7 条 + 6-11 通道 5 条）。
// 每条：名字、生成器（规模参数展开）、预期（两侧都拒绝：ok=false 或
// parse_errors 非空；进程不崩）。
var pathologicalSamples = []struct {
	name string
	src  string
}{
	{"paren_63", "int main() { return " + rep("(", 63) + "1" + rep(")", 63) + "; }"},
	{"brace_300", "int main() {" + rep("{", 300) + rep("}", 300) + "return 0; }"},
	{"init_list_3000", "int a[" + strconv.Itoa(3000) + "] = " + rep("{", 3000) + "1" + rep("}", 3000) + "; int main() { return 0; }"},
	{"assign_chain_3000", "int main() { int a = 0; a" + rep(" = a", 3000) + "; return a; }"},
	{"unary_chain_5000", "int main() { return " + rep("!", 5000) + "1; }"},
	{"pointer_chain_100", "int " + rep("*", 100) + " p; int main() { return 0; }"},
	{"postfix_chain_5000", "int main() { int a[1]; return a" + rep("[0]", 5000) + "; }"},
	{"decl_suffix_1300", "int a" + rep("[1]", 1300) + ";"},
	{"typedef_suffix_1500", "typedef int T" + rep("[1]", 1500) + ";"},
	{"sizeof_abstract_1400", "int main() { return sizeof(int" + rep("[1]", 1400) + "); }"},
	{"param_suffix_1500", "int f(int a" + rep("[1]", 1500) + ") { return 0; }"},
	{"field_suffix_1500", "struct S { int a" + rep("[1]", 1500) + "; };"},
}

func runPathological(selftest bool) int {
	tmp, err := os.MkdirTemp("", "parser_patho_*")
	if err != nil {
		fail("临时目录失败: %v", err)
	}
	defer os.RemoveAll(tmp)
	for _, s := range pathologicalSamples {
		if err := os.WriteFile(filepath.Join(tmp, s.name+".c"), []byte(s.src), 0644); err != nil {
			fail("病态样本写入失败: %v", err)
		}
	}
	files := listCFiles(tmp)
	if len(files) != len(pathologicalSamples) {
		fail("病态样本数不符: %d != %d", len(files), len(pathologicalSamples))
	}
	rustOuts := rustAstDumpBatch(files)
	if selftest {
		// J9 埋雷：把"decl_suffix_1300"的 Rust 侧结果改成 ok=true——
		// "同等拒绝"断言必须红
		for i, f := range files {
			if filepath.Base(f) == "decl_suffix_1300.c" {
				rustOuts[i] = []byte(`{"ok": true, "parse_error_count": 0, "ast": {}, "parse_errors": [], "stall_count": 0}`)
			}
		}
		fmt.Println("parser_diff: selftest 已注入差异（decl_suffix_1300 篡改为 ok=true）")
	}
	moonDir := moonDump(tmp)
	failures := 0
	for i, f := range files {
		name := filepath.Base(f)
		rustNorm := canonicalize(rustOuts[i])
		stem := stemOf(f)
		moonRaw, err := os.ReadFile(filepath.Join(moonDir, stem+".c.ast.json"))
		if err != nil {
			fail("MoonBit 侧输出缺失: %s (%v)", name, err)
		}
		moonNorm := canonicalize(moonRaw)
		// E3 断言 1：两侧同等拒绝（Rust P1 修复后同样诊断拒绝——若
		// 某侧 ok=true 且 parse_errors 空，即"同等拒绝"被破坏）
		for _, side := range []struct{ tag string; norm []byte }{{"rust", rustNorm}, {"moon", moonNorm}} {
			if bytes.Contains(side.norm, []byte(`"ok": true`)) &&
				bytes.Contains(side.norm, []byte(`"parse_error_count": 0`)) {
				failures++
				fmt.Printf("E3-REJECT-MISS %s（%s 侧意外接受）\n", name, side.tag)
			}
		}
		// E3 断言 2：错误码序列 + 首错位置 + 完整诊断面一致
		if !bytes.Equal(rustNorm, moonNorm) {
			failures++
			fmt.Printf("E3-DIFF %s\n  rust: %s\n  moon: %s\n", name,
				preview(rustNorm), preview(moonNorm))
			continue
		}
	}
	if failures > 0 {
		fmt.Printf("parser_diff --pathological: FAIL——%d 处（12 样本同等拒绝锚）\n", failures)
		return 1
	}
	fmt.Printf("parser_diff --pathological: PASS——12 病态样本同等拒绝（错误码序列+位置一致，两侧无崩溃）\n")
	return 0
}

// ---------------------------------------------------------------------------
// E4 合法深嵌套反向锚（crash_regression_tests.rs:776 形状）
// ---------------------------------------------------------------------------

func runLegalDeep(selftest bool) int {
	samples := []struct {
		name string
		src  string
	}{
		{"paren_30_add_100", "int main() { return " + rep("(", 30) + "1" +
			rep(" + 1", 100) + rep(")", 30) + "; }"},
		{"decl_suffix_1200", "int a" + rep("[1]", 1200) + "; int main() { return 0; }"},
	}
	tmp, err := os.MkdirTemp("", "parser_legal_*")
	if err != nil {
		fail("临时目录失败: %v", err)
	}
	defer os.RemoveAll(tmp)
	for _, s := range samples {
		if err := os.WriteFile(filepath.Join(tmp, s.name+".c"), []byte(s.src), 0644); err != nil {
			fail("E4 样本写入失败: %v", err)
		}
	}
	files := listCFiles(tmp)
	rustOuts := rustAstDumpBatch(files)
	if selftest {
		rustOuts[0] = []byte(`{"ok": false, "parse_error_count": 1, "ast": null, "parse_errors": [{"code":1006,"line":1,"column":1,"message":"x"}], "stall_count": 0}`)
		fmt.Println("parser_diff: selftest 已注入差异（E4 样本 0 篡改为拒绝）")
	}
	moonDir := moonDump(tmp)
	failures := 0
	for i, f := range files {
		name := filepath.Base(f)
		rustNorm := canonicalize(rustOuts[i])
		stem := stemOf(f)
		moonRaw, err := os.ReadFile(filepath.Join(moonDir, stem+".c.ast.json"))
		if err != nil {
			fail("MoonBit 侧输出缺失: %s (%v)", name, err)
		}
		moonNorm := canonicalize(moonRaw)
		for _, side := range []struct{ tag string; norm []byte }{{"rust", rustNorm}, {"moon", moonNorm}} {
			if !bytes.Contains(side.norm, []byte(`"ok": true`)) {
				failures++
				fmt.Printf("E4-MUST-SUCCEED %s（%s 侧意外拒绝）: %s\n", name, side.tag, preview(side.norm))
			}
		}
		if !bytes.Equal(rustNorm, moonNorm) {
			failures++
			fmt.Printf("E4-DIFF %s\n  rust: %s\n  moon: %s\n", name, preview(rustNorm), preview(moonNorm))
		}
	}
	if failures > 0 {
		fmt.Printf("parser_diff --legal-deep: FAIL——%d 处（E4 反向锚）\n", failures)
		return 1
	}
	fmt.Printf("parser_diff --legal-deep: PASS——合法深嵌套样本两侧成功且 AST 一致\n")
	return 0
}

// ---------------------------------------------------------------------------
// 两侧出口
// ---------------------------------------------------------------------------

// rustAstDumpBatch：单 serve 进程批量发 ast.dump 请求（JSONL 流式），
// 返回与 files 同序的 result JSON。
func rustAstDumpBatch(files []string) [][]byte {
	cli := filepath.Join("native", "target", "release", "vitro_cli.exe")
	if _, err := os.Stat(cli); err != nil {
		cli = filepath.Join("native", "target", "release", "vitro_cli")
		if _, err := os.Stat(cli); err != nil {
			fail("Rust CLI 不存在（先 cargo build --release --bin vitro_cli）: %v", err)
		}
	}
	cmd := exec.Command(cli, "serve")
	stdin, err := cmd.StdinPipe()
	if err != nil {
		fail("serve stdin 管道失败: %v", err)
	}
	stdout, err := cmd.StdoutPipe()
	if err != nil {
		fail("serve stdout 管道失败: %v", err)
	}
	cmd.Stderr = os.Stderr
	if err := cmd.Start(); err != nil {
		fail("serve 启动失败: %v", err)
	}
	go func() {
		w := bufio.NewWriter(stdin)
		enc := json.NewEncoder(w)
		for i, f := range files {
			src, err := os.ReadFile(f)
			if err != nil {
				fail("读样本失败 %s: %v", f, err)
			}
			_ = enc.Encode(map[string]any{
				"id":     i + 1,
				"method": "ast.dump",
				"params": map[string]any{"source": string(src), "filename": filepath.Base(f)},
			})
		}
		w.Flush()
		stdin.Close()
	}()
	outBy := make([][]byte, len(files))
	sc := bufio.NewScanner(stdout)
	sc.Buffer(make([]byte, 64*1024*1024), 64*1024*1024)
	seen := 0
	for sc.Scan() {
		line := sc.Bytes()
		if len(line) == 0 {
			continue
		}
		var resp struct {
			ID     int             `json:"id"`
			OK     bool            `json:"ok"`
			Result json.RawMessage `json:"result"`
			Error  json.RawMessage `json:"error"`
		}
		if err := json.Unmarshal(line, &resp); err != nil {
			fail("serve 响应解析失败: %v (%s)", err, preview(line))
		}
		if resp.ID < 1 || resp.ID > len(files) {
			fail("serve 响应 id 越界: %d", resp.ID)
		}
		if !resp.OK || len(resp.Result) == 0 {
			fail("serve 请求失败（id=%d）: %s", resp.ID, preview(resp.Error))
		}
		outBy[resp.ID-1] = append([]byte(nil), resp.Result...)
		seen++
		if seen == len(files) {
			break
		}
	}
	if err := cmd.Wait(); err != nil {
		// serve 在 stdin 关闭后退出；非零退出码在此容忍（响应已收齐）
		_ = err
	}
	if seen != len(files) {
		fail("serve 响应不齐: %d/%d", seen, len(files))
	}
	return outBy
}

// moonDump：跑 MoonBit 侧 dump_ast，返回输出目录。
func moonDump(corpus string) string {
	out, err := os.MkdirTemp("", "parser_moon_*")
	if err != nil {
		fail("临时目录失败: %v", err)
	}
	// 注：调用方负责 defer 清理不适用于返回目录——泄漏一个临时目录由
	// 系统清理（差分工具的常规代价）；如需回收可用 moonOutDir 全局化
	cmd := exec.Command("moon", "run", "--target", "native", "cmd/dump_ast", "--", corpus, out)
	cmd.Dir = "moonbit"
	var buf bytes.Buffer
	cmd.Stdout = &buf
	cmd.Stderr = &buf
	if err := cmd.Run(); err != nil {
		fail("moon run cmd/dump_ast 失败: %v\n%s", err, buf.String())
	}
	return out
}

// ---------------------------------------------------------------------------
// 工具
// ---------------------------------------------------------------------------

func canonicalize(raw []byte) []byte {
	cmd := exec.Command("go", "run", "./scripts/canonicalize")
	cmd.Stdin = bytes.NewReader(raw)
	var stderr bytes.Buffer
	cmd.Stderr = &stderr
	out, err := cmd.Output()
	if err != nil {
		fail("canonicalize 失败: %v\nstderr: %s\ninput: %s", err, stderr.String(), preview(raw))
	}
	return out
}

func listCFiles(dir string) []string {
	entries, err := os.ReadDir(dir)
	if err != nil {
		fail("读目录失败 %s: %v", dir, err)
	}
	var files []string
	for _, e := range entries {
		if !e.IsDir() && filepath.Ext(e.Name()) == ".c" {
			files = append(files, filepath.Join(dir, e.Name()))
		}
	}
	return files
}

func stemOf(path string) string {
	base := filepath.Base(path)
	ext := filepath.Ext(base)
	return base[:len(base)-len(ext)]
}

func rep(s string, n int) string {
	b := bytes.Repeat([]byte(s), n)
	return string(b)
}

func preview(b []byte) string {
	if len(b) > 220 {
		return string(b[:220]) + "..."
	}
	return string(b)
}

func fail(format string, args ...any) {
	fmt.Fprintf(os.Stderr, "parser_diff: "+format+"\n", args...)
	os.Exit(2)
}

var _ = regexp.MustCompile // 保留 regexp 引用（未来按行 diff 用）
