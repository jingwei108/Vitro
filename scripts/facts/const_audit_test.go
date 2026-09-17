package main

// 常量对账（abi_version）的证红锚测试（J9：护栏上线前先证明它会红）。
//
// 夹具全部取自真实漂移样本（2026-09-15 复核 §7A / M13 实证）：
//   - 项目路线图.md:15  "vitro_abi_version() = 1.2.0"        —— 现值句漂移
//   - 项目更名记录.md:21 "ABI 版本 1.3.0 → 2.0.0"            —— 迁移事件句，豁免
//   - 项目更名记录.md:35 "返回 \"2.0.0\"——以 major …"        —— 裸旧版本，漂移
//   - 项目更名记录.md:48 "历史注释追加 2.0.0 条目"            —— 历史标记，豁免
//   - CAPI评审回复        "返回 `1.2.0`（…；1.1.0 为 …）"     —— 多版本串同行
// 若这些样本不再被抓到/豁免，说明规则腐坏，先修规则再动文档。

import (
	"os"
	"path/filepath"
	"strings"
	"testing"
)

const abiTruth = "2.1.0"

// writeAbiFixture 在临时仓库根布下 docs/ 文档并跑 auditConst，返回该文件命中。
func writeAbiFixture(t *testing.T, lines ...string) ConstAudit {
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
		"abi_version": {SValue: abiTruth, Status: "ok"},
	}}
	audits := auditConst(root, doc)
	if len(audits) != 1 {
		t.Fatalf("期望 1 条常量规则，得到 %d", len(audits))
	}
	return audits[0]
}

func countIn(hits []ConstHit) int { return len(hits) }

func TestConstAbiDriftCaught(t *testing.T) {
	a := writeAbiFixture(t,
		"| 出口 1 · C ABI | ✅ 可用；第一批 13 个入口全部落地，`vitro_abi_version()` = 1.2.0 |",
		"`vitro_abi_version()` 返回 `\"2.0.0\"`——以 major 变更表达符号面 breaking；",
	)
	if countIn(a.Drift) != 2 {
		t.Fatalf("两条裸旧版本现值句必须判漂移：得到 Drift=%d Frozen=%d Matched=%d",
			countIn(a.Drift), countIn(a.Frozen), a.Matched)
	}
}

func TestConstAbiMigrationArrowExempt(t *testing.T) {
	a := writeAbiFixture(t,
		"| C ABI（41 入口） | `cide_abi_version()` 等 `cide_*` | `vitro_abi_version()` 等 `vitro_*`，**ABI 版本 1.3.0 → 2.0.0**（符号面 breaking） |",
	)
	if countIn(a.Drift) != 0 || countIn(a.Frozen) != 1 {
		t.Fatalf("迁移箭头事件句必须豁免：得到 Drift=%d Frozen=%d", countIn(a.Drift), countIn(a.Frozen))
	}
}

func TestConstAbiHistoricalWordExempt(t *testing.T) {
	a := writeAbiFixture(t,
		"- ABI 版本历史注释（`native/src/capi/first_batch.rs`）追加 2.0.0 条目；",
	)
	if countIn(a.Drift) != 0 || countIn(a.Frozen) != 1 {
		t.Fatalf("历史标记句必须豁免：得到 Drift=%d Frozen=%d", countIn(a.Drift), countIn(a.Frozen))
	}
}

func TestConstAbiCurrentValueMatch(t *testing.T) {
	a := writeAbiFixture(t,
		"> 已于 U2#13 随 buf 写入式 API（ABI 2.1.0）根治，vet 现为零输出并入 CI 门禁。",
		"项目更名：C ABI（2.1.0）/ CLI / DLL 全量 `vitro_*`。",
	)
	if a.Matched != 2 || countIn(a.Drift) != 0 {
		t.Fatalf("现值等于真值必须记 match：Matched=%d Drift=%d", a.Matched, countIn(a.Drift))
	}
}

