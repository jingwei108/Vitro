package main

// markdown_test.go —— 渲染器回归用例。
//
// 每条用例都对应一个**实测踩过的坑**（对照另一套独立实现 Python markdown-it 逐篇对账时发现），
// 不是为覆盖率而写的空测。改动渲染逻辑后应保证这些用例仍绿。

import (
	"strings"
	"testing"
)

// bt 反引号：Go 字符串里直接用裸反引号会与原始字符串字面量冲突。
const bt = "\x60"

func render(t *testing.T, src string) string {
	t.Helper()
	ctx := &inlineCtx{}
	if strings.HasPrefix(src, "BLOCK:") {
		return renderMarkdown(ctx, strings.TrimPrefix(src, "BLOCK:"))
	}
	return ctx.render(src)
}

func TestInlineEmphasis(t *testing.T) {
	cases := []struct {
		name string
		in   string
		want string
	}{
		{"粗体", "**abc**", "<strong>abc</strong>"},
		{"斜体", "*abc*", "<em>abc</em>"},
		{"删除线", "~~abc~~", "<del>abc</del>"},
		// rule of three：`vis_*` 里落单的 * 若与 ** 配对，正文会被吃掉/挪位
		{"rule of three", "**零侵入（不写 vis_*()）**", "<strong>零侵入（不写 vis_*()）</strong>"},
		// 全角标点是 Unicode 标点：只按 ASCII 判定会让强调无法闭合
		{"全角标点收尾", `**如实标注为"待办"**；`, `<strong>如实标注为"待办"</strong>；`},
		{"下划线词内不强调", "next_local_offset 与 a_b_c", "next_local_offset 与 a_b_c"},
	}
	for _, c := range cases {
		if got := render(t, c.in); got != c.want {
			t.Errorf("%s：\n  in   %q\n  got  %q\n  want %q", c.name, c.in, got, c.want)
		}
	}
}

func TestInlineCodeAndEscape(t *testing.T) {
	// 转义优先于代码跨度：`\`` 不能充当闭合符（否则会多切出一段代码跨度）
	in := "格式 " + bt + `"#if \` + bt + `expr\` + bt + " → 真\"" + bt
	want := "格式 " + `<code>"#if \` + `</code>` + "expr" + bt + " → 真\"" + bt
	if got := render(t, in); got != want {
		t.Errorf("转义与代码跨度顺序：\n  in   %q\n  got  %q\n  want %q", in, got, want)
	}
}

func TestInlineHTMLDetection(t *testing.T) {
	cases := []struct{ name, in, want string }{
		// 标签名必须是字母开头：<1MB / <50 是正文，不是 HTML 标签
		{"比较符不是标签", "RSS 增量 <1MB 且 >50 检查点", "RSS 增量 &lt;1MB 且 &gt;50 检查点"},
		{"合法行内标签透传", "写 <br> 换行", "写 <br> 换行"},
	}
	for _, c := range cases {
		if got := render(t, c.in); got != c.want {
			t.Errorf("%s：\n  in   %q\n  got  %q\n  want %q", c.name, c.in, got, c.want)
		}
	}
}

func TestCodeBlockEscapesRawText(t *testing.T) {
	// 代码块里的 <u32> / <class T> 必须转义，否则被浏览器当标签吞掉（内容静默消失）
	out := render(t, "BLOCK:```cpp\nOption<u32> f(vector<class T>& v);\n```\n")
	if strings.Contains(out, "<u32>") || strings.Contains(out, "<class T>") {
		t.Errorf("代码块未转义尖括号：%s", out)
	}
	if !strings.Contains(out, "&lt;u32&gt;") {
		t.Errorf("代码块应输出 &lt;u32&gt;：%s", out)
	}
}

func TestTableRules(t *testing.T) {
	// 分隔行列数必须与表头一致，否则整张表不成立
	mismatch := render(t, "BLOCK:| a | b |\n|---|------|---|\n| 1 | 2 | 3 |\n")
	if strings.Contains(mismatch, "<table>") {
		t.Errorf("列数不一致的表不该成立：%s", mismatch)
	}
	ok := render(t, "BLOCK:| a | b |\n|---|---|\n| 1 | 2 |\n")
	if !strings.Contains(ok, "<table>") {
		t.Errorf("正常表格应成立：%s", ok)
	}
	// 表格可以打断段落（本仓库大量"加粗小标题 + 紧跟表格"的写法）
	para := render(t, "BLOCK:**标题**:\n| a | b |\n|---|---|\n| 1 | 2 |\n")
	if !strings.Contains(para, "<table>") || !strings.Contains(para, "<p>") {
		t.Errorf("表格应能打断段落：%s", para)
	}
	// GFM：未转义的 | 一律切分单元格（即便在代码跨度内）
	row := splitRow("| " + bt + "a|b" + bt + " | c |")
	if len(row) != 3 {
		t.Errorf("代码跨度内的竖线也应切分，得到 %d 格：%q", len(row), row)
	}
	esc := splitRow(`| a \| b | c |`)
	if len(esc) != 2 || !strings.Contains(esc[0], "|") {
		t.Errorf(`\| 应还原为字面竖线且不切分：%q`, esc)
	}
}

func TestListInterruptParagraph(t *testing.T) {
	// 有序列表要打断段落必须以 1 开头
	seven := render(t, "BLOCK:第二批（CI 门禁，1 天）\n7. E-P0-1：补 exit code\n")
	if strings.Contains(seven, "<ol") || strings.Contains(seven, "<li") {
		t.Errorf("7. 不应打断段落，应留在段落文本里：%s", seven)
	}
	one := render(t, "BLOCK:第二批（CI 门禁，1 天）\n1. E-P0-1：补 exit code\n")
	if !strings.Contains(one, "<ol") {
		t.Errorf("1. 应能打断段落形成列表：%s", one)
	}
	// 空列表项不能打断段落
	empty := render(t, "BLOCK:说明如下\n- \n")
	if strings.Contains(empty, "<ul") {
		t.Errorf("空列表项不应打断段落：%s", empty)
	}
}

func TestHeadingAnchorSlug(t *testing.T) {
	got := render(t, "BLOCK:## §14 JIT trace 路径 P0 静默错值\n")
	if !strings.Contains(got, `id="14-jit-trace-路径-p0-静默错值"`) {
		t.Errorf("标题锚点应对齐 GitHub slug 规则：%s", got)
	}
	// 中文标点属 \p{P}，GitHub 会剔除；用码点区间判断会保留下来，导致站内锚点对不上
	got2 := render(t, "BLOCK:#### 定位、路线与架构（含 C++/CLI）\n")
	if !strings.Contains(got2, `id="定位路线与架构含-ccli"`) {
		t.Errorf("中文标点应从锚点中剔除：%s", got2)
	}
}
