package main

// 文档数字对账层 —— 用事实台账比对文档里的测试数字，并**精确替换**漂移项。
//
// 核心判据（本工具的世界观）：文档里的"测试数字"有三种，混着人肉同步必然漂移。
//
//	CURRENT   裸数字（无日期、无 as-of 词）  → 必须等于真值，否则判漂移
//	AS-OF     带日期 / "截至" / "原记" 等     → 冻结，不参与对账
//	DERIVED   普查派生（中位行数、步数…）     → 需重跑专项脚本，本工具不覆盖
//
// 判定为双层：文件级（文件名含 裁定/决议/评估报告/工作记录…）+ 行级（含日期或 as-of 词）。

import (
	"fmt"
	"os"
	"path/filepath"
	"regexp"
	"sort"
	"strconv"
	"strings"
)

// ─── 对账规则 ────────────────────────────────────────────────────────────────

type Rule struct {
	Key     string
	Label   string
	Context *regexp.Regexp
	Exclude *regexp.Regexp
	Lo, Hi  int
	Unit    string
}

func rules() []Rule {
	mk := func(key, label, ctx, exc string, lo, hi int, unit string) Rule {
		r := Rule{Key: key, Label: label, Context: regexp.MustCompile("(?i)" + ctx),
			Lo: lo, Hi: hi, Unit: unit}
		if exc != "" {
			r.Exclude = regexp.MustCompile("(?i)" + exc)
		}
		return r
	}
	return []Rule{
		mk("shadow_c_cases", "C 影子用例总数", `影子|shadow`,
			`C\+\+|shadow_verify_cpp|cpp_shadow`, 400, 900, "用例"),
		mk("shadow_cpp_cases", "C++ 影子用例总数",
			`C\+\+.*(影子|shadow)|shadow_verify_cpp|cpp_shadow|C\+\+ 侧`,
			``, 30, 250, "用例"),
		mk("cpp_e2e_cases", "C++ E2E 用例数", `C\+\+\s*E2E|cases/cpp`,
			`影子|shadow`, 30, 250, "用例"),
		mk("replay_assertions", "回放断言数", `回放|replay|S1[-–]S5|S1/S5`,
			``, 20, 200, "条"),
		mk("serve_smoke_assertions", "serve 冒烟断言数",
			`serve_smoke|serve 冒烟|协议断言|项断言`, ``, 10, 150, "项"),
		mk("cargo_test_passed", "cargo test 用例数", `cargo test|passed|全绿|rust 单测`,
			``, 500, 2500, "用例"),
		mk("cargo_test_suites", "cargo test 套件数", `套件`, `断言|项断言`, 20, 200, "个"),
		// T6/T5 审阅（2026-09-19）：moonbit README 测试数防漂移——发布面
		// 审阅实证 0.1.0 的 README 计数过期（53 vs 实跑 58），纳入机判。
		mk("moonbit_test_passed", "MoonBit 测试数", `moon\s*test|moonbit[^\n]*测试|测试[^\n]*moonbit`, ``, 40, 500, "用例"),
	}
}

// ─── 文档分级 ────────────────────────────────────────────────────────────────

var (
	reHistFile = regexp.MustCompile(`(?i)裁定|决议|评估报告|工作记录|维护方案|追踪|审计计划|回执|登记|埋雷|事故|整备|CHANGELOG|ARCHIVE`)
	reDate     = regexp.MustCompile(`\d{4}-\d{2}-\d{2}|\d{4}/\d{1,2}/\d{1,2}|\d{8}`)
	reAsofWord = regexp.MustCompile(`(?i)as[- ]?of|截至|原记|彼时|当时|历史|已废弃|旧口径|陈旧口径|归档|已过时|已被.*取代|口径已`)
	rePhase    = regexp.MustCompile(`(?i)(Phase|Stage|阶段|第)\s*\d+`)
	reNum      = regexp.MustCompile(`\d{2,6}`)
	// 数字绑定实测结果：直接替换会**伪造测量**（如"632 用例实测 103.6s"改成 671
	// 就变成假记录）。此类应标 as-of，而不是改数字。
	reMeasure = regexp.MustCompile(`实测|耗时|冷启动|加速比|吞吐|性能|基准`)
	// 数字带分解式：只换总数会让算式不成立（如 "636 个用例（617 + vitro_better 16 + 3）"）。
	// 两个数字之间有加号即算分解式（中间允许夹少量说明词）；"C++" 因加号旁无数字不会误触发。
	reBreakdown = regexp.MustCompile(`\d[^+＋\n]{0,24}[+＋][^+＋\n]{0,24}\d`)
)

