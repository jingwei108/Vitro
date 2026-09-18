package main

// 三层契约验证 CI 脚本（Phase F）。
//
// 目标：
//  1. 独立运行 Phase A/B/C/E 测试，确保三层契约在 CI 中被显式验证。
//  2. 生成综合报告，记录每层契约的通过状态。
//  3. 检查 *_FAILURES.md 的一致性：
//     - 文档标记 KNOWN_FAILURE / KNOWN_DIVERGENCE 的测试现在通过 → hard 报错；
//     - 测试失败但文档无记录 → soft 提醒（自由文本无法精确匹配，硬失败会
//       持续误报；精确双向对账由 vitro_e2e.rs 的 KNOWN_* 常量机制闭环承担）。
//  4. 报告输出到 reports/three_tier_report.md，作为 CI artifact 上传。
//
// D5 后续批次第三站（2026-09-18）：ci_three_tier_check.py → Go。双轨对账
// 口径：9 套件 PASS/FAIL 判定、hard/soft 一致性 issue 清单、退出码双侧一致。
// J9 证红：向任一 FAILURES.md 注入假 KNOWN_FAILURE 条目 → hard / exit 1
//（Python 版埋雷先例，台账 `脚本埋雷验证记录.md`）。
//
// 使用方式：go run ./scripts/ci_three_tier_check

import (
	"bytes"
	"fmt"
	"os"
	"os/exec"
	"regexp"
	"strings"
	"time"
)

// ─── 配置 ────────────────────────────────────────────────────────────────────

const (
	nativeDir  = "native"
	reportsDir = "reports"
	testsDir   = nativeDir + "/tests"
)

type tierTest struct {
	phase      string
	testFile   string
	failuresMD []string
}

var tierTests = []tierTest{
	{"Phase A", "host_contract_tests", []string{"HOST_CONTRACT_FAILURES.md"}},
	{"Phase B", "bytecode_libc_consistency", []string{"BYTECODE_LIBC_FAILURES.md"}},
	{"Phase C", "differential_stress", []string{"DIFFERENTIAL_FAILURES.md"}},
	{"Phase E", "fuzz_stress_test", []string{"FUZZ_FAILURES.md"}},
	{"K&R / E2E / LeetCode / C++", "vitro_e2e", []string{"KR_FAILURES.md", "E2E_FAILURES.md", "LEETCODE_FAILURES.md", "CPP_FAILURES.md"}},
	{"C++ Parser", "parser_cpp_unit_test", []string{"CPP_FAILURES.md"}},
	{"C++ TypeChecker", "typeck_cpp_unit_test", []string{"CPP_FAILURES.md"}},
	{"C++ BytecodeGen", "bytecode_gen_cpp_unit_test", []string{"CPP_FAILURES.md"}},
	{"C++ Dogfooding", "cpp_dogfooding_test", []string{"DOGFOODING_FAILURES.md"}},
}

// ─── 测试运行 ─────────────────────────────────────────────────────────────────

// normalizeNewlines CRLF → LF（对齐 Python universal newlines）。
func normalizeNewlines(s string) string {
	return strings.ReplaceAll(s, "\r\n", "\n")
}

type procResult struct {
	stdout, stderr string
	returncode     int
}

// runCargoTest 运行指定的 cargo integration test（--test-threads=1，
// Python 版口径：套件内串行——DLL 非线程安全实证的保守形态）。
func runCargoTest(testFile string) procResult {
	cmd := exec.Command("cargo", "test", "--test", testFile, "--", "--test-threads=1")
	cmd.Dir = nativeDir
	var out, errb bytes.Buffer
	cmd.Stdout, cmd.Stderr = &out, &errb
	err := cmd.Run()
	code := 0
	if err != nil {
		code = -1
		if ee, ok := err.(*exec.ExitError); ok {
			code = ee.ExitCode()
		}
	}
	// 行尾归一（对齐 Python text=True 的 universal newlines）：cargo 在
	// Windows 输出 CRLF，嵌入报告前归一为 LF，避免混合行尾。
	return procResult{normalizeNewlines(out.String()), normalizeNewlines(errb.String()), code}
}

type testStats struct {
	run, passed, failed, ignored int
	failedNames                  []string
	found                        bool
}

