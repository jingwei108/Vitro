// lexer_diff：S2 词法差分对拍驱动（Go，零第三方依赖，fail loud）。
//
// 管线：语料目录（.c 文件）→
//   ① Rust oracle 批量 dump：native/target/release/vitro_cli dump-tokens <dir> --out A --raw --pp
//   ② MoonBit 侧批量 dump：moon -C moonbit run --target native cmd/dump_tokens -- <dir> B both
//   ③ 逐文件逐字节 diff（A/*.l1.tsv ↔ B/*.l1.tsv、A/*.l2.tsv ↔ B/*.l2.tsv）
// 任一文件缺失或内容差异即 exit 1 并打印首差异上下文——不静默跳过。
//
// J9 埋雷（--selftest）：对一侧 TSV 注入单字节差异，驱动必须报红（护栏可触发性）。
//
// 用法（仓库根）：
//
//	go run ./scripts/lexer_diff <corpus_dir>            # 全量对拍
//	go run ./scripts/lexer_diff <corpus_dir> --selftest # 先证红再退出
package main

import (
	"bytes"
	"fmt"
	"os"
	"os/exec"
	"path/filepath"
	"sort"
)

const rustCLI = "native/target/release/vitro_cli.exe"
const selftestMutate = "count=" // 注入点：篡改尾行计数（必然逐字节差异）

func main() {
	if len(os.Args) < 2 {
		fmt.Fprintln(os.Stderr, "用法: go run ./scripts/lexer_diff <corpus_dir> [--selftest]")
		os.Exit(2)
	}
	corpus, err := filepath.Abs(os.Args[1])
	must(err, "解析语料绝对路径")
	selftest := len(os.Args) > 2 && os.Args[2] == "--selftest"
	if _, err := os.Stat("go.mod"); err != nil {
		fmt.Fprintln(os.Stderr, "lexer_diff: 必须在仓库根目录运行")
		os.Exit(2)
	}

	rustOut, err := os.MkdirTemp("", "lexdiff_rust_*")
	must(err, "创建 Rust 输出目录")
	defer os.RemoveAll(rustOut)
	mbOut, err := os.MkdirTemp("", "lexdiff_mb_*")
	must(err, "创建 MoonBit 输出目录")
	defer os.RemoveAll(mbOut)

	// ① Rust oracle
	runOrFail(exec.Command(rustCLI, "dump-tokens", corpus, "--out", rustOut, "--raw", "--pp"), rustCLI)
	// ② MoonBit
	runOrFail(exec.Command("moon", "-C", "moonbit", "run", "--target", "native",
		"cmd/dump_tokens", "--", corpus, mbOut, "both"), "moon run cmd/dump_tokens")

	// J9：对 MoonBit 侧第一个 TSV 注入差异，证明驱动会红
	if selftest {
		mutated := mutateFirstTSV(mbOut)
		fmt.Printf("[selftest] 已注入差异: %s（随后对拍必须 FAIL）\n", mutated)
	}

	// ③ 逐文件比对
	files := listTSV(rustOut)
	if len(files) == 0 {
		fmt.Fprintln(os.Stderr, "lexer_diff: Rust 侧产物为空——语料目录无 .c 文件或 dump 失败")
		os.Exit(1)
	}
	sort.Strings(files)
	mbFiles := listTSV(mbOut)
	mbSet := map[string]bool{}
	for _, f := range mbFiles {
		mbSet[f] = true
	}
	failures := 0
	for _, name := range files {
		if !mbSet[name] {
			fmt.Printf("DIFF 缺失: MoonBit 侧无 %s\n", name)
			failures++
			continue
		}
		a, errA := os.ReadFile(filepath.Join(rustOut, name))
		b, errB := os.ReadFile(filepath.Join(mbOut, name))
		if errA != nil || errB != nil {
			fmt.Printf("DIFF 读取失败: %s (%v / %v)\n", name, errA, errB)
			failures++
			continue
		}
		if !bytes.Equal(a, b) {
			fmt.Printf("DIFF %s:\n", name)
			printFirstDiff(a, b)
			failures++
		}
	}
	// 反向缺失：MoonBit 有 Rust 无（语料外产物泄漏）
	rustSet := map[string]bool{}
	for _, f := range files {
		rustSet[f] = true
	}
	for _, f := range mbFiles {
		if !rustSet[f] {
			fmt.Printf("DIFF 多余: Rust 侧无 %s\n", f)
			failures++
		}
	}
	if failures > 0 {
		fmt.Printf("lexer_diff: FAIL——%d 处差异（语料 %s）\n", failures, corpus)
		os.Exit(1)
	}
	fmt.Printf("lexer_diff: PASS——%d 个 TSV 逐字节一致（语料 %s）\n", len(files), corpus)
}

func runOrFail(cmd *exec.Cmd, label string) {
	out, err := cmd.CombinedOutput()
	if err != nil {
		fmt.Fprintf(os.Stderr, "lexer_diff: %s 执行失败: %v\n%s\n", label, err, out)
		os.Exit(1)
	}
}

func listTSV(dir string) []string {
	entries, err := os.ReadDir(dir)
	if err != nil {
		must(err, "读取输出目录")
	}
	var out []string
	for _, e := range entries {
		if filepath.Ext(e.Name()) == ".tsv" {
			out = append(out, e.Name())
		}
	}
	return out
}

func mutateFirstTSV(dir string) string {
	files := listTSV(dir)
	if len(files) == 0 {
		fmt.Fprintln(os.Stderr, "selftest: 无 TSV 可注入——前置步骤已失败")
		os.Exit(1)
	}
	sort.Strings(files)
	p := filepath.Join(dir, files[0])
	b, err := os.ReadFile(p)
	must(err, "selftest 读取")
	i := bytes.Index(b, []byte(selftestMutate))
	if i < 0 {
		fmt.Fprintln(os.Stderr, "selftest: 未找到注入点")
		os.Exit(1)
	}
	b[i+len(selftestMutate)] = '9' // count=N -> count=9（除非恰为 9，再兜底改下一字节）
	if b[i+len(selftestMutate)] == '9' && bytes.Contains(b[i:i+len(selftestMutate)+2], []byte("=9")) {
		b[i+len(selftestMutate)] = '7'
	}
	must(os.WriteFile(p, b, 0o644), "selftest 写回")
	return p
}

func printFirstDiff(a, b []byte) {
	la := bytes.Split(a, []byte("\n"))
	lb := bytes.Split(b, []byte("\n"))
	n := len(la)
	if len(lb) > n {
		n = len(lb)
	}
	for i := 0; i < n; i++ {
		var x, y []byte
		if i < len(la) {
			x = la[i]
		}
		if i < len(lb) {
			y = lb[i]
		}
		if !bytes.Equal(x, y) {
			fmt.Printf("  行 %d:\n    rust:    %s\n    moonbit: %s\n", i+1, x, y)
			return
		}
	}
}

func must(err error, what string) {
	if err != nil {
		fmt.Fprintf(os.Stderr, "lexer_diff: %s 失败: %v\n", what, err)
		os.Exit(1)
	}
}
