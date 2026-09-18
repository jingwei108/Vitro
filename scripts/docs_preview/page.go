package main

// page.go —— 页面骨架、左侧目录树、右侧大纲、目录清单页。

import (
	"regexp"
	"sort"
	"strings"
	"time"
)

const pageTemplate = `<!DOCTYPE html>
<html lang="zh-CN" data-color-mode="light" data-preview="vitro-docs-preview">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<meta name="generator" content="vitro docs_preview">
<meta name="doc-source" content="__SOURCE__">
<title>__TITLE__ · Vitro docs</title>
<link rel="stylesheet" href="__ASSETS__style.css">
</head>
<body>
<header class="topbar">
  <button class="icon-btn" id="nav-toggle" type="button" title="显示 / 隐藏目录树" aria-label="显示或隐藏目录树">☰</button>
  <div class="crumbs" title="__SOURCE__">__CRUMBS__</div>
  <div class="topbar-right">
    <a class="icon-btn gh" href="__GHURL__" target="_blank" rel="noopener" title="在 GitHub 上查看源文件">GitHub ↗</a>
    <button class="icon-btn" id="theme-toggle" type="button" title="切换明暗主题（自动 / 浅色 / 深色）">◐</button>
    <button class="icon-btn" id="outline-toggle" type="button" title="显示 / 隐藏大纲">☰☰</button>
  </div>
</header>
<div class="layout" id="layout">
  <aside class="sidebar" id="sidebar">
    <div class="search-box">
      <input id="search" type="search" placeholder="检索标题 / 标题级 / 正文（按 / 聚焦）" autocomplete="off">
      <div class="search-hits" id="search-hits" hidden></div>
    </div>
    __NAV__
  </aside>
  <main class="content">
    <div class="doc-head">
      <div class="chips">__CHIPS__</div>
      <div class="doc-meta">
        <span title="源文件">__SOURCE__</span>
        <span class="dot">·</span><span>修改 __MTIME__</span>
        <span class="dot">·</span><span>__LINES__ 行</span>
        <span class="dot">·</span><span>__SIZE__</span>
      </div>
    </div>
    <article class="markdown-body" id="doc">__BODY__</article>
    <footer class="doc-foot">
      <span>本页是 __BUILT__ 从 git 工作区快照生成的本地静态预览，不是仓库内容本身。</span>
      <span>源文件：<code>__SOURCE__</code></span>
    </footer>
  </main>
  <aside class="outline-wrap" id="outline-wrap">
    <div class="outline-title">大纲</div>
    <nav class="outline" id="outline">__OUTLINE__</nav>
  </aside>
</div>
<script src="__ASSETS__app.js" defer></script>
</body>
</html>
`

type pageTemplateData struct {
	title   string
	source  string
	assets  string
	nav     string
	chips   string
	mtime   string
	lines   string
	size    string
	body    string
	outline string
	ghURL   string
	crumbs  string
}

func renderPage(d pageTemplateData) string {
	r := strings.NewReplacer(
		"__TITLE__", escHTML(d.title),
		"__SOURCE__", escHTML(d.source),
		"__ASSETS__", d.assets,
		"__NAV__", d.nav,
		"__CHIPS__", d.chips,
		"__MTIME__", d.mtime,
		"__LINES__", d.lines,
		"__SIZE__", d.size,
		"__BODY__", d.body,
		"__OUTLINE__", d.outline,
		"__GHURL__", d.ghURL,
		"__CRUMBS__", escHTML(d.crumbs),
		"__BUILT__", time.Now().Format("2006-01-02 15:04"),
	)
	return r.Replace(pageTemplate)
}

// ---------------------------------------------------------------------------
// 目录树
// ---------------------------------------------------------------------------

type navNode struct {
	rel   string
	dirs  map[string]*navNode
	order []string
	files []*doc
}

