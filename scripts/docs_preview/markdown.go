package main

// markdown.go —— 块级解析（CommonMark 子集 + GFM 表格 / 任务列表 / 删除线）。
//
// 覆盖范围按 docs/ 实际用到的构件裁剪（2026-09-18 实测）：
//
//	围栏代码 2459 行 / 表格 10245 行 / ATX 标题 4675 / 有序+无序列表 6012 /
//	引用块 1170 / 分隔线 1661 / 任务列表 515 / HTML 注释 32 / setext 3（实现但极少触发）
//
// 未实现：引用式链接定义（0 处）、图片（0 处）、脚注、数学公式、脚注式 HTML 块类型 1/2 之外的冷门形态。

import (
	"strings"
	"unicode"
)

func splitLines(src string) []string {
	src = strings.ReplaceAll(src, "\r\n", "\n")
	src = strings.ReplaceAll(src, "\r", "\n")
	src = strings.TrimPrefix(src, "\ufeff")
	return strings.Split(src, "\n")
}

func isBlank(s string) bool { return strings.TrimSpace(s) == "" }

func indentOf(s string) int {
	n := 0
	for n < len(s) && s[n] == ' ' {
		n++
	}
	return n
}

func stripIndent(s string, n int) string {
	i := 0
	for i < n && i < len(s) && s[i] == ' ' {
		i++
	}
	return s[i:]
}

// ---------------------------------------------------------------------------
// 块级入口
// ---------------------------------------------------------------------------

func renderMarkdown(ctx *inlineCtx, src string) string {
	return parseBlocks(ctx, splitLines(src))
}

func parseBlocks(ctx *inlineCtx, lines []string) string {
	var out strings.Builder
	i := 0
	for i < len(lines) {
		line := lines[i]
		if isBlank(line) {
			i++
			continue
		}
		if fence, ok := fenceOpen(line); ok {
			body, next := consumeFence(lines, i, fence)
			out.WriteString(renderCodeBlock(fence.lang, body))
			i = next
			continue
		}
		if lvl, text, ok := atxHeading(line); ok {
			out.WriteString(headingHTML(ctx, lvl, text))
			i++
			continue
		}
		if isHR(line) {
			out.WriteString("<hr>\n")
			i++
			continue
		}
		if kind, ok := htmlBlockStart(line); ok {
			body, next := consumeHTMLBlock(lines, i, kind)
			out.WriteString(body)
			out.WriteByte('\n')
			i = next
			continue
		}
		if isBlockquoteLine(line) {
			body, next := consumeBlockquoteLines(lines, i)
			out.WriteString("<blockquote>\n" + parseBlocks(ctx, body) + "</blockquote>\n")
			i = next
			continue
		}
		if _, ok := parseListMarker(line); ok {
			html, next := parseList(ctx, lines, i)
			out.WriteString(html)
			i = next
			continue
		}
		if indentOf(line) >= 4 {
			body, next := consumeIndentedCode(lines, i)
			out.WriteString(`<pre class="code-plain"><code>` + escHTML(body) + "</code></pre>\n")
			i = next
			continue
		}
		if _, ok := tableDelimiter(lines, i); ok {
			html, next := parseTable(ctx, lines, i)
			out.WriteString(html)
			i = next
			continue
		}
		body, next := consumeParagraph(ctx, lines, i, &out)
		if body != "" {
			out.WriteString(body)
		}
		i = next
	}
	return out.String()
}

// consumeParagraph 收集一个段落；若被 setext 下划线收尾则直接按标题输出。
func consumeParagraph(ctx *inlineCtx, lines []string, start int, out *strings.Builder) (string, int) {
	buf := []string{lines[start]}
	i := start + 1
	for i < len(lines) {
		line := lines[i]
		if isBlank(line) {
			break
		}
		if lvl, ok := setextLevel(line); ok {
			out.WriteString(headingHTML(ctx, lvl, strings.Join(buf, "\n")))
			return "", i + 1
		}
		// 表格可以打断段落（GFM / markdown-it 行为）：本仓库大量"加粗小标题 + 紧跟表格"的写法
		if _, ok := tableDelimiter(lines, i); ok {
			break
		}
		if startsBlock(line) {
			break
		}
		buf = append(buf, line)
		i++
	}
	return "<p>" + ctx.render(strings.Join(buf, "\n")) + "</p>\n", i
}

