package main

// 测试事实采集层 —— 把各防线的当前真值采进 reports/facts.json。
//
// 设计原则（与 Python 原型一致，此处为长期维护版）：
//  1. 不猜、不兜底：值只能来自产物文件或显式 --run。取不到记 unavailable 并附 how_to_get，
//     绝不用 0 或旧值兜底——兜底本身就是漂移来源。
//  2. 默认只读：默认只读已有产物，不触发任何防线执行。
//  3. 带溯源：每项记录 source / as_of / provenance。

import (
	"encoding/json"
	"fmt"
	"os"
	"os/exec"
	"path/filepath"
	"regexp"
	"sort"
	"strconv"
	"strings"
	"time"
)

// ─── 数据结构 ────────────────────────────────────────────────────────────────

type Fact struct {
	Value *int `json:"value"`
	// SValue 字符串常量真值（如 ABI 版本 "2.1.0"）。Value/SValue 恰一者非零值；
	// 对账层按 Key 区分数字规则与常量规则。
	SValue     string `json:"svalue,omitempty"`
	Unit       string `json:"unit"`
	Source     string `json:"source"`
	Provenance string `json:"provenance"`
	AsOf       string `json:"as_of"`
	Status     string `json:"status"`
	Note       string `json:"note"`
	HowToGet   string `json:"how_to_get"`
}

type GitInfo struct {
	Rev    string `json:"rev"`
	Branch string `json:"branch"`
	Dirty  bool   `json:"dirty"`
}

type FactsDoc struct {
	Schema      string          `json:"schema"`
	GeneratedAt string          `json:"generated_at"`
	Git         GitInfo         `json:"git"`
	Facts       map[string]Fact `json:"facts"`
}

const factsSchema = "vitro.facts.v1"

// ─── 基础设施 ────────────────────────────────────────────────────────────────

func nowISO() string { return time.Now().Format("2006-01-02T15:04:05-07:00") }

func gitInfo(root string) GitInfo {
	run := func(args ...string) string {
		out, err := exec.Command("git", args...).Output()
		if err != nil {
			return ""
		}
		return strings.TrimSpace(string(out))
	}
	rev := run("-C", root, "rev-parse", "--short", "HEAD")
	if rev == "" {
		rev = "unknown"
	}
	branch := run("-C", root, "rev-parse", "--abbrev-ref", "HEAD")
	if branch == "" {
		branch = "unknown"
	}
	return GitInfo{Rev: rev, Branch: branch, Dirty: run("-C", root, "status", "--short") != ""}
}

func iptr(v int) *int { return &v }

func okFact(v int, unit, source, provenance, asOf string) Fact {
	return Fact{Value: iptr(v), Unit: unit, Source: source, Provenance: provenance,
		AsOf: asOf, Status: "ok"}
}

func unavail(unit, source, howToGet, note string) Fact {
	return Fact{Unit: unit, Source: source, Provenance: "none", Status: "unavailable",
		HowToGet: howToGet, Note: note}
}

func mtimeISO(p string) string {
	fi, err := os.Stat(p)
	if err != nil {
		return ""
	}
	return fi.ModTime().Format("2006-01-02T15:04:05-07:00")
}

func readJSON(p string, v any) error {
	b, err := os.ReadFile(p)
	if err != nil {
		return err
	}
	return json.Unmarshal(b, v)
}

func relOf(root, p string) string {
	r, err := filepath.Rel(root, p)
	if err != nil {
		return p
	}
	return filepath.ToSlash(r)
}

// ─── 采集器：影子防线（读产物，零副作用）────────────────────────────────────

func collectShadowC(root string, facts map[string]Fact) {
	// 两个产物名并存：本地默认写 shadow_data_latest.json；CI 用 --json
	// 指定 shadow_data.json（M15 接线后 CI 每轮采集，两者都认，缺一不兜底）。
	rel := "native/tests/shadow_verification/reports/shadow_data_latest.json"
	how := "cd native && cargo build --release && go run ./scripts/shadow_verify"

	var d struct {
		Timestamp string         `json:"timestamp"`
		Summary   map[string]int `json:"summary"`
	}
	var asOf string
	found := false
	for _, name := range []string{"shadow_data_latest.json", "shadow_data.json"} {
		p := filepath.Join(root, "native/tests/shadow_verification/reports", name)
		if err := readJSON(p, &d); err == nil && d.Summary != nil {
			rel = "native/tests/shadow_verification/reports/" + name
			asOf = d.Timestamp
			if asOf == "" {
				asOf = mtimeISO(p)
			}
			found = true
			break
		}
	}
	if !found {
		facts["shadow_c_cases"] = unavail("用例", rel, how, "产物缺失或格式不符")
		facts["shadow_c_match"] = unavail("用例", rel, how, "产物缺失或格式不符")
		return
	}
	total, hasTotal := d.Summary["total"]
	match, hasMatch := d.Summary["match"]
	if !hasTotal {
		facts["shadow_c_cases"] = unavail("用例", rel, how, "summary 缺 total 字段")
		return
	}
	facts["shadow_c_cases"] = okFact(total, "用例", rel, "read_report", asOf)
	if hasMatch {
		facts["shadow_c_match"] = okFact(match, "用例", rel, "read_report", asOf)
	}
}