func splitLinesKeep(p string) ([]string, error) {
	b, err := os.ReadFile(p)
	if err != nil {
		return nil, err
	}
	return strings.Split(string(b), "\n"), nil
}

// scanFiles 扫描目标文档：docs/ 全部 + 根目录 md（归档目录跳过）。
func scanFiles(root string) []string {
	var out []string
	docsDir := filepath.Join(root, "docs")
	_ = filepath.WalkDir(docsDir, func(p string, d os.DirEntry, err error) error {
		if err != nil {
			return nil
		}
		rel := relOf(root, p)
		if d.IsDir() {
			if strings.Contains(rel, "archive") {
				return filepath.SkipDir
			}
			return nil
		}
		if strings.HasSuffix(p, ".md") {
			out = append(out, rel)
		}
		return nil
	})
	for _, n := range []string{"AGENTS.md", "README.md", "moonbit/README.md", "moonbit/README.mbt.md"} {
		if _, err := os.Stat(filepath.Join(root, n)); err == nil {
			out = append(out, n)
		}
	}
	sort.Strings(out)
	return out
}

func classifyFile(rel string) string {
	if strings.Contains(rel, "/archive/") || strings.HasPrefix(filepath.Base(rel), "ARCHIVE_") {
		return "ARCHIVE"
	}
	// docs/spec/ 是对外承诺的协议文档（wire format / 验收快照）：其中的
	// 数字是 as-of 时点记录（且可能引用已退役的旧驱动路径），改数字等于
	// 往旧快照里塞新事实——按行级日期判冻结之外，文件级整体按 AS-OF 处理。
	if strings.HasPrefix(rel, "docs/spec/") {
		return "AS-OF"
	}
	if reHistFile.MatchString(filepath.Base(rel)) {
		return "AS-OF"
	}
	return "CURRENT"
}

func classifyLine(line string) bool {
	if rePhase.MatchString(line) {
		return true
	}
	return reDate.MatchString(line) || reAsofWord.MatchString(line)
}

// blankOut 用等长空格抹掉匹配片段，**保持字节偏移不变**。
func blankOut(s string, re *regexp.Regexp) string {
	return re.ReplaceAllStringFunc(s, func(m string) string {
		return strings.Repeat(" ", len(m))
	})
}

func isWordByte(b byte) bool {
	return b == '_' || b == '.' ||
		(b >= '0' && b <= '9') || (b >= 'a' && b <= 'z') || (b >= 'A' && b <= 'Z')
}

type numSpan struct {
	start, end int // 字节偏移（相对整行）
	value      int
}

// numberSpans 提取候选数字：先抹掉日期与阶段序号，再按 16 进制安全的边界检查过滤。
func numberSpans(line string) []numSpan {
	masked := blankOut(blankOut(line, rePhase), reDate)
	var out []numSpan
	for _, loc := range reNum.FindAllStringIndex(masked, -1) {
		s, e := loc[0], loc[1]
		if s > 0 && isWordByte(masked[s-1]) {
			continue
		}
		if e < len(masked) && isWordByte(masked[e]) {
			continue
		}
		v, err := strconv.Atoi(masked[s:e])
		if err != nil {
			continue
		}
		out = append(out, numSpan{s, e, v})
	}
	return out
}

// ─── 对账 ────────────────────────────────────────────────────────────────────

type Hit struct {
	Key     string
	File    string
	LineNo  int
	Spans   []numSpan
	Text    string
	Cooccur []string
	Warn    string
}

func (h Hit) Value() int { return h.Spans[0].value }

type FactAudit struct {
	Rule    Rule
	Truth   *int
	Matched int
	Drift   []Hit
	Frozen  []Hit
	Pending []Hit
	// Manual：数字分解式 / 绑定实测结果的行。子项落在 Lo/Hi 区间内时
	// 与总数无法机判区分（`671 个用例（664 + 3 + 4）` 的 664 会被误判
	// 漂移），自动替换又会改断算式/伪造测量——归入人工维护，不判 drift
	// 不自动 sync，报告常显提醒。
	Manual []Hit
}