// startsBlock 判断该行是否会打断正在进行的段落。
// 注意两条容易漏的规则（GFM / CommonMark）：
//   - 有序列表要打断段落必须**以 1 开头**（`7. xxx` 紧跟段落时是普通文本）；
//   - 空列表项不能打断段落。
//
// 表格的"能否打断段落"由调用方单独判断（需要看下一行），见 consumeParagraph。
func startsBlock(line string) bool {
	if _, ok := fenceOpen(line); ok {
		return true
	}
	if _, _, ok := atxHeading(line); ok {
		return true
	}
	if isHR(line) {
		return true
	}
	if _, ok := htmlBlockStart(line); ok {
		return true
	}
	if isBlockquoteLine(line) {
		return true
	}
	if m, ok := parseListMarker(line); ok {
		if m.ordered && m.start != 1 {
			return false
		}
		if strings.TrimSpace(m.content) == "" {
			return false
		}
		return true
	}
	return false
}

// startsBlockStrict 用于**收集列表项时的终止判定**：任何列表标记都算块起点。
// 与 startsBlock 的差别在于不受"有序列表须以 1 开头才能打断段落""空项不能打断段落"
// 这两条例外约束——那两条只在"打断正在进行的段落"时生效（markdown-it 的
// isTerminatingParagraph 只在 parentType == paragraph 时成立）。
// 反例：项目符号项后面紧跟 `12. xxx`，若按 startsBlock 判定会被当懒继续行吞进上一个项。
func startsBlockStrict(line string) bool {
	if _, ok := parseListMarker(line); ok {
		return true
	}
	return startsBlock(line)
}

// ---------------------------------------------------------------------------
// 标题 / 分隔线
// ---------------------------------------------------------------------------

func atxHeading(line string) (int, string, bool) {
	s := line
	if indentOf(s) >= 4 {
		return 0, "", false
	}
	s = stripIndent(s, 3)
	n := 0
	for n < len(s) && s[n] == '#' {
		n++
	}
	if n == 0 || n > 6 {
		return 0, "", false
	}
	rest := s[n:]
	if rest != "" && rest[0] != ' ' && rest[0] != '\t' {
		return 0, "", false
	}
	rest = strings.TrimSpace(rest)
	rest = strings.TrimRight(rest, "#")
	rest = strings.TrimRight(rest, " ")
	return n, rest, true
}

func setextLevel(line string) (int, bool) {
	s := strings.TrimSpace(line)
	if s == "" || indentOf(line) >= 4 {
		return 0, false
	}
	if strings.Trim(s, "=") == "" && len(s) >= 1 {
		return 1, true
	}
	if strings.Trim(s, "-") == "" && len(s) >= 1 {
		return 2, true
	}
	return 0, false
}

func isHR(line string) bool {
	s := strings.TrimSpace(line)
	if len(s) < 3 || indentOf(line) >= 4 {
		return false
	}
	var c byte
	for i := 0; i < len(s); i++ {
		ch := s[i]
		if ch == ' ' || ch == '\t' {
			continue
		}
		if c == 0 {
			if ch != '-' && ch != '*' && ch != '_' {
				return false
			}
			c = ch
		} else if ch != c {
			return false
		}
	}
	count := strings.Count(s, string(c))
	return c != 0 && count >= 3
}

func headingHTML(ctx *inlineCtx, level int, text string) string {
	slug := ctx.slug(slugify(plainInline(text)))
	if slug == "" {
		slug = "section"
	}
	tag := "h" + itoa(level)
	return "<" + tag + ` id="` + escAttr(slug) + `">` + ctx.render(text) + "</" + tag + ">\n"
}

// slug 保证同页标题 id 唯一（GitHub 的 -1/-2 去重规则）。
func (c *inlineCtx) slug(base string) string {
	if c.slugs == nil {
		c.slugs = map[string]int{}
	}
	n := c.slugs[base]
	c.slugs[base] = n + 1
	if n == 0 {
		return base
	}
	return base + "-" + itoa(n)
}