func collectShadowCpp(root string, facts map[string]Fact) {
	rel := "native/tests/shadow_verification/reports/cpp_shadow_report.json"
	p := filepath.Join(root, "native/tests/shadow_verification/reports/cpp_shadow_report.json")
	how := "cd native && cargo build --release && go run ./scripts/shadow_verify_cpp"

	var arr []struct {
		DiffType string `json:"diff_type"`
	}
	if err := readJSON(p, &arr); err != nil || len(arr) == 0 {
		facts["shadow_cpp_cases"] = unavail("用例", rel, how, "产物缺失或不是数组")
		facts["shadow_cpp_match"] = unavail("用例", rel, how, "产物缺失或不是数组")
		return
	}
	matched := 0
	for _, c := range arr {
		if c.DiffType == "match" {
			matched++
		}
	}
	asOf := mtimeISO(p)
	facts["shadow_cpp_cases"] = okFact(len(arr), "用例", rel, "read_report", asOf)
	facts["shadow_cpp_match"] = okFact(matched, "用例", rel, "read_report", asOf)
}

// ─── 采集器：失败台账活跃条目（解析 md，零副作用）──────────────────────────

var (
	reActiveSection = regexp.MustCompile(`(?i)^#{2}\s+.*?(KNOWN_FAILURE|KNOWN_DIVERGENCE|KNOWN_LIMITATION)`)
	reEntry         = regexp.MustCompile(`^#{3}\s+`)
	reSection       = regexp.MustCompile(`^#{2}\s+`)
	reResolved      = regexp.MustCompile(`(?i)已修复|FIXED|RESOLVED|不再失败|no longer fails`)
	reTableDiv      = regexp.MustCompile(`^\|[-:\|\s]+\|$`)
)

func countActiveEntries(p string) (int, bool) {
	b, err := os.ReadFile(p)
	if err != nil {
		return 0, false
	}
	count, inActive, inTable := 0, false, false
	for _, line := range strings.Split(string(b), "\n") {
		s := strings.TrimSpace(strings.TrimSuffix(line, "\r"))
		if reSection.MatchString(s) {
			inActive, inTable = reActiveSection.MatchString(s), false
			continue
		}
		if !inActive {
			continue
		}
		if reEntry.MatchString(s) {
			if !reResolved.MatchString(s) {
				count++
			}
			inTable = false
			continue
		}
		if reTableDiv.MatchString(s) {
			inTable = true
			continue
		}
		if inTable && strings.HasPrefix(s, "|") && strings.HasSuffix(s, "|") {
			if !reResolved.MatchString(s) {
				count++
			}
			continue
		}
		if s != "" && !strings.HasPrefix(s, "|") {
			inTable = false
		}
	}
	return count, true
}

func collectFailureLedgers(root string, facts map[string]Fact) {
	for _, m := range []struct{ key, file string }{
		{"e2e_failures_active", "E2E_FAILURES.md"},
		{"cpp_failures_active", "CPP_FAILURES.md"},
	} {
		rel := "native/tests/" + m.file
		p := filepath.Join(root, "native", "tests", m.file)
		if n, ok := countActiveEntries(p); ok {
			facts[m.key] = okFact(n, "条", rel, "parse_markdown", mtimeISO(p))
		} else {
			facts[m.key] = unavail("条", rel, "确认 "+rel+" 是否存在", "")
		}
	}
}

// ─── 采集器：用例目录计数（磁盘真值，零副作用）──────────────────────────────