// ─── 常量对账（M13 细化，2026-09-18）─────────────────────────────────────────
//
// 背景（§7A 复核 M13 实证）：ABI 版本代码升至 2.1.0 后，多份 CURRENT 文档
// 仍写 1.2.0 / 2.0.0——数字对账规则只抓区间整数，抓不住 "x.y.z" 版本串。
// 判据（与 §7A 登记一致）：**陈述"当前现值"的句子对账，记录"版本号历史
// 事件"的句子豁免**——两者可同篇共存（如 项目更名记录.md）。
//
// 机械化为三条：
//   1. 行提及 vitro_abi_version 或 "ABI 版本" 且含 x.y.z 版本串 → 参与判定；
//   2. 行级豁免（历史事件）：既有冻结判据（日期/as-of 词）+ 迁移箭头（→/->
//      即 "1.3.0 → 2.0.0" 事件句）+ 历史标记词（首批/曾/当时/追加/条目…）；
//   3. 非豁免行的**全部**版本串必须等于真值——旧版本号要么待在箭头迁移句/
//      历史标记句里，要么是当前现值，不允许裸旧版本号以现在时出现。
//
// 漂移处置一律人工（不进 sync 自动替换）：现值句自动替换成新值会把
// "更名时返回 2.0.0" 这类事件句篡改成假历史；改写句子形态（补时点标记）
// 是语义决策，机器没有依据替人做。

type ConstRule struct {
	Key     string
	Label   string
	Context *regexp.Regexp
	Exempt  *regexp.Regexp
	Unit    string
}

func constRules() []ConstRule {
	return []ConstRule{{
		Key:     "abi_version",
		Label:   "ABI 版本（vitro_abi_version）",
		Context: regexp.MustCompile(`vitro_abi_version|ABI`),
		Exempt: regexp.MustCompile(`→|->|首批|历史|曾|当时|彼时|追加|条目|已升|` +
			`旧版|更名时|彼时|迁移自|升级自`),
		Unit: "版本",
	}}
}

var reVersion = regexp.MustCompile(`\d+\.\d+\.\d+`)

type ConstHit struct {
	Key    string
	File   string
	LineNo int
	Found  []string
	Text   string
}

type ConstAudit struct {
	Rule     ConstRule
	Truth    string
	HasTruth bool
	Matched  int
	Drift    []ConstHit
	Frozen   []ConstHit
}

type AuditResult struct {
	Audits   []FactAudit
	TruthOf  map[string]int
	DriftN   int
	FrozenN  int
	PendingN int
	ManualN  int
	ScanN    int
	// 坏引用：文档指向不存在的脚本文件（CURRENT 层才算；as-of/归档里的
	// 旧路径是有意的历史叙述）。数字冻住不等于路径永远有效——退役驱动
	// 留在文档里会让读者按图索骥扑空（spec 曾实测：shadow_verify.py 等
	// 三个退役路径在冻结文档里躺了数日无人发现）。
	Broken []BrokenRef
	// 常量对账（版本号等字符串真值），与数字对账平行；DriftN 已并入其
	// 漂移计数，check 同样一票红。
	Consts []ConstAudit
}

// auditConst 常量对账：只扫 CURRENT 层文档（AS-OF 文件的版本号是有意的
// 时点记载——裁定/登记文档引用"当时代码是什么版本"正是复核的证据链）。
func auditConst(root string, doc FactsDoc) []ConstAudit {
	rs := constRules()
	files := scanFiles(root)
	var out []ConstAudit
	for _, r := range rs {
		f, ok := doc.Facts[r.Key]
		a := ConstAudit{Rule: r}
		if ok && f.SValue != "" {
			a.Truth = f.SValue
			a.HasTruth = true
		}
		for _, rel := range files {
			if classifyFile(rel) != "CURRENT" {
				continue
			}
			lines, err := splitLinesKeep(filepath.Join(root, filepath.FromSlash(rel)))
			if err != nil {
				continue
			}
			for i, raw := range lines {
				line := strings.TrimSuffix(raw, "\r")
				if !r.Context.MatchString(line) {
					continue
				}
				vers := reVersion.FindAllString(line, -1)
				if len(vers) == 0 {
					continue
				}
				h := ConstHit{Key: r.Key, File: rel, LineNo: i + 1, Found: vers,
					Text: strings.TrimSpace(line)}
				if !a.HasTruth {
					continue // 真值不可得：待采集，不判（unavail 由台账自报）
				}
				ok := true
				for _, v := range vers {
					if v != a.Truth {
						ok = false
						break
					}
				}
				if ok {
					a.Matched++
					continue
				}
				if classifyLine(line) || r.Exempt.MatchString(line) {
					a.Frozen = append(a.Frozen, h)
				} else {
					a.Drift = append(a.Drift, h)
				}
			}
		}
		out = append(out, a)
	}
	return out
}