// plainInline 去掉行内标记，得到标题的可见文本（供 slug 使用，对齐 github-slugger）。
func plainInline(s string) string {
	var b strings.Builder
	for i := 0; i < len(s); i++ {
		switch s[i] {
		case '`', '*', '_', '~':
			continue // 标记符号不参与 slug
		case '\\':
			if i+1 < len(s) && isASCIIPunct(s[i+1]) {
				b.WriteByte(s[i+1])
				i++
				continue
			}
			b.WriteByte(s[i])
		case '[':
			label, after, ok := bracketLabel(s, i)
			if !ok {
				b.WriteByte(s[i])
				continue
			}
			b.WriteString(label)
			i = after - 1
		default:
			b.WriteByte(s[i])
		}
	}
	return b.String()
}

// bracketLabel 从 s[i]（'['）解析 [label](dest)，返回 label 与结束位置。
func bracketLabel(s string, i int) (string, int, bool) {
	depth := 0
	end := -1
	for j := i; j < len(s); j++ {
		if s[j] == '\\' {
			j++
			continue
		}
		switch s[j] {
		case '[':
			depth++
		case ']':
			depth--
			if depth == 0 {
				end = j
			}
		}
		if end >= 0 {
			break
		}
	}
	if end < 0 {
		return "", 0, false
	}
	label := s[i+1 : end]
	k := end + 1
	for k < len(s) && s[k] == ' ' {
		k++
	}
	if k < len(s) && s[k] == '(' {
		d := 0
		for k < len(s) {
			if s[k] == '(' {
				d++
			}
			if s[k] == ')' {
				d--
				if d == 0 {
					k++
					break
				}
			}
			k++
		}
	}
	return label, k, true
}

// slugify 对齐 GitHub：小写 → 去掉非字母数字（保留汉字/标记/下划线/连字符/空格）→ 空格转连字符。
func slugify(text string) string {
	text = strings.TrimSpace(strings.ToLower(text))
	var b strings.Builder
	for _, r := range text {
		switch {
		case r == ' ' || r == '-' || r == '_':
			b.WriteRune(r)
		case isAlnumRune(r):
			b.WriteRune(r)
		case isMarkRune(r):
			b.WriteRune(r)
		}
	}
	return strings.ReplaceAll(strings.TrimSpace(b.String()), " ", "-")
}

// ---------------------------------------------------------------------------
// 围栏代码
// ---------------------------------------------------------------------------

type fenceInfo struct {
	char   byte
	length int
	lang   string
	indent int
}

func fenceOpen(line string) (fenceInfo, bool) {
	ind := indentOf(line)
	if ind >= 4 {
		return fenceInfo{}, false
	}
	s := stripIndent(line, 3)
	if len(s) < 3 {
		return fenceInfo{}, false
	}
	ch := s[0]
	if ch != '`' && ch != '~' {
		return fenceInfo{}, false
	}
	n := 0
	for n < len(s) && s[n] == ch {
		n++
	}
	if n < 3 {
		return fenceInfo{}, false
	}
	info := strings.TrimSpace(s[n:])
	if ch == '`' && strings.Contains(info, "`") {
		return fenceInfo{}, false
	}
	lang := info
	if sp := strings.IndexAny(info, " \t{"); sp >= 0 {
		lang = info[:sp]
	}
	return fenceInfo{char: ch, length: n, lang: lang, indent: ind}, true
}

func isFenceClose(line string, f fenceInfo) bool {
	if indentOf(line) >= 4 {
		return false
	}
	s := stripIndent(line, 3)
	n := 0
	for n < len(s) && s[n] == f.char {
		n++
	}
	if n < f.length {
		return false
	}
	return strings.TrimSpace(s[n:]) == ""
}

func consumeFence(lines []string, start int, f fenceInfo) (string, int) {
	var body []string
	i := start + 1
	for i < len(lines) {
		if isFenceClose(lines[i], f) {
			return strings.Join(body, "\n"), i + 1
		}
		body = append(body, stripIndent(lines[i], f.indent))
		i++
	}
	return strings.Join(body, "\n"), i
}

