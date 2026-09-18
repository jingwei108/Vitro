package main

// 就近绑定（2026-09-18 P1 批）的证红锚测试（J9：判定面变更上线前先证红）。
//
// 背景：auditDocs 此前行内所有落 Lo/Hi 区间的数字都归属每条命中规则，
// "replay 61 / serve_smoke 57"（两个真值键共行、数字互落对方区间：
// serve 的 [10,150] 含 61，replay 的 [20,200] 含 57）必然互斥判红——
// MoonBit迁移总计划.md:81 的既有误报（数字全对却判漂移）。改为就近绑定
// （数字只归属其左侧最近的规则关键词）后：
//   - 跨键共行且各自正确 → 双绿（本测试 TestNearBindMultiKeySameLine）
//   - 近邻数字写错 → 仍红（TestNearBindNearbyDriftStillCaught）
//   - 数字在关键词左侧 → 不归属任何规则（TestNearBindLeadingNumberIgnored）
//
// 夹具语义对应真实漂移样本：
//   - docs/current/01-定位与路线/MoonBit迁移总计划.md:81
//     "| **C 端到端** | Clang golden 733 全量 / replay 61 / serve_smoke 57 / ..."
// 若这些样本的判定结果变化（误报复活或真漏报），先修绑定再动文档。

import (
	"os"
	"path/filepath"
	"strings"
	"testing"
)

func writeAuditFixture(t *testing.T, lines ...string) AuditResult {
	t.Helper()
	root := t.TempDir()
	rel := filepath.Join(root, "docs", "current", "01-定位与路线", "夹具.md")
	if err := os.MkdirAll(filepath.Dir(rel), 0o755); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(rel, []byte(strings.Join(lines, "\n")+"\n"), 0o644); err != nil {
		t.Fatal(err)
	}
	doc := FactsDoc{Facts: map[string]Fact{
		"replay_assertions":      {Value: intPtr(61), Status: "ok"},
		"serve_smoke_assertions": {Value: intPtr(57), Status: "ok"},
		"shadow_c_cases":         {Value: intPtr(676), Status: "ok"},
	}}
	return auditDocs(root, doc)
}

func intPtr(v int) *int { return &v }

func findAudit(res AuditResult, key string) *FactAudit {
	for i := range res.Audits {
		if res.Audits[i].Rule.Key == key {
			return &res.Audits[i]
		}
	}
	return nil
}

// 双真值键共行且各自正确：旧语义互斥双红，就近绑定后双绿。
func TestNearBindMultiKeySameLine(t *testing.T) {
	res := writeAuditFixture(t,
		"| **C 端到端** | Clang golden 733 全量 / replay 61 / serve_smoke 57 / JIT parity 八形状（若复活） | golden 缺失必红；`.out` 只作第二来源，live clang 为主真值 |",
	)
	for _, key := range []string{"replay_assertions", "serve_smoke_assertions"} {
		a := findAudit(res, key)
		if a == nil {
			t.Fatalf("规则 %s 未出现在审计结果中", key)
		}
		if len(a.Drift) != 0 || a.Matched != 1 {
			t.Fatalf("%s：共行双键各自正确应判 Matched=1，得到 Matched=%d Drift=%d（%s）",
				key, a.Matched, len(a.Drift), driftText(a.Drift))
		}
	}
}

// 近邻写错仍必须红：61 写成 60（紧贴 replay 关键词）。
func TestNearBindNearbyDriftStillCaught(t *testing.T) {
	res := writeAuditFixture(t,
		"| **C 端到端** | Clang golden 733 全量 / replay 60 / serve_smoke 57 | 备注 |",
	)
	a := findAudit(res, "replay_assertions")
	if a == nil || len(a.Drift) != 1 {
		t.Fatalf("近邻写错（replay 60，真值 61）必须判漂移：得到 Drift=%d", lenOf(a))
	}
}

// 数字前置写法（"675 个 Shadow golden"，真值 676）必须仍被抓：
// 双向最近绑定下 675 的最近关键词是右侧的 Shadow。
func TestNearBindTrailingKeywordStillCaught(t *testing.T) {
	res := writeAuditFixture(t,
		"- 字节码格式与 675 个 Shadow golden（C# 侧只允许按既有规则追加 opcode）",
	)
	a := findAudit(res, "shadow_c_cases")
	if a == nil || len(a.Drift) != 1 {
		t.Fatalf("数字前置写法的漂移（675 个 Shadow，真值 676）必须判漂移：Drift=%d", lenOf(a))
	}
	// 改对后必须绿（同形态复检，防止只红不绿）。
	res2 := writeAuditFixture(t,
		"- 字节码格式与 676 个 Shadow golden（C# 侧只允许按既有规则追加 opcode）",
	)
	a2 := findAudit(res2, "shadow_c_cases")
	if a2 == nil || a2.Matched != 1 || len(a2.Drift) != 0 {
		t.Fatalf("数字前置写法写对（676 个 Shadow）应判 Matched=1：Matched=%d Drift=%d",
			a2.Matched, len(a2.Drift))
	}
}

// 键位平局取右："cargo test 70 套件" 的 70 左右等距，
// 后置的"套件"是计量单位——归套件数规则，不归前置的用例数规则。
func TestNearBindTiePrefersTrailingUnit(t *testing.T) {
	root := t.TempDir()
	rel := filepath.Join(root, "docs", "current", "01-定位与路线", "夹具.md")
	if err := os.MkdirAll(filepath.Dir(rel), 0o755); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(rel, []byte("验收：cargo test 70 套件全绿\n"), 0o644); err != nil {
		t.Fatal(err)
	}
	doc := FactsDoc{Facts: map[string]Fact{
		"cargo_test_suites": {Value: intPtr(70), Status: "ok"},
	}}
	res := auditDocs(root, doc)
	s := findAudit(res, "cargo_test_suites")
	if s == nil || s.Matched != 1 {
		t.Fatalf("\"cargo test 70 套件\" 的 70 应归属套件数规则（Matched=1）：Matched=%d Drift=%d",
			s.Matched, len(s.Drift))
	}
}

// 就近归属跨规则并集：serve_smoke 关键词出现在 replay 之后时，
// 其后的数字只归 serve，不再落进 replay 的候选。
func TestNearBindNearestKeywordWins(t *testing.T) {
	res := writeAuditFixture(t,
		"| 项 | replay 断言数见 serve_smoke 57 项 | 备注 |",
	)
	a := findAudit(res, "serve_smoke_assertions")
	if a == nil || a.Matched != 1 {
		t.Fatalf("57 紧贴 serve_smoke 关键词应判 Matched=1：得到 %v", lenOf(a))
	}
	r := findAudit(res, "replay_assertions")
	if r != nil && (len(r.Drift) != 0 || r.Matched != 0) {
		t.Fatalf("57 的最近关键词是 serve_smoke，不应再归属 replay：Matched=%d Drift=%d",
			r.Matched, len(r.Drift))
	}
}

func lenOf(a *FactAudit) int {
	if a == nil {
		return -1
	}
	return len(a.Drift)
}

func driftText(hits []Hit) string {
	var sb strings.Builder
	for _, h := range hits {
		sb.WriteString(h.File)
		sb.WriteString(":")
		sb.WriteString(strings.TrimSpace(h.Text))
		sb.WriteString("；")
	}
	return sb.String()
}