func auditDocs(root string, doc FactsDoc) AuditResult {
	rs := rules()
	byKey := map[string]*FactAudit{}
	order := make([]string, 0, len(rs))
	for i := range rs {
		a := &FactAudit{Rule: rs[i], Truth: doc.Facts[rs[i].Key].Value}
		byKey[rs[i].Key] = a
		order = append(order, rs[i].Key)
	}

	files := scanFiles(root)
	for _, rel := range files {
		tier := classifyFile(rel)
		if tier == "ARCHIVE" {
			continue
		}
		lines, err := splitLinesKeep(filepath.Join(root, filepath.FromSlash(rel)))
		if err != nil {
			continue
		}
		for i, raw := range lines {
			line := strings.TrimSuffix(raw, "\r")
			if strings.TrimSpace(line) == "" {
				continue
			}
			frozen := classifyLine(line) || tier == "AS-OF"
			spans := numberSpans(line)

			// 这一行对每个事实各命中哪些候选数字。
			// P1 批（2026-09-18）：数字**就近绑定**——每个候选数字只归属
			// （双向）距离最近的规则关键词，平局取左。此前行内所有落区间
			// 数字都归属每条命中规则："replay 61 / serve_smoke 57"（两个
			// 真值键共行、数字互落对方 Lo/Hi 区间）必然互斥判红。就近绑定
			// 消除跨键误报，同时保住两种既有写法的判定：
			//   - "关键词 数字"（Shadow 676 用例）——左侧最近即本键；
			//   - "数字 关键词"（675 个 Shadow golden / cargo test 70 套件）——
			//     右侧最近即本键，不会被左侧更近的其他键挤占。
			// 近邻写错的数字（如 "Shadow 600 用例"）仍按最近关键词判红
			// （见 near_bind_test.go 的 J9 证红锚）。
			type kwPos struct {
				pos int // 关键词起点（字节）
				end int // 关键词终点（字节）
				key string
			}
			var kws []kwPos
			for _, r := range rs {
				if !r.Context.MatchString(line) {
					continue
				}
				if r.Exclude != nil && r.Exclude.MatchString(line) {
					continue
				}
				for _, loc := range r.Context.FindAllStringIndex(line, -1) {
					kws = append(kws, kwPos{pos: loc[0], end: loc[1], key: r.Key})
				}
			}
			sort.Slice(kws, func(i, j int) bool { return kws[i].pos < kws[j].pos })
			// nearestKeyOf：数字 span 与哪个关键词（双向）最近。左距 = 数字
			// 起点减关键词终点；右距 = 关键词起点减数字终点。平局取右：
			// "cargo test 70 套件" 的 70 左右等距，后置的"套件"才是它的
			// 计量单位（前置的 "cargo test" 是行首状语）。
			nearestKeyOf := func(s numSpan) string {
				best, bestDist := "", -1
				for _, k := range kws {
					if k.end <= s.start {
						d := s.start - k.end
						if bestDist < 0 || d < bestDist {
							best, bestDist = k.key, d
						}
					} else {
						d := k.pos - s.end
						if bestDist < 0 || d <= bestDist {
							best, bestDist = k.key, d
						}
					}
				}
				return best
			}
			byRule := map[string][]numSpan{}
			for _, r := range rs {
				if !r.Context.MatchString(line) {
					continue
				}
				if r.Exclude != nil && r.Exclude.MatchString(line) {
					continue
				}
				var c []numSpan
				for _, s := range spans {
					if nearestKeyOf(s) != r.Key {
						continue
					}
					if s.value >= r.Lo && s.value <= r.Hi {
						c = append(c, s)
					}
				}
				if len(c) > 0 {
					byRule[r.Key] = c
				}
			}
			if len(byRule) == 0 {
				continue
			}
			for key, cands := range byRule {
				a := byKey[key]
				h := Hit{Key: key, File: rel, LineNo: i + 1, Spans: cands,
					Text: strings.TrimSpace(line)}
				if a.Truth == nil {
					a.Pending = append(a.Pending, h) // 无真值：待采集，不判漂移
					continue
				}
				hit := false
				for _, s := range cands {
					if s.value != *a.Truth {
						hit = true
					}
				}
				if !hit {
					a.Matched++
					continue
				}
				var others []string
				for k2 := range byRule {
					if k2 != key {
						others = append(others, byKey[k2].Rule.Label)
					}
				}
				sort.Strings(others)
				h.Cooccur = others
				var warns []string
				if reMeasure.MatchString(line) {
					warns = append(warns,
						"该行数字绑定了实测结果，直接替换会伪造测量——建议标 as-of 日期而非改数字")
				}
				if reBreakdown.MatchString(line) {
					warns = append(warns,
						"该行含数字分解式（a + b + c），只替换总数会让算式不成立——请一并核对子项")
				}
				h.Warn = strings.Join(warns, "；")
				if frozen {
					a.Frozen = append(a.Frozen, h)
				} else if h.Warn != "" {
					// 分解式/实测行：子项与总数无法机判区分、自动替换有破坏面
					//（见 FactAudit.Manual 注释）——人工维护，不判 drift。
					a.Manual = append(a.Manual, h)
				} else {
					a.Drift = append(a.Drift, h)
				}
			}
		}
	}

	res := AuditResult{TruthOf: map[string]int{}, ScanN: len(files)}
	for _, k := range order {
		a := byKey[k]
		res.Audits = append(res.Audits, *a)
		res.DriftN += len(a.Drift)
		res.ManualN += len(a.Manual)
		res.FrozenN += len(a.Frozen)
		res.PendingN += len(a.Pending)
		if a.Truth != nil {
			res.TruthOf[k] = *a.Truth
		}
	}
	// 按文件 + 行号排序，便于人工顺序核对
	sort.SliceStable(res.Audits, func(i, j int) bool { return order[i] < order[j] })
	res.Broken = scanBrokenRefs(root, files)
	// 常量对账（版本号）：与数字对账平行的第二轴，漂移并入 DriftN 一票红。
	res.Consts = auditConst(root, doc)
	for _, c := range res.Consts {
		res.DriftN += len(c.Drift)
		res.FrozenN += len(c.Frozen)
	}
	return res
}