func renderCodeBlock(lang, body string) string {
	if lang == "" {
		return `<pre class="code-plain"><code>` + escHTML(body) + "</code></pre>\n"
	}
	return `<pre class="code-hl" data-lang="` + escAttr(lang) + `"><code class="hl">` +
		highlightCode(lang, body) + "</code></pre>\n"
}

// ---------------------------------------------------------------------------
// 缩进代码
// ---------------------------------------------------------------------------

func consumeIndentedCode(lines []string, start int) (string, int) {
	var body []string
	i := start
	for i < len(lines) {
		if isBlank(lines[i]) {
			// 空行只有后面仍是缩进代码时才保留
			j := i
			blanks := 0
			for j < len(lines) && isBlank(lines[j]) {
				j++
				blanks++
			}
			if j < len(lines) && indentOf(lines[j]) >= 4 {
				for k := 0; k < blanks; k++ {
					body = append(body, "")
				}
				i = j
				continue
			}
			break
		}
		if indentOf(lines[i]) < 4 {
			break
		}
		body = append(body, stripIndent(lines[i], 4))
		i++
	}
	return strings.Join(body, "\n"), i
}

// ---------------------------------------------------------------------------
// 引用块
// ---------------------------------------------------------------------------

func isBlockquoteLine(line string) bool {
	if indentOf(line) >= 4 {
		return false
	}
	s := stripIndent(line, 3)
	return strings.HasPrefix(s, ">")
}

func stripQuoteMark(line string) string {
	s := stripIndent(line, 3)
	if !strings.HasPrefix(s, ">") {
		return line
	}
	s = s[1:]
	if strings.HasPrefix(s, " ") {
		s = s[1:]
	}
	return s
}

func consumeBlockquoteLines(lines []string, start int) ([]string, int) {
	var body []string
	i := start
	for i < len(lines) {
		if isBlank(lines[i]) {
			// 引用内的空行：后面仍带 '>' 才继续
			j := i
			blanks := 0
			for j < len(lines) && isBlank(lines[j]) {
				j++
				blanks++
			}
			if j < len(lines) && isBlockquoteLine(lines[j]) {
				for k := 0; k < blanks; k++ {
					body = append(body, "")
				}
				i = j
				continue
			}
			break
		}
		if isBlockquoteLine(lines[i]) {
			body = append(body, stripQuoteMark(lines[i]))
			i++
			continue
		}
		if startsBlock(lines[i]) { // 懒继续
			break
		}
		body = append(body, lines[i])
		i++
	}
	return body, i
}

// ---------------------------------------------------------------------------
// HTML 块
// ---------------------------------------------------------------------------

// htmlBlockStart 返回 (终止条件种类, 是否命中)。
func htmlBlockStart(line string) (string, bool) {
	if indentOf(line) >= 4 {
		return "", false
	}
	s := strings.TrimSpace(line)
	if !strings.HasPrefix(s, "<") {
		return "", false
	}
	lower := strings.ToLower(s)
	switch {
	case strings.HasPrefix(lower, "<!--"):
		return "-->", true
	case strings.HasPrefix(lower, "<?"):
		return "?>", true
	case strings.HasPrefix(lower, "<![cdata["):
		return "]]>", true
	case strings.HasPrefix(lower, "<!"):
		return ">", true
	}
	name := ""
	if strings.HasPrefix(lower, "</") {
		name = tagName(lower)
		if blockTagNames[name] {
			return "", true
		}
		return "", false
	}
	name = tagName(lower)
	if name == "" {
		return "", false
	}
	if name == "script" || name == "pre" || name == "style" || name == "textarea" {
		return "</" + name + ">", true
	}
	if blockTagNames[name] {
		// 必须是合法的标签起始（其后为空白、'>'、'/>' 或行尾）
		rest := strings.TrimPrefix(lower[1:], name)
		if rest == "" || strings.HasPrefix(rest, ">") || strings.HasPrefix(rest, "/>") ||
			strings.HasPrefix(rest, " ") || strings.HasPrefix(rest, "\t") {
			return "", true // 直到空行
		}
	}
	return "", false
}