func collectCaseDirs(root string, facts map[string]Fact) {
	// 按用例扩展名计数：.in（stdin 伴生输入）与 .h（被 include 的辅助头）
	// 不是用例。曾按"非目录文件"计数，knr 的 29 个 .in、baseline 的 4 个 .h
	// 全被计入（knr 报 110 而实际 69 个 .c）——事实台账失真，未来接对账
	// 规则即引爆。
	for _, m := range []struct{ key, sub, ext string }{
		{"cpp_e2e_cases", "cpp", ".cpp"},
		{"c_e2e_baseline_cases", "baseline", ".c"},
		{"c_e2e_gap_cases", "gap", ".c"},
		{"c_e2e_knr_cases", "knr", ".c"},
		{"c_e2e_leetcode_cases", "leetcode", ".c"},
	} {
		rel := "native/tests/cases/" + m.sub + "/"
		d := filepath.Join(root, "native", "tests", "cases", m.sub)
		ents, err := os.ReadDir(d)
		if err != nil {
			facts[m.key] = unavail("用例", rel, "确认目录 "+rel+" 是否存在", "")
			continue
		}
		n := 0
		for _, e := range ents {
			if !e.IsDir() && strings.HasSuffix(e.Name(), m.ext) {
				n++
			}
		}
		// as_of 取**采集时刻**，不是目录 mtime：目录计数是"此刻磁盘状态的快照"，
		// 每次运行都重新数，不存在"陈旧"概念。用 mtime 会把"长期无增删"误判成
		// 真值超龄 —— 实测 cases/leetcode 自 06-26 未增删文件，遂被 demoteStale
		// 降级为待采集（cpp_e2e_cases 同机制，只因目录恰有新文件才侥幸逃过）。
		facts[m.key] = okFact(n, "用例", rel, "count_dir", nowISO())
	}
}

// ─── 采集器：常量真值（读源码，零副作用）────────────────────────────────────

var reAbiConst = regexp.MustCompile(`VITRO_ABI_VERSION\s*:\s*&str\s*=\s*"(\d+\.\d+\.\d+)"`)

// collectAbiVersion 采集 C ABI 版本真值。唯一来源是
// native/src/capi/first_batch.rs 的 VITRO_ABI_VERSION 常量——文档里的
// 版本号一律不是真相（M13 实证：代码 2.1.0 时仍有多份文档写 1.2.0/2.0.0）。
func collectAbiVersion(root string, facts map[string]Fact) {
	rel := "native/src/capi/first_batch.rs"
	p := filepath.Join(root, "native", "src", "capi", "first_batch.rs")
	how := "读 " + rel + " 的 VITRO_ABI_VERSION 常量"
	b, err := os.ReadFile(p)
	if err != nil {
		facts["abi_version"] = unavail("版本", rel, how, "文件缺失")
		return
	}
	m := reAbiConst.FindSubmatch(b)
	if m == nil {
		facts["abi_version"] = unavail("版本", rel, how, "未解析到 VITRO_ABI_VERSION 常量（形态变更？）")
		return
	}
	facts["abi_version"] = Fact{SValue: string(m[1]), Unit: "版本", Source: rel,
		Provenance: "read_const", AsOf: mtimeISO(p), Status: "ok"}
}

// ─── 采集器：需执行类（--run / --run-slow 才启用）───────────────────────────

func runCmd(root string, timeout time.Duration, args ...string) (string, int, bool) {
	cmd := exec.Command(args[0], args[1:]...)
	cmd.Dir = root
	var buf strings.Builder
	cmd.Stdout = &buf
	cmd.Stderr = &buf
	done := make(chan error, 1)
	if err := cmd.Start(); err != nil {
		return "", -1, false
	}
	go func() { done <- cmd.Wait() }()
	select {
	case err := <-done:
		code := 0
		if err != nil {
			if ee, ok := err.(*exec.ExitError); ok {
				code = ee.ExitCode()
			} else {
				code = -1
			}
		}
		return buf.String(), code, true
	case <-time.After(timeout):
		_ = cmd.Process.Kill()
		return buf.String(), -1, false
	}
}

func collectReplay(root string, facts map[string]Fact) {
	how := "go run ./scripts/replay/replay_s1_s5.go --anchor <short-hash>"
	out, code, ok := runCmd(root, 5*time.Minute, "go", "run", "./scripts/replay/replay_s1_s5.go")
	if !ok {
		facts["replay_assertions"] = unavail("条", "scripts/replay/replay_s1_s5.go", how, "go 不可用或超时")
		return
	}
	if m := regexp.MustCompile(`断言总数:\s*(\d+)`).FindStringSubmatch(out); m != nil {
		n, _ := strconv.Atoi(m[1])
		f := okFact(n, "条", "scripts/replay/replay_s1_s5.go", "run", nowISO())
		f.Note = fmt.Sprintf("exit=%d", code)
		facts["replay_assertions"] = f
	} else {
		facts["replay_assertions"] = unavail("条", "scripts/replay/replay_s1_s5.go", how,
			fmt.Sprintf("未解析到汇总行（exit=%d）", code))
	}
}