// ─── 坏引用扫描（文档指向不存在的脚本）────────────────────────────────────

var reScriptRef = regexp.MustCompile(
	`(?:scripts|native/tests|native/scripts)/[A-Za-z0-9_/.-]+\.(?:py|go|ps1)`)

// reNarrativeRef 叙述性引用豁免：该行本身就在说明"此脚本已删除/退役/
// 规划中"时，路径出现是历史叙述而非失效指引（实测 16 处坏引用中 12 处
// 属此类——前端切割/D5 退役的脚本在文档里留有说明性记载）。行级豁免，
// 与数字对账的 freeze 行级判定同一粒度。
// "取代/接替"刻意不在词表（U1 复审受控实验实锤误豁免："已用
// nonexistent.py 取代旧流程"——取代标记的是新路径存活）；接替句的旧
// 路径由"同句活路径"规则豁免（见 scanBrokenRefs 注释）。
var reNarrativeRef = regexp.MustCompile(
	`不存在|已迁出|已移除|已失效|未恢复|仍缺|删除|退役|规划中`)

type BrokenRef struct {
	File   string
	LineNo int
	Path   string
	Text   string
}

// scanBrokenRefs 检查文档中引用的脚本路径是否真实存在。
//
// 判据是**行级语境**而非文件层级（U1 复审修正）：
//   - 命令语境（路径紧跟 python / go run / cargo 等命令动词）= 祈使指引，
//     **任何文件层级都扫**——AS-OF 文件里的活步骤恰恰是最需要抓的
//     （实测：工程债务维护方案的"执行步骤"第 1 步已更新 Go 版、第 2 步
//     还是退役的 shadow_verify_cpp.py——同段半更新，文件级豁免整体放过）；
//   - 非命令语境的路径提及：只扫 CURRENT 层——历史文档的叙述性旧路径
//     （spec 勘误注记、埋雷台账、迁出记载）是有意保留的。
//
// 两道行级豁免：
//   - 豁免词（不存在|已迁出|已移除|已失效|未恢复|仍缺|删除|退役|规划中）——
//     行本身在说明"此脚本已死"。"取代/接替"**不在**词表（方向性错误：
//     它们标记的是新路径存活——受控实验实锤"已用 nonexistent.py 取代
//     旧流程"被误豁免）；接替句改由下行"同句活路径"规则豁免被接替方；
//   - 同句活路径：一行内既有活路径又有死路径（"A 接替 B"形态）→ 死路径
//     是被接替方，豁免；全死才参与判定。
//
// 存在性按三级判定：①字面路径存在；②去扩展名后是含 .go 文件的目录
// （包路径引用——文档写 `scripts/shadow_verify.go` 而磁盘是
// `scripts/shadow_verify/main.go`，Go 程序按目录组织的合法形态）；
// ③其余即坏引用。
func scanBrokenRefs(root string, files []string) []BrokenRef {
	var out []BrokenRef
	for _, rel := range files {
		if strings.Contains(rel, "/archive/") || strings.HasPrefix(filepath.Base(rel), "ARCHIVE_") {
			continue // 归档目录整体跳过（含 ARCHIVE_ 前缀的明确归档件）
		}
		asof := classifyFile(rel) != "CURRENT"
		lines, err := splitLinesKeep(filepath.Join(root, filepath.FromSlash(rel)))
		if err != nil {
			continue
		}
		for i, raw := range lines {
			line := strings.TrimSuffix(raw, "\r")
			refs := reScriptRef.FindAllString(line, -1)
			if len(refs) == 0 {
				continue
			}
			var dead []string
			alive := false
			for _, m := range refs {
				if scriptRefAlive(root, m) {
					alive = true
				} else {
					dead = append(dead, m)
				}
			}
			if len(dead) == 0 {
				continue
			}
			if alive {
				continue // 接替句：同句有活路径，死路径是被接替方
			}
			isCmd := false
			for _, m := range dead {
				if inCommandContext(line, m) {
					isCmd = true
					break
				}
			}
			if asof && !isCmd {
				continue // 历史文档的非命令叙述：有意保留的旧路径
			}
			if reNarrativeRef.MatchString(line) {
				continue // 行自身说明该脚本已死（"原命令 xxx 已失效"等）
			}
			for _, m := range dead {
				out = append(out, BrokenRef{File: rel, LineNo: i + 1, Path: m,
					Text: strings.TrimSpace(line)})
			}
		}
	}
	sort.Slice(out, func(a, b int) bool {
		if out[a].File != out[b].File {
			return out[a].File < out[b].File
		}
		return out[a].LineNo < out[b].LineNo
	})
	return out
}