var (
	reResultLine = regexp.MustCompile(
		`test result:\s+(ok|FAILED)\.\s+(\d+)\s+passed;\s+(\d+)\s+failed;\s+(\d+)\s+ignored`)
	reFailedName = regexp.MustCompile(`(?m)^test\s+(\S+)\s+\.\.\.\s+FAILED`)
)

// parseTestOutput 从 cargo test 输出中提取统计信息。
// E-P0-2："found" 标志——cargo 编译失败/依赖拉取失败时输出中没有任何
// "test result:" 行，此前返回全 0 统计 → failed==0 → 被误判 PASS。
func parseTestOutput(output string) testStats {
	var s testStats
	if m := reResultLine.FindStringSubmatch(output); m != nil {
		s.found = true
		s.passed = atoi(m[2])
		s.failed = atoi(m[3])
		s.ignored = atoi(m[4])
		s.run = s.passed + s.failed
	}
	for _, m := range reFailedName.FindAllStringSubmatch(output, -1) {
		s.failedNames = append(s.failedNames, m[1])
	}
	return s
}

func atoi(s string) int {
	n := 0
	for _, c := range s {
		if c < '0' || c > '9' {
			return 0
		}
		n = n*10 + int(c-'0')
	}
	return n
}

// ─── 文档一致性检查 ───────────────────────────────────────────────────────────

type mdEntry struct {
	title  string
	status string // FIXED / KNOWN / DIVERGENCE
}

var (
	reHTMLComment = regexp.MustCompile(`(?s)<!--.*?-->`)
	reH2          = regexp.MustCompile(`(?m)^##\s+`)
	reH3          = regexp.MustCompile(`(?m)^###\s+`)
	reH3Title     = regexp.MustCompile(`(?m)^###\s+(.+)$`)
	reStrikethru  = regexp.MustCompile(`^~~(.+?)~~\s*(?:→\s*(?:已修复|FIXED))?`)
)

// extractMDStatus 从 *_FAILURES.md 中提取状态信息。
func extractMDStatus(path string) (entries []mdEntry, missing bool) {
	b, err := os.ReadFile(path)
	if err != nil {
		return nil, true
	}
	// 先移除 HTML 注释（含多行），避免占位符 <case_name> 被误判
	content := reHTMLComment.ReplaceAllString(string(b), "")

	parts := reH2.Split(content, -1)
	for _, part := range parts[1:] {
		lines := strings.Split(part, "\n")
		if len(lines) == 0 {
			continue
		}
		sectionTitle := strings.TrimSpace(lines[0])
		sectionText := strings.Join(lines[1:], "\n")

		isKnownFailure := strings.Contains(sectionTitle, "KNOWN_FAILURE") || strings.Contains(sectionTitle, "已知失败")
		isDivergence := strings.Contains(sectionTitle, "KNOWN_DIVERGENCE") || strings.Contains(sectionTitle, "已知偏差")

		for _, loc := range reH3Title.FindAllStringSubmatchIndex(sectionText, -1) {
			// title 取捕获组（.+) 而非全匹配——全匹配含 "### " 前缀；
			// $ 锚定行尾（\n 前），. 可吞 \r，由 lineEndTrim 剥除。
			title := strings.TrimSpace(lineEndTrim(sectionText[loc[2]:loc[3]]))
			if title == "" {
				continue
			}
			// 标题本身带删除线 → 已修复
			if m := reStrikethru.FindStringSubmatch(title); m != nil {
				entries = append(entries, mdEntry{strings.TrimSpace(m[1]), "FIXED"})
				continue
			}
			// 检查该标题所在段落（到下一个三级标题）是否包含"已修复"字样
			paragraphStart := loc[1]
			paragraphEnd := len(sectionText)
			if next := reH3.FindStringIndex(sectionText[paragraphStart:]); next != nil {
				paragraphEnd = paragraphStart + next[0]
			}
			paragraph := sectionText[paragraphStart:paragraphEnd]
			if strings.Contains(paragraph, "已修复") || strings.Contains(paragraph, "FIXED") {
				entries = append(entries, mdEntry{title, "FIXED"})
				continue
			}
			if isDivergence {
				entries = append(entries, mdEntry{title, "DIVERGENCE"})
			} else if isKnownFailure {
				entries = append(entries, mdEntry{title, "KNOWN"})
			}
		}
	}
	return entries, false
}

