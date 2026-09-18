package main

// 工程健康度看板脚本。
//
// 目标：
//  1. 统计项目关键健康度指标：超大文件、TODO/FIXME/HACK、unwrap/expect、
//     失败记录活跃问题、Shadow Verification 匹配率。
//  2. 生成 reports/engineering_health.md，供维护者定期 review。
//
// D5 后续批次第四站（2026-09-18）：engineering_health.py → Go。本脚本是
// 报告生成器（无红绿判定，无 J9 埋雷义务——台账四判定型脚本不含它）；
// 双轨对账口径：摘要数值、Top 表格逐项一致（tie 行的先后属遍历序差异，
// 数值集合一致即对齐）。
//
// 使用方式：go run ./scripts/engineering_health

import (
	"encoding/json"
	"fmt"
	"io/fs"
	"os"
	"os/exec"
	"path/filepath"
	"regexp"
	"sort"
	"strings"
	"time"

	"vitro/scripts/internal/capi"
)

var (
	projectRoot     = capi.ProjectRoot()
	reportsDir      = filepath.Join(projectRoot, "reports")
	nativeSrc       = filepath.Join(projectRoot, "native", "src")
	nativeCrates    = filepath.Join(projectRoot, "native", "crates")
	nativeCodeDirs  = []string{nativeSrc, nativeCrates}
	shadowReportDir = filepath.Join(projectRoot, "native", "tests", "shadow_verification", "reports")
)

var failuresFiles = []string{
	"native/tests/FUZZ_FAILURES.md",
	"native/tests/HOST_CONTRACT_FAILURES.md",
	"native/tests/BYTECODE_LIBC_FAILURES.md",
	"native/tests/DIFFERENTIAL_FAILURES.md",
	// 注意：根目录 GOLDEN_FAILURES.md 已迁移至 cases_golden/ 下
	"native/tests/KR_FAILURES.md",
	"native/tests/E2E_FAILURES.md",
	"native/tests/LEETCODE_FAILURES.md",
	"native/tests/CPP_FAILURES.md",
	"native/tests/DOGFOODING_FAILURES.md",
	"native/tests/cases_golden/GOLDEN_FAILURES.md",
}

const topN = 20

// ─── 工具函数 ─────────────────────────────────────────────────────────────────

// countLines 统计文件非空行数。
func countLines(path string) int {
	b, err := os.ReadFile(path)
	if err != nil {
		return 0
	}
	n := 0
	for _, line := range strings.Split(string(b), "\n") {
		if strings.TrimSpace(stripCR(line)) != "" {
			n++
		}
	}
	return n
}

func stripCR(s string) string { return strings.TrimSuffix(s, "\r") }

var excludeDirs = map[string]bool{"target": true, ".dart_tool": true, "build": true}
var ignoredNames = map[string]bool{
	"frb_generated.rs": true, "frb_generated.dart": true,
	"frb_generated.io.dart": true, "frb_generated.web.dart": true,
}

// gatherFiles 递归收集指定后缀文件，排除生成目录与 target。
func gatherFiles(roots []string, suffix string) []string {
	var files []string
	for _, r := range roots {
		filepath.WalkDir(r, func(p string, d fs.DirEntry, err error) error {
			if err != nil {
				return nil
			}
			if d.IsDir() {
				if excludeDirs[d.Name()] {
					return filepath.SkipDir
				}
				return nil
			}
			if ignoredNames[d.Name()] {
				return nil
			}
			if strings.HasSuffix(p, "."+suffix) {
				files = append(files, p)
			}
			return nil
		})
	}
	return files
}

type fileCount struct {
	rel   string
	count int
}

func topFilesByLines(roots []string, suffix string, n int) []fileCount {
	var out []fileCount
	for _, p := range gatherFiles(roots, suffix) {
		out = append(out, fileCount{relOf(p), countLines(p)})
	}
	sort.SliceStable(out, func(i, j int) bool { return out[i].count > out[j].count })
	if len(out) > n {
		out = out[:n]
	}
	return out
}