func renderNav(docs []*doc, dirs []string, dirMap map[string]string, pageDir, activeRel string) string {
	root := &navNode{dirs: map[string]*navNode{}}
	for _, d := range dirs {
		if d == "." {
			continue
		}
		cur := root
		for _, part := range strings.Split(d, "/") {
			cur = cur.child(part)
		}
		cur.rel = d
	}
	for _, d := range docs {
		cur := root
		parts := strings.Split(d.rel, "/")
		for _, part := range parts[:len(parts)-1] {
			cur = cur.child(part)
		}
		cur.files = append(cur.files, d)
	}

	var b strings.Builder
	b.WriteString(`<ul class="nav-list nav-root">`)
	sort.Slice(root.files, func(i, j int) bool {
		return naturalLess(strings.ToLower(root.files[i].rel), strings.ToLower(root.files[j].rel))
	})
	for _, d := range root.files {
		class := "nav-link nav-root-link"
		if d.rel == activeRel {
			class += " is-active"
		}
		b.WriteString(`<li><a class="` + class + `" href="` + relHref(pageDir, d.out) + `">` +
			escHTML(d.title) + `</a></li>`)
	}
	names := append([]string(nil), root.order...)
	sort.Slice(names, func(i, j int) bool { return naturalLess(names[i], names[j]) })
	for _, name := range names {
		writeNavNode(&b, root.dirs[name], dirMap, pageDir, activeRel)
	}
	b.WriteString(`</ul>`)
	return b.String()
}

func (n *navNode) child(name string) *navNode {
	if c, ok := n.dirs[name]; ok {
		return c
	}
	c := &navNode{dirs: map[string]*navNode{}}
	n.dirs[name] = c
	n.order = append(n.order, name)
	return c
}

func writeNavNode(b *strings.Builder, n *navNode, dirMap map[string]string, pageDir, activeRel string) {
	open := true
	if strings.HasPrefix(n.rel, "archive") {
		open = false
	}
	active := activeRel != "" && n.rel != "" && strings.HasPrefix(activeRel+"/", n.rel+"/")
	if active {
		open = true
	}
	b.WriteString(`<li class="nav-dir"><details`)
	if open {
		b.WriteString(` open`)
	}
	b.WriteString(` data-group="` + escAttr(n.rel) + `"><summary>`)
	if page, ok := dirMap[n.rel]; ok {
		b.WriteString(`<a class="nav-dir-a" href="` + relHref(pageDir, page) + `">` +
			escHTML(dirLabel(n.rel)) + `</a>`)
	} else {
		b.WriteString(`<span class="nav-dir-a">` + escHTML(dirLabel(n.rel)) + `</span>`)
	}
	b.WriteString(`<span class="nav-cnt">` + strconvItoa(countNav(n)) + `</span></summary><ul class="nav-list">`)

	sort.Slice(n.files, func(i, j int) bool {
		return naturalLess(strings.ToLower(n.files[i].rel), strings.ToLower(n.files[j].rel))
	})
	for _, d := range n.files {
		if d.rel == "README.md" {
			continue
		}
		class := "nav-link"
		if d.rel == activeRel {
			class += " is-active"
		}
		title := d.rel + "（" + d.mtime.Format("2006-01-02 15:04") + "）"
		b.WriteString(`<li><a class="` + class + `" href="` + relHref(pageDir, d.out) + `" title="` +
			escAttr(title) + `">` + escHTML(d.title) + `</a></li>`)
	}
	names := append([]string(nil), n.order...)
	sort.Slice(names, func(i, j int) bool { return naturalLess(names[i], names[j]) })
	for _, name := range names {
		writeNavNode(b, n.dirs[name], dirMap, pageDir, activeRel)
	}
	b.WriteString(`</ul></details></li>`)
}

func countNav(n *navNode) int {
	total := len(n.files)
	for _, c := range n.dirs {
		total += countNav(c)
	}
	return total
}

func strconvItoa(n int) string {
	if n == 0 {
		return "0"
	}
	var buf [20]byte
	i := len(buf)
	for n > 0 {
		i--
		buf[i] = byte('0' + n%10)
		n /= 10
	}
	return string(buf[i:])
}

func dirLabel(rel string) string {
	if rel == "." || rel == "" {
		return "docs"
	}
	if i := strings.LastIndex(rel, "/"); i >= 0 {
		return rel[i+1:]
	}
	return rel
}

func chipsFor(rel string) string {
	if rel == "README.md" {
		return `<span class="chip chip-root">文档索引</span>`
	}
	top := rel
	if i := strings.Index(rel, "/"); i >= 0 {
		top = rel[:i]
	}
	switch top {
	case "current":
		return `<span class="chip chip-current">当前有效</span>`
	case "spec":
		return `<span class="chip chip-spec">协议承诺</span>`
	case "archive":
		return `<span class="chip chip-archive">历史归档</span>` +
			`<span class="chip chip-warn">内容可能已过时 · 不再维护</span>`
	default:
		return `<span class="chip chip-other">` + escHTML(top) + `</span>`
	}
}

