package main

// 回归测试：同一行被多条规则命中时，逐条 applyHit 的字节偏移必须仍然正确。
//
// 背景：applyHit 按 `Hit.Spans`（audit 阶段算出的字节偏移）定位并替换数字。
// 若同一行先替换了长度不同的数字（如 100→97 少 1 字节），则该行后续数字的
// 偏移整体前移，而第二条 Hit 仍持有旧偏移 —— 替换会切在错误位置。
//
// 这是 audit.go 与 main.go 的契约测试：no drift 时不涉及，一旦同行多命中就会踩。

import (
	"os"
	"path/filepath"
	"strings"
	"testing"
)

// TestApplyHitSameLineOffset 验证同行两处替换后，行内容仍完全正确。
//
// 夹具选取真实可发生的同行双命中：该行同时含 "Shadow"（命中 shadow_c_cases）
// 与 "serve 冒烟"（命中 serve_smoke_assertions），且都不被对方规则 exclude。
// 636→671 为 +1 字节，正是会让后续偏移失真的情形。
func TestApplyHitSameLineOffset(t *testing.T) {
	root := t.TempDir()
	const file = "t.md"
	line := "- 测试防线：C Shadow 636 用例、serve 冒烟 40 项断言，clippy 零警告"
	if err := os.WriteFile(filepath.Join(root, file), []byte(line+"\n"), 0o644); err != nil {
		t.Fatal(err)
	}

	i636 := strings.Index(line, "636")
	i40 := strings.Index(line, "40 项")
	if i636 < 0 || i40 < 0 {
		t.Fatal("测试夹具构造失败")
	}

	// 与 auditDocs 的形状一致：两条独立 Hit，各持 audit 时刻算出的偏移。
	h1 := Hit{Key: "shadow_c_cases", File: file, LineNo: 1,
		Spans: []numSpan{{start: i636, end: i636 + 3, value: 636}}}
	h2 := Hit{Key: "serve_smoke_assertions", File: file, LineNo: 1,
		Spans: []numSpan{{start: i40, end: i40 + 2, value: 40}}}

	if err := applyHit(root, h1, 671); err != nil {
		t.Fatal(err)
	}
	if err := applyHit(root, h2, 51); err != nil {
		t.Fatal(err)
	}

	got, err := os.ReadFile(filepath.Join(root, file))
	if err != nil {
		t.Fatal(err)
	}
	want := "- 测试防线：C Shadow 671 用例、serve 冒烟 51 项断言，clippy 零警告"
	if strings.TrimSpace(string(got)) != want {
		t.Errorf("同行二次替换结果错误（偏移失效）\n得到: %q\n期望: %q",
			strings.TrimSpace(string(got)), want)
	}
}

// TestApplyHitRejectsChangedLine 验证安全网：前序条目若把行改得不再匹配本规则，
// 必须报错拒改，而不是按陈旧偏移乱切。
func TestApplyHitRejectsChangedLine(t *testing.T) {
	root := t.TempDir()
	const file = "t3.md"
	// 该行不含 "C++"，却被伪造一条 cpp_e2e_cases 的 Hit —— 应当被拒。
	line := "- 某行已被前序条目改写，不再含目标关键词"
	if err := os.WriteFile(filepath.Join(root, file), []byte(line+"\n"), 0o644); err != nil {
		t.Fatal(err)
	}
	h := Hit{Key: "cpp_e2e_cases", File: file, LineNo: 1,
		Spans: []numSpan{{start: 0, end: 1, value: 7}}}
	if err := applyHit(root, h, 81); err == nil {
		t.Error("行不再匹配规则时应当报错，却静默通过了")
	}
}

// TestApplyHitIsLengthSafe 验证单条 Hit 内部多个 span 的替换不受长度变化影响
// （applyHit 内部倒序替换，这一点应当成立）。
func TestApplyHitIsLengthSafe(t *testing.T) {
	root := t.TempDir()
	const file = "t2.md"
	line := "C Shadow 636 用例 / 636 通过"
	if err := os.WriteFile(filepath.Join(root, file), []byte(line+"\n"), 0o644); err != nil {
		t.Fatal(err)
	}
	a := strings.Index(line, "636")
	b := strings.LastIndex(line, "636")
	h := Hit{Key: "shadow_c_cases", File: file, LineNo: 1,
		Spans: []numSpan{{a, a + 3, 636}, {b, b + 3, 636}}}
	if err := applyHit(root, h, 671); err != nil {
		t.Fatal(err)
	}
	got, _ := os.ReadFile(filepath.Join(root, file))
	want := "C Shadow 671 用例 / 671 通过"
	if strings.TrimSpace(string(got)) != want {
		t.Errorf("单条多 span 替换错误\n得到: %q\n期望: %q",
			strings.TrimSpace(string(got)), want)
	}
}