// relOf 输出平台原生分隔符路径（对齐 Python str(Path)：Windows 为 \）。
func relOf(p string) string {
	r, err := filepath.Rel(projectRoot, p)
	if err != nil {
		return p
	}
	return r
}

var (
	reTODO   = regexp.MustCompile(`//\s*(TODO|FIXME|HACK)`)
	reUnwrap = regexp.MustCompile(`\b(unwrap\(\)|expect\()`)
)

func countPatternInFiles(roots []string, suffix string, re *regexp.Regexp) (int, []fileCount) {
	total := 0
	var perFile []fileCount
	for _, p := range gatherFiles(roots, suffix) {
		b, err := os.ReadFile(p)
		if err != nil {
			continue
		}
		n := len(re.FindAllString(string(b), -1))
		if n > 0 {
			total += n
			perFile = append(perFile, fileCount{relOf(p), n})
		}
	}
	return total, perFile
}

var (
	reCfgTest  = regexp.MustCompile(`#\[cfg\(test\)\]`)
	reTestAttr = regexp.MustCompile(`^#\[test\]$`)
)

// countProductionUnwrapExpect 统计生产代码中的 unwrap/expect 数量。
// 排除 #[cfg(test)] 模块与 #[test] 标注的测试函数（brace 深度状态机，
// 逐行复刻 Python 版口径）。
func countProductionUnwrapExpect() (int, []fileCount) {
	perFile := map[string]int{}
	order := []string{}
	total := 0
	for _, p := range gatherFiles(nativeCodeDirs, "rs") {
		if filepath.Base(p) == "frb_generated.rs" {
			continue
		}
		b, err := os.ReadFile(p)
		if err != nil {
			continue
		}
		lines := strings.Split(string(b), "\n")
		inTestMod, inTestFn := false, false
		modDepth, fnDepth, localCount := 0, 0, 0
		for _, raw := range lines {
			line := stripCR(raw)
			stripped := strings.TrimSpace(line)
			if reCfgTest.MatchString(stripped) {
				inTestMod = true
				modDepth = 0
				continue
			}
			if inTestMod {
				modDepth += strings.Count(stripped, "{") - strings.Count(stripped, "}")
				if modDepth < 0 {
					inTestMod = false
				}
				continue
			}
			if reTestAttr.MatchString(stripped) {
				inTestFn = true
				fnDepth = 0
				continue
			}
			if inTestFn {
				fnDepth += strings.Count(stripped, "{") - strings.Count(stripped, "}")
				if fnDepth < 0 {
					inTestFn = false
				}
				continue
			}
			if reUnwrap.MatchString(line) {
				localCount++
			}
		}
		if localCount > 0 {
			total += localCount
			rel := relOf(p)
			if _, seen := perFile[rel]; !seen {
				order = append(order, rel)
			}
			perFile[rel] += localCount
		}
	}
	var out []fileCount
	for _, rel := range order {
		out = append(out, fileCount{rel, perFile[rel]})
	}
	return total, out
}

var (
	reActiveSection  = regexp.MustCompile(`(?i)^#{2}\s+.*?(KNOWN_FAILURE|KNOWN_DIVERGENCE|KNOWN_LIMITATION)`)
	reEntryStart     = regexp.MustCompile(`^#{3}\s+`)
	reSectionStart   = regexp.MustCompile(`^#{2}\s+`)
	reResolvedMarker = regexp.MustCompile(`(?i)已修复|FIXED|RESOLVED|不再失败|no longer fails`)
	reTableDivider   = regexp.MustCompile(`^\|[-:|\s]+\|$`)
)

