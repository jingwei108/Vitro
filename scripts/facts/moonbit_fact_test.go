package main

import "testing"

// T6（2026-09-19）：MoonBit 测试真值解析器——绿路径 + J9 埋雷（假输出 /
// 无 Total 行 / 失败计数非零时的字段语义），红证随 go test 留痕。

func TestMoonbitParseGreen(t *testing.T) {
	facts := map[string]Fact{}
	collectMoonbitFromOutput("Total tests: 58, passed: 58, failed: 0", 0, facts)
	f, ok := facts["moonbit_test_passed"]
	if !ok || f.Status != "ok" || f.Value == nil || *f.Value != 58 {
		t.Fatalf("绿路径解析失败: %+v", f)
	}
}

func TestMoonbitParseFailureRunStillCounts(t *testing.T) {
	// 失败计数非零：passed 是真值（诚实记录失败量，不粉饰为 ok 之外的状态）
	facts := map[string]Fact{}
	collectMoonbitFromOutput("Total tests: 10, passed: 8, failed: 2", 1, facts)
	f := facts["moonbit_test_passed"]
	if f.Status != "ok" || *f.Value != 8 {
		t.Fatalf("失败形态应记 passed=8: %+v", f)
	}
}

func TestMoonbitParseNoTotalLine(t *testing.T) {
	// J9 埋雷：编译失败输出（无 Total 行）必须 unavailable，禁止兜底 0
	facts := map[string]Fact{}
	collectMoonbitFromOutput("Error: [3002] parse error ...", 1, facts)
	f, ok := facts["moonbit_test_passed"]
	if !ok || f.Status != "unavailable" {
		t.Fatalf("无 Total 行应 unavailable: %+v", f)
	}
}

func TestMoonbitParseEmptyOutput(t *testing.T) {
	// J9 埋雷：空输出 unavailable
	facts := map[string]Fact{}
	collectMoonbitFromOutput("", 0, facts)
	if f := facts["moonbit_test_passed"]; f.Status != "unavailable" {
		t.Fatalf("空输出应 unavailable: %+v", f)
	}
}