// 命令动词前缀：路径紧跟其后即视为祈使指引（可复现命令形态）。
var reCmdVerb = regexp.MustCompile(
	`(?i)(?:python3?|py|go\s+run|cargo|bash|sh|pwsh|powershell)\s+(?:\.\/|\.\\)?$`)

// inCommandContext 判定路径 ref 在行内是否处于命令语境
// （其前方紧邻窗口内以命令动词结尾）。
func inCommandContext(line, ref string) bool {
	idx := strings.Index(line, ref)
	if idx <= 0 {
		return false
	}
	start := idx - 24
	if start < 0 {
		start = 0
	}
	return reCmdVerb.MatchString(line[start:idx])
}

func scriptRefAlive(root, ref string) bool {
	p := filepath.Join(root, filepath.FromSlash(ref))
	if _, err := os.Stat(p); err == nil {
		return true
	}
	// 包路径引用：xxx.go 不存在但 xxx/ 是含 .go 文件的目录
	if strings.HasSuffix(ref, ".go") {
		dir := strings.TrimSuffix(p, ".go")
		if ents, err := os.ReadDir(dir); err == nil {
			for _, e := range ents {
				if !e.IsDir() && strings.HasSuffix(e.Name(), ".go") {
					return true
				}
			}
		}
	}
	return false
}

// allDrift 展开为按位置排序的待改清单。
func (r AuditResult) allDrift() []Hit {
	var out []Hit
	for _, a := range r.Audits {
		out = append(out, a.Drift...)
	}
	sort.SliceStable(out, func(i, j int) bool {
		if out[i].File != out[j].File {
			return out[i].File < out[j].File
		}
		return out[i].LineNo < out[j].LineNo
	})
	return out
}