// lineEndTrim 去掉行尾 \r（title 匹配 `(.+)$` 的 $ 在 \n 前，可能残留 \r）。
func lineEndTrim(s string) string {
	return strings.TrimSuffix(s, "\r")
}

type tierResult struct {
	phase       string
	testFile    string
	failuresMD  []string
	passed      bool
	testsRun    int
	testsPassed int
	testsFailed int
	stdout      string
	stderr      string
	failedTests []string
}

// checkConsistency 检查测试结果与 *_FAILURES.md 的一致性。
// E-P0-3：hard（计入 CI 退出码）= 确定性不一致——文档声明 KNOWN_FAILURE 但
// 测试现在全部通过、失败记录文件缺失；soft = 测试有失败时的记录提醒。
func checkConsistency(r tierResult) (hard, soft []string) {
	for _, failuresMD := range r.failuresMD {
		entries, missing := extractMDStatus(testsDir + "/" + failuresMD)
		if missing {
			hard = append(hard, "缺少失败记录文件: "+failuresMD)
			continue
		}
		// KNOWN_DIVERGENCE（设计决策导致的偏差）不视为需要修复的故障，
		// 测试通过是正常的。
		if r.passed {
			var known []mdEntry
			for _, e := range entries {
				if e.status == "KNOWN" {
					known = append(known, e)
				}
			}
			if len(known) > 0 {
				var titles []string
				for _, e := range known {
					titles = append(titles, truncate(e.title, 40))
				}
				hard = append(hard, fmt.Sprintf(
					"Tests all passed, but %s still has %d un-fixed KNOWN entries: %s"+
						"（KNOWN_FAILURE 已通过，请更新文档标记为已修复）",
					failuresMD, len(known), strings.Join(titles, ", ")))
			}
		} else {
			fixedCount := 0
			for _, e := range entries {
				if e.status == "FIXED" {
					fixedCount++
				}
			}
			soft = append(soft, fmt.Sprintf(
				"Tests have failures. Ensure all are recorded in %s (currently %d FIXED records)",
				failuresMD, fixedCount))
		}
	}
	return hard, soft
}

func truncate(s string, n int) string {
	r := []rune(s)
	if len(r) <= n {
		return s
	}
	return string(r[:n])
}

// ─── 报告生成 ─────────────────────────────────────────────────────────────────

func generateReport(results []tierResult, consistency map[string][]string) string {
	var lines []string
	add := func(format string, args ...any) { lines = append(lines, fmt.Sprintf(format, args...)) }

	add("# 三层契约验证报告（Three Tier Test Report）")
	add("")
	add("生成时间: %s", time.Now().Format(time.RFC3339))
	add("")
	add("> 本报告由 CI 自动生成，对应 Phase F 要求。")
	add("")
	add("## 摘要")
	add("")
	add("| 阶段 | 测试文件 | 状态 | 通过 | 失败 | 忽略 |")
	add("|------|----------|------|------|------|------|")
	for _, r := range results {
		status := "✅ PASS"
		if !r.passed {
			status = "❌ FAIL"
		}
		add("| %s | `%s` | %s | %d | %d | %d |",
			r.phase, r.testFile, status, r.testsPassed, r.testsFailed,
			r.testsRun-r.testsPassed-r.testsFailed)
	}

	add("")
	add("## 一致性检查")
	add("")
	hasIssues := false
	for _, r := range results {
		issues := consistency[r.phase]
		if len(issues) == 0 {
			continue
		}
		hasIssues = true
		add("### %s", r.phase)
		add("")
		for _, issue := range issues {
			add("- ⚠️ %s", issue)
		}
		add("")
	}
	if !hasIssues {
		add("✅ 所有失败记录文档与测试结果一致。")
		add("")
	}

	add("## 详细输出")
	add("")
	for _, r := range results {
		add("### %s: %s", r.phase, r.testFile)
		add("")
		if len(r.failedTests) > 0 {
			add("**失败的测试:**")
			for _, name := range r.failedTests {
				add("- `%s`", name)
			}
			add("")
		}
		add("```")
		// 截取最后的 800 字符，避免报告过长（runes 截断对齐 Python 字符语义）
		add("%s", tailRunes(r.stdout+"\n"+r.stderr, 800))
		add("```")
		add("")
	}
	return strings.Join(lines, "\n") + "\n"
}