// countActiveFailureEntries 统计各失败记录文件中的活跃条目数。
// 规则：仅统计 KNOWN_* 二级标题下的 `### ` 条目与表格数据行；
// 含已修复/FIXED/RESOLVED/不再失败 字样的条目不计入。
func countActiveFailureEntries() (int, []fileCount) {
	var perFile []fileCount
	total := 0
	for _, rel := range failuresFiles {
		p := filepath.Join(projectRoot, filepath.FromSlash(rel))
		count := 0
		if b, err := os.ReadFile(p); err == nil {
			inActive, insideTable := false, false
			for _, raw := range strings.Split(string(b), "\n") {
				stripped := strings.TrimSpace(stripCR(raw))
				if reSectionStart.MatchString(stripped) {
					inActive = reActiveSection.MatchString(stripped)
					insideTable = false
					continue
				}
				if !inActive {
					continue
				}
				if reEntryStart.MatchString(stripped) {
					if !reResolvedMarker.MatchString(stripped) {
						count++
					}
					insideTable = false
					continue
				}
				if reTableDivider.MatchString(stripped) {
					insideTable = true
					continue
				}
				if insideTable && strings.HasPrefix(stripped, "|") && strings.HasSuffix(stripped, "|") {
					if !reResolvedMarker.MatchString(stripped) {
						count++
					}
					continue
				}
				if stripped != "" && !strings.HasPrefix(stripped, "|") {
					insideTable = false
				}
			}
		}
		perFile = append(perFile, fileCount{filepath.FromSlash(rel), count})
		total += count
	}
	return total, perFile
}

// readShadowMatchRate 读取 Shadow Verification 报告中的匹配率摘要。
func readShadowMatchRate() map[string]string {
	result := map[string]string{}

	// C++ 报告
	cppReport := filepath.Join(shadowReportDir, "cpp_shadow_report.json")
	result["C++"] = "N/A"
	if b, err := os.ReadFile(cppReport); err == nil {
		var arr []struct {
			DiffType string `json:"diff_type"`
		}
		if err := json.Unmarshal(b, &arr); err == nil {
			matched := 0
			for _, c := range arr {
				if c.DiffType == "match" {
					matched++
				}
			}
			result["C++"] = fmt.Sprintf("%d/%d", matched, len(arr))
		}
	}

	// C 报告：读取 shadow_data_latest.json（由 scripts/shadow_verify 同步更新）
	result["C"] = "N/A"
	if b, err := os.ReadFile(filepath.Join(shadowReportDir, "shadow_data_latest.json")); err == nil {
		var data struct {
			Summary struct {
				Total int `json:"total"`
			} `json:"summary"`
			Details []struct {
				DiffType string `json:"diff_type"`
			} `json:"details"`
		}
		if err := json.Unmarshal(b, &data); err == nil {
			// 与 README/AGENTS.md 对外口径一致：完全匹配 + Vitro 更优 + 已知差异均计入匹配
			matched := 0
			for _, c := range data.Details {
				switch c.DiffType {
				case "match", "vitro_better", "known_issue":
					matched++
				}
			}
			result["C"] = fmt.Sprintf("%d/%d", matched, data.Summary.Total)
		}
	}
	return result
}

func gitShortHead() string {
	out, err := exec.Command("git", "-C", projectRoot, "rev-parse", "--short", "HEAD").Output()
	if err != nil {
		return "unknown"
	}
	return strings.TrimSpace(string(out))
}

func gitDirty() bool {
	out, err := exec.Command("git", "-C", projectRoot, "status", "--short").Output()
	if err != nil {
		return false
	}
	return strings.TrimSpace(string(out)) != ""
}

// ─── 报告生成 ─────────────────────────────────────────────────────────────────

// sortByCountDesc 就地把 (文件, 数量) 按(count desc, 文件名 asc)排序，
// 并截取前 n。tie 按文件名字典序（Python Counter.most_common 的 tie 序是
// 遍历序——对账只比数值集合，见文件头注）。
func sortByCountDesc(fc []fileCount, n int) []fileCount {
	sort.SliceStable(fc, func(i, j int) bool {
		if fc[i].count != fc[j].count {
			return fc[i].count > fc[j].count
		}
		return fc[i].rel < fc[j].rel
	})
	if len(fc) > n {
		return fc[:n]
	}
	return fc
}