func consumeHTMLBlock(lines []string, start int, kind string) (string, int) {
	var body []string
	i := start
	for i < len(lines) {
		body = append(body, lines[i])
		if kind != "" {
			if strings.Contains(strings.ToLower(lines[i]), kind) {
				return strings.Join(body, "\n"), i + 1
			}
			i++
			continue
		}
		i++
		if i < len(lines) && isBlank(lines[i]) {
			break
		}
	}
	return strings.Join(body, "\n"), i
}

// ---------------------------------------------------------------------------
// 表格
// ---------------------------------------------------------------------------

// tableDelimiter 判断第 i+1 行是否为分隔行，从而认定第 i 行是表头。
// 口径对齐 GFM / markdown-it：
//   - 分隔行只允许 |-: 与空白，且 '-' 打头时其后不能紧跟空格（避免与列表歧义）；
//   - 每格形如 `:?-+:?`，中间不得出现空格分格；
//   - **分隔行的格数必须与表头完全相同**，否则整张表不成立（本仓库出现过 6 格表头配 7 格
//     分隔行的写法，早期版本因不校验列数而把它渲染成了表）。
func tableDelimiter(lines []string, i int) ([]string, bool) {
	if i+1 >= len(lines) || !strings.Contains(lines[i], "|") {
		return nil, false
	}
	delim := lines[i+1]
	if indentOf(delim) >= 4 {
		return nil, false
	}
	s := strings.TrimSpace(delim)
	if len(s) < 2 {
		return nil, false
	}
	if s[0] != '|' && s[0] != '-' && s[0] != ':' {
		return nil, false
	}
	if s[0] == '-' && (s[1] == ' ' || s[1] == '\t') {
		return nil, false // 与列表歧义
	}
	for k := 0; k < len(s); k++ {
		c := s[k]
		if c != '|' && c != '-' && c != ':' && c != ' ' && c != '\t' {
			return nil, false
		}
	}
	cells := splitRow(delim)
	if len(cells) == 0 {
		return nil, false
	}
	var aligns []string
	for k, c := range cells {
		t := strings.TrimSpace(c)
		if t == "" {
			if k == 0 || k == len(cells)-1 {
				continue // 允许表格左右两侧的空列
			}
			return nil, false
		}
		if strings.Trim(strings.Trim(t, ":"), "-") != "" || !strings.Contains(t, "-") {
			return nil, false
		}
		left := strings.HasPrefix(t, ":")
		right := strings.HasSuffix(t, ":")
		switch {
		case left && right:
			aligns = append(aligns, "center")
		case left:
			aligns = append(aligns, "left")
		case right:
			aligns = append(aligns, "right")
		default:
			aligns = append(aligns, "")
		}
	}
	if len(aligns) == 0 || len(splitRow(lines[i])) != len(aligns) {
		return nil, false
	}
	return aligns, true
}

// splitRow 按 '|' 切分单元格：**未转义的 '|' 一律切分（即便处于代码跨度内）**——
// 这是 GFM 的明确规定（要保留竖线须写 `\|`，写在反引号里也一样），
// 早期版本"保护代码跨度"会导致本仓库大量表格与 GitHub 渲染不一致。
func splitRow(line string) []string {
	line = strings.TrimSpace(line)
	start := 0
	if len(line) > 0 && line[0] == '|' {
		start = 1
	}
	end := len(line)
	if end > start && line[end-1] == '|' && !(end >= 2 && line[end-2] == '\\') {
		end--
	}
	var cells []string
	var cur strings.Builder
	for i := start; i < end; i++ {
		ch := line[i]
		if ch == '\\' && i+1 < end && line[i+1] == '|' {
			cur.WriteByte('|')
			i++
			continue
		}
		if ch == '|' {
			cells = append(cells, cur.String())
			cur.Reset()
			continue
		}
		cur.WriteByte(ch)
	}
	cells = append(cells, cur.String())
	return cells
}