// ─── 主流程 ───────────────────────────────────────────────────────────────────

func main() {
	if err := os.MkdirAll(reportsDir, 0o755); err != nil {
		fmt.Fprintln(os.Stderr, "错误: 创建 reports/ 失败:", err)
		os.Exit(2)
	}

	var results []tierResult
	consistency := map[string][]string{}
	anyHardIssue := false

	fmt.Println(strings.Repeat("=", 60))
	fmt.Println("Three Tier Verification Start")
	fmt.Println(strings.Repeat("=", 60))

	// 同一 test_file 只跑一次（Python 版缓存口径；当前清单无重复，防御性保留）
	cache := map[string]procResult{}
	for _, tt := range tierTests {
		fmt.Printf("\n>> %s: cargo test --test %s\n", tt.phase, tt.testFile)
		proc, ok := cache[tt.testFile]
		if !ok {
			proc = runCargoTest(tt.testFile)
			cache[tt.testFile] = proc
		}
		stats := parseTestOutput(proc.stdout + proc.stderr)

		// E-P0-2：cargo 本身失败或解析不到 "test result:" 行时，不得误判 PASS。
		if !stats.found {
			fmt.Println("   [ERROR] 未能从 cargo test 输出解析到 'test result:' 行 —— cargo 可能编译失败")
			fmt.Printf("   [ERROR] returncode=%d\n", proc.returncode)
			tail := tailRunes(proc.stderr+"\n"+proc.stdout, 600)
			if strings.TrimSpace(tail) != "" {
				fmt.Println("   [ERROR] 输出尾部:")
				var tl []string
				for _, l := range strings.Split(strings.TrimSpace(tail), "\n") {
					tl = append(tl, l)
				}
				if len(tl) > 12 {
					tl = tl[len(tl)-12:]
				}
				for _, l := range tl {
					fmt.Printf("      %s\n", l)
				}
			}
		}
		passed := proc.returncode == 0 && stats.found && stats.failed == 0
		results = append(results, tierResult{
			phase: tt.phase, testFile: tt.testFile, failuresMD: tt.failuresMD,
			passed: passed, testsRun: stats.run, testsPassed: stats.passed,
			testsFailed: stats.failed, stdout: proc.stdout, stderr: proc.stderr,
			failedTests: stats.failedNames,
		})

		status := "[PASS]"
		if !passed {
			status = "[FAIL]"
		}
		fmt.Printf("   %s — %d passed, %d failed\n", status, stats.passed, stats.failed)

		hard, soft := checkConsistency(results[len(results)-1])
		consistency[tt.phase] = append(append([]string{}, hard...), soft...)
		if len(hard) > 0 {
			anyHardIssue = true
			for _, issue := range hard {
				fmt.Printf("   [ERROR] %s\n", issue)
			}
		}
		for _, issue := range soft {
			fmt.Printf("   [WARN] %s\n", issue)
		}
	}

	report := generateReport(results, consistency)
	reportPath := reportsDir + "/three_tier_report.md"
	if err := os.WriteFile(reportPath, []byte(report), 0o644); err != nil {
		fmt.Fprintln(os.Stderr, "错误: 写报告失败:", err)
		os.Exit(2)
	}
	fmt.Printf("\n[REPORT] Generated: %s\n", reportPath)

	// 最终判定（E-P0-3：一致性 hard 问题与测试失败同等阻塞 CI）
	allPassed := true
	for _, r := range results {
		if !r.passed {
			allPassed = false
		}
	}
	if allPassed && !anyHardIssue {
		fmt.Println("\n[SUCCESS] All three tier tests passed!")
		return
	}
	if !allPassed {
		fmt.Println("\n[FAILED] Some three tier tests failed. See report and *_FAILURES.md.")
	}
	if anyHardIssue {
		fmt.Println("[FAILED] 一致性检查存在 hard 问题（文档与测试结果矛盾），见上方 [ERROR]。")
	}
	os.Exit(1)
}

// tailRunes 取字符串末尾 n 个 rune。
func tailRunes(s string, n int) string {
	r := []rune(s)
	if len(r) <= n {
		return s
	}
	return string(r[len(r)-n:])
}