func generateReport() string {
	now := time.Now().Format("2006-01-02T15:04:05-0700")
	rev := gitShortHead()
	dirty := ""
	if gitDirty() {
		dirty = " (dirty)"
	}

	rustTop := topFilesByLines(nativeCodeDirs, "rs", topN)

	todoTotal, todoPerFile := countPatternInFiles(
		[]string{filepath.Join(projectRoot, "native")}, "rs", reTODO)
	unwrapTotal, unwrapPerFile := countPatternInFiles(nativeCodeDirs, "rs", reUnwrap)
	prodUnwrapTotal, prodUnwrapPerFile := countProductionUnwrapExpect()
	activeFailures, failuresPerFile := countActiveFailureEntries()
	shadowRates := readShadowMatchRate()

	var lines []string
	add := func(format string, args ...any) { lines = append(lines, fmt.Sprintf(format, args...)) }

	add("# Vitro 工程健康度看板")
	add("")
	add("> 生成时间: %s", now)
	add("> 基线提交: `%s%s`", rev, dirty)
	add("")
	add("## 摘要")
	add("")
	add("| 指标 | 数值 |")
	add("|------|------|")
	add("| Rust TODO/FIXME/HACK | %d |", todoTotal)
	add("| Rust unwrap/expect（全量） | %d |", unwrapTotal)
	add("| Rust unwrap/expect（生产代码） | %d |", prodUnwrapTotal)
	add("| 活跃失败记录条目 | %d |", activeFailures)
	add("| C Shadow Verification | %s |", shadowRates["C"])
	add("| C++ Shadow Verification | %s |", shadowRates["C++"])
	add("")

	add("## Rust 源文件行数 Top 20")
	add("")
	add("| 排名 | 文件 | 非空行数 |")
	add("|------|------|----------|")
	for i, fc := range rustTop {
		add("| %d | `%s` | %d |", i+1, fc.rel, fc.count)
	}
	add("")

	add("## Rust TODO/FIXME/HACK 分布（Top 10）")
	add("")
	add("| 文件 | 数量 |")
	add("|------|------|")
	for _, fc := range sortByCountDesc(todoPerFile, 10) {
		add("| `%s` | %d |", fc.rel, fc.count)
	}
	add("")

	add("## Rust unwrap/expect 分布（Top 10）")
	add("")
	add("| 文件 | 数量 |")
	add("|------|------|")
	for _, fc := range sortByCountDesc(unwrapPerFile, 10) {
		add("| `%s` | %d |", fc.rel, fc.count)
	}
	add("")

	add("## Rust 生产代码 unwrap/expect 分布（Top 10）")
	add("")
	add("| 文件 | 数量 |")
	add("|------|------|")
	for _, fc := range sortByCountDesc(prodUnwrapPerFile, 10) {
		add("| `%s` | %d |", fc.rel, fc.count)
	}
	add("")

	add("## 失败记录文件活跃条目")
	add("")
	add("| 文件 | 活跃条目 |")
	add("|------|----------|")
	for _, fc := range failuresPerFile {
		add("| `%s` | %d |", fc.rel, fc.count)
	}
	add("")

	add("## 趋势说明")
	add("")
	add("- 本报告仅反映当前快照，建议与历史报告对比观察趋势。")
	add("- 若 Rust/Dart 超大文件持续膨胀，应触发新一轮拆分评估。")
	add("- 若 `unwrap/expect` 数量持续上升，应评估错误处理健壮性。")
	add("")

	return strings.Join(lines, "\n")
}

func main() {
	if err := os.MkdirAll(reportsDir, 0o755); err != nil {
		capi.Fatal("创建 reports/ 失败: %v", err)
	}
	report := generateReport()
	outPath := filepath.Join(reportsDir, "engineering_health.md")
	if err := os.WriteFile(outPath, []byte(report), 0o644); err != nil {
		capi.Fatal("写报告失败: %v", err)
	}
	fmt.Printf("工程健康度看板已生成: %s\n", outPath)
}