func TestConstAbiMultiVersionSameLine(t *testing.T) {
	// 行内既有"现值 1.2.0"又有历史引用 1.1.0：全部版本串 ≠ 真值 → 漂移
	//（修复时人工改写，机器不猜哪段是现值哪段是历史）。
	a := writeAbiFixture(t,
		"| `vitro_abi_version` | ✅ | 返回 `1.2.0`（**加函数 = minor**；1.1.0 为首轮引入） |",
	)
	if countIn(a.Drift) != 1 {
		t.Fatalf("多版本串且全部非真值必须判漂移：Drift=%d", countIn(a.Drift))
	}
}

func TestConstAbiNoTruthNoVerdict(t *testing.T) {
	// 真值不可得时不得妄判（unavailable 由台账自报，缺真值不红——与
	// 数字对账的 Pending 同语义）。
	root := t.TempDir()
	rel := filepath.Join(root, "docs", "current", "01-定位与路线", "夹具2.md")
	if err := os.MkdirAll(filepath.Dir(rel), 0o755); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(rel, []byte("`vitro_abi_version()` = 1.2.0\n"), 0o644); err != nil {
		t.Fatal(err)
	}
	audits := auditConst(root, FactsDoc{Facts: map[string]Fact{}})
	if len(audits) != 1 || audits[0].HasTruth {
		t.Fatalf("真值缺失时 HasTruth 必须为 false（得到 %v）", audits[0].HasTruth)
	}
	if countIn(audits[0].Drift) != 0 || countIn(audits[0].Frozen) != 0 {
		t.Fatalf("缺真值不得判漂移/冻结")
	}
}

func TestCollectAbiVersionReadsConst(t *testing.T) {
	root := t.TempDir()
	p := filepath.Join(root, "native", "src", "capi", "first_batch.rs")
	if err := os.MkdirAll(filepath.Dir(p), 0o755); err != nil {
		t.Fatal(err)
	}
	src := "pub const VITRO_ABI_VERSION: &str = \"2.1.0\";\n"
	if err := os.WriteFile(p, []byte(src), 0o644); err != nil {
		t.Fatal(err)
	}
	facts := map[string]Fact{}
	collectAbiVersion(root, facts)
	f, ok := facts["abi_version"]
	if !ok || f.Status != "ok" || f.SValue != abiTruth {
		t.Fatalf("采集失败: %+v", f)
	}

	// 常量形态变更必须 fail loud（unavailable + how_to_get），禁止静默兜底
	if err := os.WriteFile(p, []byte("pub const VITRO_ABI_VERSION: &str = \"x\";\n"), 0o644); err != nil {
		t.Fatal(err)
	}
	facts = map[string]Fact{}
	collectAbiVersion(root, facts)
	if facts["abi_version"].Status != "unavailable" {
		t.Fatalf("常量解析失败必须 unavailable，得到 %+v", facts["abi_version"])
	}
}

func TestCollectCargoTestFromLog(t *testing.T) {
	root := t.TempDir()
	// 夹具对齐真实 cargo test 输出形态：套件行以 Running 开头且含
	// target\debug\deps\ 路径。
	log := "Running unittests src\\lib.rs (target\\debug\\deps\\foo-abc123.exe)\n" +
		"test result: ok. 900 passed; 0 failed\n" +
		"Running tests\\fuzz_stress_test.rs (target\\debug\\deps\\bar-def456.exe)\n" +
		"test result: ok. 25 passed; 0 failed\n"
	rel := "native/cargo_test_ci.log"
	if err := os.MkdirAll(filepath.Join(root, "native"), 0o755); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(filepath.Join(root, "native", "cargo_test_ci.log"), []byte(log), 0o644); err != nil {
		t.Fatal(err)
	}
	facts := map[string]Fact{}
	collectCargoTestFromLog(root, rel, facts)
	if f := facts["cargo_test_passed"]; f.Value == nil || *f.Value != 925 {
		t.Fatalf("日志解析 900+25=925 失败: %+v", f)
	}
	if f := facts["cargo_test_suites"]; f.Value == nil || *f.Value != 2 {
		t.Fatalf("套件计数解析失败: %+v", f)
	}

	// 日志缺失必须 unavailable，禁止兜底
	facts = map[string]Fact{}
	collectCargoTestFromLog(root, "native/不存在的日志.log", facts)
	if facts["cargo_test_passed"].Status != "unavailable" {
		t.Fatalf("日志缺失必须 unavailable")
	}
}