func parseTable(ctx *inlineCtx, lines []string, start int) (string, int) {
	aligns, ok := tableDelimiter(lines, start)
	if !ok {
		return "", start
	}
	header := splitRow(lines[start])
	if len(header) < len(aligns) {
		header = append(header, make([]string, len(aligns)-len(header))...)
	}
	var b strings.Builder
	b.WriteString(`<div class="table-wrap"><table>` + "\n<thead>\n<tr>\n")
	for k := range aligns {
		cell := ""
		if k < len(header) {
			cell = strings.TrimSpace(header[k])
		}
		b.WriteString("<th" + alignAttr(aligns[k]) + ">" + ctx.render(cell) + "</th>\n")
	}
	b.WriteString("</tr>\n</thead>\n")
	i := start + 2
	var rows strings.Builder
	rows.WriteString("<tbody>\n")
	count := 0
	for i < len(lines) {
		if isBlank(lines[i]) || !strings.Contains(lines[i], "|") {
			break
		}
		if _, isDelim := tableDelimiter(lines, i); isDelim {
			break
		}
		cells := splitRow(lines[i])
		rows.WriteString("<tr>\n")
		for k := range aligns {
			cell := ""
			if k < len(cells) {
				cell = strings.TrimSpace(cells[k])
			}
			rows.WriteString("<td" + alignAttr(aligns[k]) + ">" + ctx.render(cell) + "</td>\n")
		}
		rows.WriteString("</tr>\n")
		count++
		i++
	}
	rows.WriteString("</tbody>\n")
	b.WriteString(rows.String())
	b.WriteString("</table></div>\n")
	return b.String(), i
}

func alignAttr(a string) string {
	if a == "" {
		return ""
	}
	return ` align="` + a + `"`
}

// ---------------------------------------------------------------------------
// 列表
// ---------------------------------------------------------------------------

type listMarker struct {
	ordered       bool
	start         int
	content       string
	contentIndent int
	indent        int
}

func parseListMarker(line string) (listMarker, bool) {
	ind := indentOf(line)
	if ind >= 4 {
		return listMarker{}, false
	}
	s := line[ind:]
	if s == "" {
		return listMarker{}, false
	}
	m := listMarker{indent: ind}
	width := 0
	if s[0] == '-' || s[0] == '*' || s[0] == '+' {
		m.ordered = false
		width = 1
	} else if s[0] >= '0' && s[0] <= '9' {
		j := 0
		for j < len(s) && s[j] >= '0' && s[j] <= '9' {
			j++
		}
		if j >= len(s) || (s[j] != '.' && s[j] != ')') {
			return listMarker{}, false
		}
		m.ordered = true
		m.start = atoiSafe(s[:j])
		width = j + 1
	} else {
		return listMarker{}, false
	}
	rest := s[width:]
	if rest != "" && rest[0] != ' ' && rest[0] != '\t' {
		return listMarker{}, false
	}
	spaces := 0
	for spaces < len(rest) && rest[spaces] == ' ' {
		spaces++
	}
	if rest != "" && spaces == 0 {
		return listMarker{}, false
	}
	contentOffset := spaces
	if contentOffset > 4 {
		contentOffset = 1
	}
	m.content = rest[contentOffset:]
	m.contentIndent = ind + width + contentOffset
	return m, true
}

type listRun struct {
	items    [][]string
	marks    []listMarker
	blankGap bool // 项之间存在空行 → 松散列表
	next     int
}

func collectList(lines []string, start int) (listRun, bool) {
	first, ok := parseListMarker(lines[start])
	if !ok {
		return listRun{}, false
	}
	run := listRun{next: start}
	i := start
	for i < len(lines) {
		if isBlank(lines[i]) {
			// 空行：可能是项间分隔（列表继续）或列表结束
			j := i
			for j < len(lines) && isBlank(lines[j]) {
				j++
			}
			if j >= len(lines) {
				run.next = j
				return run, true
			}
			nm, ok2 := parseListMarker(lines[j])
			if ok2 && nm.ordered == first.ordered {
				run.blankGap = true
				i = j
				continue
			}
			// 缩进更深的续行也属于本项
			if indentOf(lines[j]) >= first.contentIndent {
				break
			}
			run.next = i
			return run, true
		}
		mark, ok := parseListMarker(lines[i])
		if !ok || mark.ordered != first.ordered {
			break
		}
		itemLines := []string{mark.content}
		i++
		internalBlank := false
		for i < len(lines) {
			if isBlank(lines[i]) {
				k := i
				for k < len(lines) && isBlank(lines[k]) {
					k++
				}
				if k < len(lines) && indentOf(lines[k]) >= mark.contentIndent {
					itemLines = append(itemLines, "")
					internalBlank = true
					i = k
					continue
				}
				break
			}
			if indentOf(lines[i]) >= mark.contentIndent {
				itemLines = append(itemLines, stripIndent(lines[i], mark.contentIndent))
				i++
				continue
			}
			if nm, ok2 := parseListMarker(lines[i]); ok2 && nm.ordered == first.ordered {
				break
			}
			if startsBlockStrict(lines[i]) {
				break
			}
			itemLines = append(itemLines, strings.TrimLeft(lines[i], " ")) // 懒继续
			i++
		}
		if internalBlank {
			run.blankGap = true
		}
		run.items = append(run.items, itemLines)
		run.marks = append(run.marks, mark)
	}
	run.next = i
	return run, true
}