func labelOf(r AuditResult, key string) string {
	for _, a := range r.Audits {
		if a.Rule.Key == key {
			return a.Rule.Label
		}
	}
	return key
}

func unitOf(r AuditResult, key string) string {
	for _, a := range r.Audits {
		if a.Rule.Key == key {
			return a.Rule.Unit
		}
	}
	return ""
}

// ─── 报告渲染 ────────────────────────────────────────────────────────────────

func renderReport(root string, doc FactsDoc, res AuditResult, verbose bool) string {
	var b strings.Builder
	b.WriteString("# 文档测试数字漂移报告\n\n")
	b.WriteString("> 生成时间: " + nowISO() + "\n")
	dirty := ""
	if doc.Git.Dirty {
		dirty = " (dirty)"
	}
	b.WriteString("> 基线: `" + doc.Git.Rev + dirty + "`  真值源: `reports/facts.json`\n\n")
	b.WriteString("判据：**CURRENT 文档中的裸数字（无日期、无 as-of 词）必须等于真值**；" +
		"带日期或位于历史文档中的数字视为 as-of 快照，冻结不对账。\n\n")
	b.WriteString("## 摘要\n\n")
	b.WriteString("| 事实 | 真值 | 一致命中 | 漂移（需修） | 冻结（as-of） | 待采集 |\n")
	b.WriteString("|------|------|----------|--------------|---------------|--------|\n")
	for _, a := range res.Audits {
		truth := "—"
		if a.Truth != nil {
			truth = strconv.Itoa(*a.Truth)
		} else if f, ok := doc.Facts[a.Rule.Key]; ok && f.Status == "stale" {
			// 区分"从无真值"（需 --run 采集）与"真值超龄被降级"（需先刷新产物）
			truth = "— ⏰超龄"
		}
		b.WriteString(fmt.Sprintf("| %s | %s | %d | %d | %d | %d |\n",
			a.Rule.Label, truth, a.Matched, len(a.Drift), len(a.Frozen), len(a.Pending)))
	}
	b.WriteString("\n")

	b.WriteString("## 漂移明细（CURRENT 文档，需与真值对齐）\n\n")
	if res.DriftN == 0 {
		b.WriteString("无漂移。\n\n")
	} else {
		for _, a := range res.Audits {
			if len(a.Drift) == 0 {
				continue
			}
			b.WriteString(fmt.Sprintf("### %s — 真值 **%d** %s（%d 处）\n\n",
				a.Rule.Label, *a.Truth, a.Rule.Unit, len(a.Drift)))
			for _, h := range a.Drift {
				extra := ""
				if len(h.Cooccur) > 0 {
					extra = " — ⚠ 同行还含 **" + strings.Join(h.Cooccur, " / ") + "**，改这行时别漏"
				}
				b.WriteString(fmt.Sprintf("- `%s:%d` 出现 **%d**%s\n", h.File, h.LineNo, h.Value(), extra))
				b.WriteString("  > " + h.Text + "\n")
				if h.Warn != "" {
					b.WriteString("  > ⚠ " + h.Warn + "\n")
				}
			}
			b.WriteString("\n")
		}
	}

	if len(res.Consts) > 0 {
		b.WriteString("## 常量对账（版本号等字符串真值）\n\n")
		b.WriteString("判据：陈述\"当前现值\"的句子必须等于源码真值；记录\"历史事件\"的句子" +
			"（迁移箭头 → / 首批 / 曾 / 当时 等）豁免。**漂移一律人工修**——自动替换会把" +
			"事件句篡改成假历史，改写句子形态（补时点标记）是语义决策。\n\n")
		for _, c := range res.Consts {
			truth := c.Truth
			if !c.HasTruth {
				truth = "—"
			}
			b.WriteString(fmt.Sprintf("### %s — 真值 **%s**（一致 %d / 漂移 %d / 冻结 %d）\n\n",
				c.Rule.Label, truth, c.Matched, len(c.Drift), len(c.Frozen)))
			for _, h := range c.Drift {
				b.WriteString(fmt.Sprintf("- `%s:%d` 出现 %s\n", h.File, h.LineNo,
					strings.Join(h.Found, "、")))
				b.WriteString("  > " + h.Text + "\n")
			}
			b.WriteString("\n")
		}
	}

	if verbose {
		b.WriteString("## 冻结命中（as-of 快照，不参与对账）\n\n")
		for _, a := range res.Audits {
			if len(a.Frozen) == 0 {
				continue
			}
			b.WriteString(fmt.Sprintf("### %s（%d 处）\n\n", a.Rule.Label, len(a.Frozen)))
			for _, h := range a.Frozen {
				b.WriteString(fmt.Sprintf("- `%s:%d` = %d\n", h.File, h.LineNo, h.Value()))
			}
			b.WriteString("\n")
		}
	}

	if len(res.Broken) > 0 {
		b.WriteString("## 坏引用（CURRENT 文档指向不存在的脚本）\n\n")
		b.WriteString("数字冻住不等于路径永远有效：退役驱动留在文档里，读者按图索骥会扑空。\n\n")
		for _, r := range res.Broken {
			b.WriteString(fmt.Sprintf("- `%s:%d` → `%s`\n", r.File, r.LineNo, r.Path))
			b.WriteString("  > " + r.Text + "\n")
		}
		b.WriteString("\n")
	}

	if res.ManualN > 0 {
		b.WriteString("## 人工维护（分解式 / 实测数字行）\n\n")
		b.WriteString("子项落在判定区间内时与总数无法机判区分，自动替换会改断算式或伪造测量——\n")
		b.WriteString("这些行不参与漂移判定与自动 sync，真值变化时请人工核对整行。\n\n")
		for _, a := range res.Audits {
			if len(a.Manual) == 0 {
				continue
			}
			b.WriteString(fmt.Sprintf("### %s — 真值 **%d** %s（%d 处）\n\n",
				a.Rule.Label, *a.Truth, a.Rule.Unit, len(a.Manual)))
			for _, h := range a.Manual {
				b.WriteString(fmt.Sprintf("- `%s:%d`（首数 %d）\n", h.File, h.LineNo, h.Value()))
				b.WriteString("  > " + h.Text + "\n")
			}
			b.WriteString("\n")
		}
	}
	return b.String()
}