func pythonExe() string {
	if v := os.Getenv("VITRO_PYTHON"); v != "" {
		return v
	}
	for _, c := range []string{"python", "python3", "py"} {
		if _, err := exec.LookPath(c); err == nil {
			return c
		}
	}
	return ""
}

func collectServeSmoke(root string, facts map[string]Fact) {
	how := "python scripts/serve_smoke.py（需先构建 vitro_cli）"
	py := pythonExe()
	if py == "" {
		facts["serve_smoke_assertions"] = unavail("项", "scripts/serve_smoke.py", how, "找不到 python")
		return
	}
	out, code, ok := runCmd(root, 5*time.Minute, py, "scripts/serve_smoke.py")
	if !ok {
		facts["serve_smoke_assertions"] = unavail("项", "scripts/serve_smoke.py", how, "超时或无法执行")
		return
	}
	if m := regexp.MustCompile(`断言数:\s*(\d+)`).FindStringSubmatch(out); m != nil {
		n, _ := strconv.Atoi(m[1])
		f := okFact(n, "项", "scripts/serve_smoke.py", "run", nowISO())
		f.Note = fmt.Sprintf("exit=%d", code)
		facts["serve_smoke_assertions"] = f
	} else {
		facts["serve_smoke_assertions"] = unavail("项", "scripts/serve_smoke.py", how,
			"脚本未自报断言总数")
	}
}

func collectCargoTest(root string, facts map[string]Fact) {
	how := "cargo test --workspace --all-features"
	out, code, ok := runCmd(root, 30*time.Minute, "cargo", "test", "--workspace", "--all-features")
	if !ok {
		facts["cargo_test_passed"] = unavail("用例", "cargo test", how, "超时或 cargo 不可用")
		return
	}
	collectCargoTestFromOutput(out, code, "cargo test", "run", facts)
}

// collectCargoTestFromLog 从 CI 已落盘的 cargo test 输出解析真值（M15 接线：
// CI 里 cargo test 步骤 tee 日志后，facts 无需重跑 30 分钟的测试即可取得
// 新鲜真值）。日志缺失时记 unavailable，不兜底。
func collectCargoTestFromLog(root, logPath string, facts map[string]Fact) {
	how := "cargo test --workspace --all-features 2>&1 | tee " + logPath + "（CI 已执行）"
	b, err := os.ReadFile(filepath.Join(root, filepath.FromSlash(logPath)))
	if err != nil {
		facts["cargo_test_passed"] = unavail("用例", logPath, how, "日志缺失")
		facts["cargo_test_suites"] = unavail("个", logPath, how, "日志缺失")
		return
	}
	collectCargoTestFromOutput(string(b), 0, logPath, "parse_ci_log", facts)
}

func collectCargoTestFromOutput(out string, code int, source, provenance string, facts map[string]Fact) {
	passed := 0
	for _, m := range regexp.MustCompile(`test result: ok\.\s+(\d+) passed`).FindAllStringSubmatch(out, -1) {
		n, _ := strconv.Atoi(m[1])
		passed += n
	}
	if passed > 0 {
		f := okFact(passed, "用例", source, provenance, nowISO())
		f.Note = fmt.Sprintf("exit=%d", code)
		facts["cargo_test_passed"] = f
	} else {
		facts["cargo_test_passed"] = unavail("用例", source, "cargo test --workspace --all-features",
			"未解析到 test result 行")
	}
	if n := len(regexp.MustCompile(`Running .*target[\\/]debug[\\/]deps[\\/]`).FindAllString(out, -1)); n > 0 {
		facts["cargo_test_suites"] = okFact(n, "个", source, provenance, nowISO())
	}
}

// ─── 主入口 ─────────────────────────────────────────────────────────────────

// runKeys 是需要实际执行才能取得真值的事实；未执行时沿用上次采集值（标 cached），
// 这样 `--run` 补全一次之后，日常运行不必每次重跑防线。
var runKeys = []string{
	"replay_assertions", "serve_smoke_assertions",
	"cargo_test_passed", "cargo_test_suites",
}