func parseList(ctx *inlineCtx, lines []string, start int) (string, int) {
	run, ok := collectList(lines, start)
	if !ok || len(run.items) == 0 {
		return "", start
	}
	ordered := run.marks[0].ordered
	var b strings.Builder
	tag := "ul"
	if ordered {
		tag = "ol"
		b.WriteString(`<ol`)
		if run.marks[0].start != 1 {
			b.WriteString(` start="` + itoa(run.marks[0].start) + `"`)
		}
		b.WriteString(">\n")
	} else {
		b.WriteString("<ul>\n")
	}
	for _, itemLines := range run.items {
		task := ""
		if len(itemLines) > 0 {
			trimmed := strings.TrimLeft(itemLines[0], " ")
			if strings.HasPrefix(trimmed, "[ ]") || strings.HasPrefix(strings.ToLower(trimmed), "[x]") {
				checked := strings.HasPrefix(strings.ToLower(trimmed), "[x]")
				sp := trimmed[3:]
				sp = strings.TrimLeft(sp, " \t")
				itemLines[0] = sp
				box := `<input type="checkbox" disabled`
				if checked {
					box += ` checked`
				}
				box += `> `
				task = box
			}
		}
		inner := parseBlocks(ctx, itemLines)
		if !run.blankGap {
			inner = tighten(inner)
		}
		classAttr := ""
		if task != "" {
			classAttr = ` class="task-list-item"`
		}
		b.WriteString("<li" + classAttr + ">" + task + inner + "</li>\n")
	}
	b.WriteString("</" + tag + ">\n")
	return b.String(), run.next
}

// tighten 紧凑列表：去掉项内首/尾段落的 <p> 外壳（GitHub 同款行为）。
func tighten(html string) string {
	s := strings.TrimSpace(html)
	if strings.HasPrefix(s, "<p>") {
		if e := strings.Index(s, "</p>"); e >= 0 {
			s = s[3:e] + s[e+4:]
		}
	}
	if strings.HasSuffix(s, "</p>") {
		if b := strings.LastIndex(s, "<p>"); b >= 0 {
			s = s[:b] + s[b+3:len(s)-4]
		}
	}
	return s
}

// ---------------------------------------------------------------------------
// 小工具
// ---------------------------------------------------------------------------

func itoa(n int) string {
	if n == 0 {
		return "0"
	}
	neg := n < 0
	if neg {
		n = -n
	}
	var buf [20]byte
	i := len(buf)
	for n > 0 {
		i--
		buf[i] = byte('0' + n%10)
		n /= 10
	}
	if neg {
		i--
		buf[i] = '-'
	}
	return string(buf[i:])
}

func atoiSafe(s string) int {
	n := 0
	for i := 0; i < len(s); i++ {
		if s[i] < '0' || s[i] > '9' {
			return n
		}
		n = n*10 + int(s[i]-'0')
	}
	return n
}

// isAlnumRune 对齐 github-slugger 的保留集合：\p{L}（字母，含汉字）与 \p{N}（数字）。
// 注意不能用"CJK 码点区间"这种粗粒度判断——U+3000~U+303F 一带全是中文标点（、。「」等），
// 属于 \p{P}，GitHub 会剔除；按区间保留会让锚点与 GitHub 不一致、站内跳转对不上。
func isAlnumRune(r rune) bool {
	return unicode.IsLetter(r) || unicode.IsNumber(r)
}

func isMarkRune(r rune) bool {
	return unicode.IsMark(r)
}