// ─── 精确替换 ────────────────────────────────────────────────────────────────

// ruleByKey 取回事实键对应的规则（applyHit 需要 Lo/Hi/Context/Exclude）。
func ruleByKey(key string) (Rule, bool) {
	for _, r := range rules() {
		if r.Key == key {
			return r, true
		}
	}
	return Rule{}, false
}

// applyHit 只替换该行内**落在候选区间且不等于真值**的数字，其余字节原样保留。
//
// 关键：候选区间在**替换前基于当前文件内容重新计算**，不复用 `Hit.Spans`。
// 后者是 audit 时刻算出的字节偏移；同一行被多条规则命中时，先替换的那条若
// 改变了长度（如 100→97 少 1 字节），后续条目的旧偏移会整体失真 —— 实测会把
// "78 个用例" 切成 "781个用例"（见 offsets_test.go）。重新定位即可免疫。
func applyHit(root string, h Hit, truth int) error {
	p := filepath.Join(root, filepath.FromSlash(h.File))
	lines, err := splitLinesKeep(p)
	if err != nil {
		return err
	}
	idx := h.LineNo - 1
	if idx < 0 || idx >= len(lines) {
		return fmt.Errorf("%s:%d 越界", h.File, h.LineNo)
	}
	line := lines[idx]

	r, ok := ruleByKey(h.Key)
	if !ok {
		return fmt.Errorf("未知事实键 %q", h.Key)
	}
	// 安全网：前序条目可能已改写该行，若它不再匹配本规则就不许动手。
	if !r.Context.MatchString(line) ||
		(r.Exclude != nil && r.Exclude.MatchString(line)) {
		return fmt.Errorf("%s:%d 已不再匹配规则 %s（可能已被其他条目改动），请重跑", h.File, h.LineNo, h.Key)
	}

	var spans []numSpan
	for _, s := range numberSpans(line) {
		if s.value >= r.Lo && s.value <= r.Hi {
			spans = append(spans, s)
		}
	}
	// 倒序替换，避免前面的替换影响后面的偏移
	for i := len(spans) - 1; i >= 0; i-- {
		s := spans[i]
		if s.value == truth {
			continue
		}
		if s.start < 0 || s.end > len(line) || s.start >= s.end {
			continue
		}
		line = line[:s.start] + strconv.Itoa(truth) + line[s.end:]
	}
	lines[idx] = line
	return os.WriteFile(p, []byte(strings.Join(lines, "\n")), 0o644)
}