func collectAll(root string, run, runSlow bool, cargoLog string, prev *FactsDoc) FactsDoc {
	facts := map[string]Fact{}
	collectShadowC(root, facts)
	collectShadowCpp(root, facts)
	collectFailureLedgers(root, facts)
	collectCaseDirs(root, facts)
	collectAbiVersion(root, facts)

	if run {
		collectReplay(root, facts)
		collectServeSmoke(root, facts)
	} else {
		facts["replay_assertions"] = unavail("条", "scripts/replay/replay_s1_s5.go", "--run",
			"需 --run 才执行")
		facts["serve_smoke_assertions"] = unavail("项", "scripts/serve_smoke.py", "--run",
			"需 --run 才执行")
	}
	switch {
	case cargoLog != "":
		// CI 接线路径：日志由 cargo test 步骤 tee 落盘，此处只解析不重跑。
		collectCargoTestFromLog(root, cargoLog, facts)
	case runSlow:
		collectCargoTest(root, facts)
	default:
		facts["cargo_test_passed"] = unavail("用例", "cargo test", "--run-slow 或 --cargo-log",
			"需 --run-slow 才执行（很慢）；CI 传 --cargo-log 解析已落盘日志")
	}

	// 沿用上次采集（仅当本轮没拿到真值）
	if prev != nil {
		for _, k := range runKeys {
			f, ok := facts[k]
			if !ok || f.Status != "unavailable" {
				continue
			}
			pv, exists := prev.Facts[k]
			if !exists || pv.Value == nil {
				continue
			}
			pv.Status = "cached"
			pv.Note = fmt.Sprintf("沿用上次采集（as_of=%s）；重跑请加 --run", pv.AsOf)
			facts[k] = pv
		}
	}

	return FactsDoc{Schema: factsSchema, GeneratedAt: nowISO(), Git: gitInfo(root), Facts: facts}
}

// cachedKeys 返回本轮沿用上次采集的事实键。
func cachedKeys(doc FactsDoc) []string {
	var out []string
	for k, v := range doc.Facts {
		if v.Status == "cached" {
			out = append(out, k)
		}
	}
	sort.Strings(out)
	return out
}

// demoteStale 真值新鲜度门禁：as_of 距今超过预算的真值降级为待采集
// （Value 置 nil、Status=stale）——不参与漂移判定与 sync，防止用陈旧
// 真值判新文档、把错数字写进文档。返回降级条数。
// as_of 两种格式都兼容：RFC3339（mtimeISO / run 采集）与 shadow 产物
// 的 "2006-01-02 15:04:05"（本地时间，无时区）。解析失败的保守不降级
// （缺年龄信息时不动真值，与"不猜"原则一致）。
func demoteStale(doc FactsDoc, maxAge time.Duration) int {
	if maxAge <= 0 {
		return 0
	}
	now := time.Now()
	n := 0
	for k, v := range doc.Facts {
		if v.Value == nil || v.AsOf == "" {
			continue
		}
		t, err := time.Parse(time.RFC3339, v.AsOf)
		if err != nil {
			t, err = time.ParseInLocation("2006-01-02 15:04:05", v.AsOf, time.Local)
		}
		if err != nil {
			continue
		}
		if now.Sub(t) > maxAge {
			n++
			v.Status = "stale"
			v.Note = fmt.Sprintf("真值超龄（as_of=%s，预算 %s）——已降级待采集，勿用于判定", v.AsOf, maxAge)
			v.Value = nil
			v.HowToGet = "加 --run（或 --run-slow）刷新采集"
			doc.Facts[k] = v
		}
	}
	return n
}

func writeFacts(doc FactsDoc, out string) error {
	if err := os.MkdirAll(filepath.Dir(out), 0o755); err != nil {
		return err
	}
	b, err := json.MarshalIndent(doc, "", "  ")
	if err != nil {
		return err
	}
	return os.WriteFile(out, append(b, '\n'), 0o644)
}

func printFacts(doc FactsDoc) {
	g := doc.Git
	dirty := ""
	if g.Dirty {
		dirty = " (dirty)"
	}
	fmt.Printf("\n事实台账 — %s  git=%s%s\n", doc.GeneratedAt, g.Rev, dirty)
	fmt.Println(strings.Repeat("-", 78))
	keys := make([]string, 0, len(doc.Facts))
	for k := range doc.Facts {
		keys = append(keys, k)
	}
	sort.Strings(keys)
	fmt.Printf("%-26s%10s  %-12s%s\n", "事实键", "值", "状态", "来源")
	fmt.Println(strings.Repeat("-", 78))
	unavailN := 0
	for _, k := range keys {
		v := doc.Facts[k]
		val := "—"
		if v.Value != nil {
			val = strconv.Itoa(*v.Value)
		}
		if v.Status == "unavailable" {
			unavailN++
		}
		fmt.Printf("%-26s%10s  %-12s%s\n", k, val, v.Status, v.Source)
	}
	fmt.Println(strings.Repeat("-", 78))
	fmt.Printf("共 %d 项，其中 %d 项不可得（不兜底，见 how_to_get）\n\n", len(doc.Facts), unavailN)
}