// ---------------------------------------------------------------------------
// 大纲
// ---------------------------------------------------------------------------

var (
	headingRe = regexp.MustCompile(`(?s)<h([1-6])([^>]*)>(.*?)</h[1-6]>`)
	idRe      = regexp.MustCompile(`\sid="([^"]*)"`)
	// RE2 不支持反向引用，script/style 用显式分支
	tagRe = regexp.MustCompile(`(?s)<script[^>]*>.*?</script>|<style[^>]*>.*?</style>|<[^>]+>`)
	h1Re  = regexp.MustCompile(`(?s)<h1[^>]*>(.*?)</h1>`)
)

func extractOutline(html string, maxLevel int) []outlineItem {
	var items []outlineItem
	for _, m := range headingRe.FindAllStringSubmatch(html, -1) {
		level := int(m[1][0] - '0')
		if level > maxLevel {
			continue
		}
		id := ""
		if sm := idRe.FindStringSubmatch(m[2]); sm != nil {
			id = sm[1]
		}
		text := strings.TrimSpace(unescapeEntities(tagRe.ReplaceAllString(m[3], "")))
		if text == "" || id == "" {
			continue
		}
		items = append(items, outlineItem{Level: level, ID: id, Text: text})
	}
	return items
}

func renderOutline(items []outlineItem) string {
	if len(items) == 0 {
		return `<div class="outline-empty">（本文无标题）</div>`
	}
	var b strings.Builder
	for _, it := range items {
		b.WriteString(`<a class="lv` + strconvItoa(it.Level) + `" href="#` + escAttr(it.ID) + `">` +
			escHTML(it.Text) + `</a>`)
	}
	return b.String()
}

func htmlToText(html string) string {
	t := tagRe.ReplaceAllString(html, " ")
	t = unescapeEntities(t)
	var b strings.Builder
	space := false
	for _, r := range t {
		if r == ' ' || r == '\t' || r == '\n' || r == '\u00a0' {
			if !space {
				b.WriteByte(' ')
				space = true
			}
			continue
		}
		space = false
		b.WriteRune(r)
	}
	return strings.TrimSpace(b.String())
}

func firstHeadingText(html string) string {
	if m := h1Re.FindStringSubmatch(html); m != nil {
		return strings.TrimSpace(unescapeEntities(tagRe.ReplaceAllString(m[1], "")))
	}
	return ""
}

// ---------------------------------------------------------------------------
// 目录清单页
// ---------------------------------------------------------------------------

func renderDirPage(docs []*doc, subdirs []string, rel string) string {
	var b strings.Builder
	b.WriteString(`<h1 id="dir-index">` + escHTML(dirLabel(rel)) + `/ 目录</h1>` + "\n")
	b.WriteString(`<p>本目录共 ` + strconvItoa(len(docs)) + ` 篇文档`)
	if len(subdirs) > 0 {
		b.WriteString(`、` + strconvItoa(len(subdirs)) + ` 个子目录`)
	}
	b.WriteString(`。</p>` + "\n")
	b.WriteString(`<div class="table-wrap"><table><thead><tr><th>文件</th><th>标题</th><th>修改时间</th><th>大小</th></tr></thead><tbody>`)
	for _, d := range subdirs {
		b.WriteString(`<tr><td class="col-n"><a href="` + encodeSegment(dirLabel(d)) + `/index.html">` +
			escHTML(dirLabel(d)) + `/</a></td><td class="col-t">目录</td><td class="col-m">—</td><td class="col-m">—</td></tr>`)
	}
	sort.Slice(docs, func(i, j int) bool {
		return naturalLess(strings.ToLower(docs[i].rel), strings.ToLower(docs[j].rel))
	})
	for _, d := range docs {
		name := pathBase(d.rel)
		b.WriteString(`<tr><td class="col-n"><a href="` + encodeSegment(strings.TrimSuffix(name, ".md")) + `.html">` +
			escHTML(name) + `</a></td><td class="col-t">` + escHTML(d.title) +
			`</td><td class="col-m">` + d.mtime.Format("2006-01-02 15:04") +
			`</td><td class="col-m">` + humanSize(d.size) + `</td></tr>`)
	}
	b.WriteString(`</tbody></table></div>` + "\n")
	return b.String()
}

func pathBase(p string) string {
	if i := strings.LastIndex(p, "/"); i >= 0 {
		return p[i+1:]
	}
	return p
}
