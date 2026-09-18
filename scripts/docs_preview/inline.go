package main

// inline.go —— 行内级解析（CommonMark 子集 + GFM 删除线 / 自动链接）。
//
// 设计：多趟扫描 + 占位符暂存。
//
//	① 代码跨度、反斜杠转义、行内原始 HTML、自动链接 先抽取为占位符（\x00N\x00）
//	② 链接 / 图片（此时代码里出现的 [x](y) 已被保护，不会误判）
//	③ 强调 / 删除线（标准定界符栈规则，含 "_" 的词内禁用与左右侧翼判定）
//	④ 收尾：还原占位符 + 转义剩余文本 + 处理硬换行与实体
//
// 占位符在一次文档渲染内全局共享（同一次行内递归复用同一 stash），
// 避免嵌套渲染时索引错位——这是最容易踩的坑。

import (
	"strconv"
	"strings"
	"unicode"
	"unicode/utf8"
)

const ph = byte(0)

// stashEntry 记录被占位符替换掉的片段：html 是替换结果，first/last 是**源码首尾字符**。
// 后者是强调侧翼判定所必需的：`**` 后面紧跟反引号（代码跨度）时属"标点在后"，
// 不能开合；若把占位符当成字母，就会与本实现之外的行为分叉（中文文档里极易触发）。
type stashEntry struct {
	html  string
	first byte
	last  byte
}

// inlineCtx 承载一次文档渲染期间共享的占位符池、标题 id 去重表与链接改写上下文。
type inlineCtx struct {
	stash []stashEntry
	slugs map[string]int
	rw    *rewriter // 链接改写器（在解析期按 token 改写，不做渲染后正则扫描）
	rel   string    // 当前文档的 docs 相对路径
}

// mapHref 把 markdown 里的链接目标改写成预览页地址；无改写器时原样返回。
func (c *inlineCtx) mapHref(dest string) string {
	if c.rw == nil {
		return dest
	}
	return c.rw.mapHref(dest, c.rel)
}

// put 登记一个替换片段（含源码首尾字符，供侧翼判定使用）。
func (c *inlineCtx) put(src, html string) string {
	first, last := byte('A'), byte('A')
	if len(src) > 0 {
		first, last = src[0], src[len(src)-1]
	}
	c.stash = append(c.stash, stashEntry{html: html, first: first, last: last})
	return string(ph) + strconv.Itoa(len(c.stash)-1) + string(ph)
}

// putSynthetic 登记一个由渲染器自己生成的片段（如强调标签）；其首尾字符不参与侧翼判定。
func (c *inlineCtx) putSynthetic(html string) string { return c.put("", html) }

func (c *inlineCtx) stashAt(i int) (stashEntry, bool) {
	if i < 0 || i >= len(c.stash) {
		return stashEntry{}, false
	}
	return c.stash[i], true
}

// renderInline 渲染一段行内文本。
func (c *inlineCtx) render(src string) string {
	if src == "" {
		return ""
	}
	s := c.extractCodeAndEscapes(src)
	s = c.extractRawHTML(s)
	s = c.extractLinks(s)
	s = c.processEmphasis(s)
	return c.finalize(s)
}

// ---------------------------------------------------------------------------
// ① 代码跨度 + 反斜杠转义（**必须单趟左到右处理**）
//
// 顺序敏感：markdown-it 是边前进边判定，转义规则先于代码跨度规则命中，
// 因此 `\`` 永远不可能充当代码跨度的闭合符。早期版本"先抽全部代码跨度、再处理转义"
// 会把被转义的反引号当成闭合符，切出多余的代码跨度（正文出现 `\` 与丢失的反引号）。
// ---------------------------------------------------------------------------

func isBacktick(b byte) bool { return b == '`' }

func (c *inlineCtx) extractCodeAndEscapes(src string) string {
	var b strings.Builder
	i := 0
	for i < len(src) {
		ch := src[i]
		// 转义优先
		if ch == '\\' && i+1 < len(src) {
			next := src[i+1]
			if next == '\n' { // 行末反斜杠 = 硬换行
				b.WriteString(c.put("\n", "<br>"))
				i += 2
				continue
			}
			if isASCIIPunct(next) {
				b.WriteString(c.put(string(next), escHTML(string(next))))
				i += 2
				continue
			}
		}
		if !isBacktick(ch) {
			b.WriteByte(ch)
			i++
			continue
		}
		// 代码跨度：向后找**等长**的反引号串（与 markdown-it 一致，闭合扫描不感知转义）
		j := i
		for j < len(src) && src[j] == '`' {
			j++
		}
		runLen := j - i
		closeAt := -1
		for k := j; k < len(src); {
			if src[k] == '`' {
				m := k
				for m < len(src) && src[m] == '`' {
					m++
				}
				if m-k == runLen {
					closeAt = k
					break
				}
				k = m
				continue
			}
			k++
		}
		if closeAt < 0 { // 未闭合 → 原样保留
			b.WriteString(src[i:j])
			i = j
			continue
		}
		content := src[j:closeAt]
		content = strings.ReplaceAll(content, "\n", " ")
		if len(content) >= 2 && strings.HasPrefix(content, " ") && strings.HasSuffix(content, " ") &&
			strings.TrimSpace(content) != "" {
			content = content[1 : len(content)-1]
		}
		b.WriteString(c.put(src[i:closeAt+runLen], "<code>"+escHTML(content)+"</code>"))
		i = closeAt + runLen
	}
	return b.String()
}

// isASCIIPunct 按 CommonMark 定义（含 !"#$%&'()*+,-./:;<=>?@[\]^_`{|}~）。
func isASCIIPunct(b byte) bool {
	switch {
	case b >= '!' && b <= '/':
		return true
	case b >= ':' && b <= '@':
		return true
	case b >= '[' && b <= '`':
		return true
	case b >= '{' && b <= '~':
		return true
	}
	return false
}

// ---------------------------------------------------------------------------
// ② 行内原始 HTML 与自动链接
// ---------------------------------------------------------------------------

var blockTagNames = map[string]bool{
	"address": true, "article": true, "aside": true, "base": true, "basefont": true,
	"blockquote": true, "body": true, "caption": true, "center": true, "col": true,
	"colgroup": true, "dd": true, "details": true, "dialog": true, "dir": true,
	"div": true, "dl": true, "dt": true, "fieldset": true, "figcaption": true,
	"figure": true, "footer": true, "form": true, "frame": true, "frameset": true,
	"h1": true, "h2": true, "h3": true, "h4": true, "h5": true, "h6": true,
	"head": true, "header": true, "hr": true, "html": true, "iframe": true,
	"legend": true, "li": true, "link": true, "main": true, "menu": true,
	"menuitem": true, "nav": true, "noframes": true, "ol": true, "optgroup": true,
	"option": true, "p": true, "param": true, "search": true, "section": true,
	"summary": true, "table": true, "tbody": true, "td": true, "tfoot": true,
	"th": true, "thead": true, "title": true, "tr": true, "track": true, "ul": true,
}

// scanTag 从 s[i]（'<'）起尝试匹配一个合法的行内 HTML 片段，返回结束位置（-1 表示不合法）。
func scanTag(s string, i int) int {
	if i >= len(s) || s[i] != '<' {
		return -1
	}
	rest := s[i:]
	switch {
	case strings.HasPrefix(rest, "<!--"):
		if e := strings.Index(rest, "-->"); e >= 0 {
			return i + e + 3
		}
		return -1
	case strings.HasPrefix(rest, "<?"):
		if e := strings.Index(rest, "?>"); e >= 0 {
			return i + e + 2
		}
		return -1
	case strings.HasPrefix(rest, "<![CDATA["):
		if e := strings.Index(rest, "]]>"); e >= 0 {
			return i + e + 3
		}
		return -1
	case len(rest) > 2 && rest[1] == '!':
		if e := strings.IndexByte(rest, '>'); e >= 0 {
			return i + e + 1
		}
		return -1
	}
	// </name> / <name ...> / <name/>
	j := i + 1
	if j < len(s) && s[j] == '/' {
		j++
	}
	// CommonMark：标签名必须以字母开头。否则 `<1MB`、`<50` 这类正文比较式
	// 会被当成标签原样透传，再由浏览器吞掉（内容静默消失）。
	if j >= len(s) || !isLetter(s[j]) {
		return -1
	}
	start := j
	for j < len(s) && (isAlphaNum(s[j]) || s[j] == '-') {
		j++
	}
	if j == start {
		return -1
	}
	if j < len(s) && s[j] == '>' {
		return j + 1
	}
	if j < len(s) && s[j] == '/' && j+1 < len(s) && s[j+1] == '>' {
		return j + 2
	}
	if j < len(s) && (s[j] == ' ' || s[j] == '\t' || s[j] == '\n') {
		// 属性区：到 '>' 为止，且不得出现 '<'
		k := j
		for k < len(s) && s[k] != '>' && s[k] != '<' {
			k++
		}
		if k < len(s) && s[k] == '>' {
			return k + 1
		}
	}
	return -1
}

func isAlphaNum(b byte) bool {
	return isLetter(b) || (b >= '0' && b <= '9')
}

func isLetter(b byte) bool {
	return (b >= 'a' && b <= 'z') || (b >= 'A' && b <= 'Z')
}

func isSchemeChar(b byte) bool {
	return isAlphaNum(b) || b == '+' || b == '.' || b == '-'
}

// scanAutolink 处理 <scheme:...> 与 <mail@host>。
func scanAutolink(s string, i int) (int, string) {
	end := strings.IndexByte(s[i:], '>')
	if end < 0 {
		return -1, ""
	}
	end += i
	inner := s[i+1 : end]
	if inner == "" || strings.ContainsAny(inner, " \t\n<") {
		return -1, ""
	}
	if c := strings.IndexByte(inner, ':'); c > 0 {
		scheme := inner[:c]
		if len(scheme) >= 2 && len(scheme) <= 32 {
			ok := true
			for k := 0; k < len(scheme); k++ {
				if k == 0 && !((scheme[k] >= 'a' && scheme[k] <= 'z') || (scheme[k] >= 'A' && scheme[k] <= 'Z')) {
					ok = false
					break
				}
				if !isSchemeChar(scheme[k]) {
					ok = false
					break
				}
			}
			if ok {
				return end + 1, `<a href="` + escAttr(inner) + `">` + escHTML(inner) + `</a>`
			}
		}
	} else if at := strings.IndexByte(inner, '@'); at > 0 && at < len(inner)-1 &&
		!strings.ContainsAny(inner, "@:<>") && !strings.HasPrefix(inner, "@") {
		return end + 1, `<a href="mailto:` + escAttr(inner) + `">` + escHTML(inner) + `</a>`
	}
	return -1, ""
}

func (c *inlineCtx) extractRawHTML(src string) string {
	var b strings.Builder
	i := 0
	for i < len(src) {
		if src[i] != '<' {
			b.WriteByte(src[i])
			i++
			continue
		}
		if e, html := scanAutolink(src, i); e > 0 {
			b.WriteString(c.put(src[i:e], html))
			i = e
			continue
		}
		if e := scanTag(src, i); e > 0 {
			b.WriteString(c.put(src[i:e], src[i:e]))
			i = e
			continue
		}
		b.WriteByte(src[i])
		i++
	}
	return b.String()
}

func tagName(frag string) string {
	j := 1
	if j < len(frag) && frag[j] == '/' {
		j++
	}
	start := j
	for j < len(frag) && (isAlphaNum(frag[j]) || frag[j] == '-') {
		j++
	}
	return strings.ToLower(frag[start:j])
}

// ---------------------------------------------------------------------------
// ③ 链接与图片
// ---------------------------------------------------------------------------

func (c *inlineCtx) extractLinks(src string) string {
	var b strings.Builder
	i := 0
	for i < len(src) {
		isImage := src[i] == '!' && i+1 < len(src) && src[i+1] == '['
		if src[i] != '[' && !isImage {
			b.WriteByte(src[i])
			i++
			continue
		}
		bracket := i
		if isImage {
			bracket = i + 1
		}
		close := matchBracket(src, bracket)
		if close < 0 {
			b.WriteByte(src[i])
			i++
			continue
		}
		dest, title, end, ok := parseLinkTarget(src, close+1)
		if !ok {
			b.WriteByte(src[i])
			i++
			continue
		}
		label := src[bracket+1 : close]
		dest = c.mapHref(dest)
		srcSpan := src[i:end]
		if isImage {
			alt := stripPlaceholders(label)
			b.WriteString(c.put(srcSpan, `<img src="`+escAttr(dest)+`" alt="`+escAttr(alt)+`"`+
				titleAttr(title)+`>`))
		} else {
			b.WriteString(c.put(srcSpan, `<a href="`+escAttr(dest)+`"`+titleAttr(title)+`>`+
				c.render(label)+`</a>`))
		}
		i = end
	}
	return b.String()
}

func titleAttr(title string) string {
	if title == "" {
		return ""
	}
	return ` title="` + escAttr(title) + `"`
}

// matchBracket 返回与 s[open]（'['）配对的 ']' 位置；跳过代码跨度与转义。
func matchBracket(s string, open int) int {
	depth := 0
	for i := open; i < len(s); i++ {
		switch s[i] {
		case '\\':
			i++
		case '`':
			j := i
			for j < len(s) && s[j] == '`' {
				j++
			}
			run := j - i
			for k := j; k < len(s); {
				if s[k] == '`' {
					m := k
					for m < len(s) && s[m] == '`' {
						m++
					}
					if m-k == run {
						i = m - 1
						break
					}
					k = m
					continue
				}
				k++
			}
		case '[':
			depth++
		case ']':
			depth--
			if depth == 0 {
				return i
			}
		case ph:
			// 跳过整个占位符（\x00N\x00），其中的数字不参与定界符判定
			j := i + 1
			for j < len(s) && s[j] != ph {
				j++
			}
			i = j
		}
	}
	return -1
}

// parseLinkTarget 解析 (dest "title")，返回结束位置。
func parseLinkTarget(s string, pos int) (dest, title string, end int, ok bool) {
	for pos < len(s) && (s[pos] == ' ' || s[pos] == '\t') {
		pos++
	}
	if pos >= len(s) || s[pos] != '(' {
		return "", "", 0, false
	}
	pos++
	for pos < len(s) && (s[pos] == ' ' || s[pos] == '\t' || s[pos] == '\n') {
		pos++
	}
	if pos < len(s) && s[pos] == '<' {
		e := strings.IndexByte(s[pos:], '>')
		if e < 0 {
			return "", "", 0, false
		}
		dest = s[pos+1 : pos+e]
		pos += e + 1
	} else {
		depth := 0
		start := pos
		for pos < len(s) {
			ch := s[pos]
			if ch == '\\' {
				pos += 2
				continue
			}
			if ch == '(' {
				depth++
			}
			if ch == ')' {
				if depth == 0 {
					break
				}
				depth--
			}
			if (ch == ' ' || ch == '\t' || ch == '\n') && depth == 0 {
				break
			}
			pos++
		}
		dest = s[start:pos]
	}
	for pos < len(s) && (s[pos] == ' ' || s[pos] == '\t' || s[pos] == '\n') {
		pos++
	}
	if pos < len(s) && (s[pos] == '"' || s[pos] == '\'' || s[pos] == '(') {
		closer := byte('"')
		switch s[pos] {
		case '\'':
			closer = '\''
		case '(':
			closer = ')'
		}
		e := strings.IndexByte(s[pos+1:], closer)
		if e < 0 {
			return "", "", 0, false
		}
		title = s[pos+1 : pos+1+e]
		pos += e + 2
	}
	for pos < len(s) && (s[pos] == ' ' || s[pos] == '\t' || s[pos] == '\n') {
		pos++
	}
	if pos >= len(s) || s[pos] != ')' {
		return "", "", 0, false
	}
	dest = strings.TrimSpace(dest)
	dest = unescapePunct(dest)
	return dest, title, pos + 1, true
}

func unescapePunct(s string) string {
	if !strings.Contains(s, "\\") {
		return s
	}
	var b strings.Builder
	for i := 0; i < len(s); i++ {
		if s[i] == '\\' && i+1 < len(s) && isASCIIPunct(s[i+1]) {
			b.WriteByte(s[i+1])
			i++
			continue
		}
		b.WriteByte(s[i])
	}
	return b.String()
}

// stripPlaceholders 取占位符文本的原始可见内容（供 img alt 使用）。
func stripPlaceholders(s string) string {
	var b strings.Builder
	for i := 0; i < len(s); {
		if s[i] == ph {
			j := i + 1
			for j < len(s) && s[j] != ph {
				j++
			}
			i = j + 1
			continue
		}
		b.WriteByte(s[i])
		i++
	}
	return strings.TrimSpace(b.String())
}

// 裸 URL 自动链接（GFM linkify 的常见形态；不含裸域名）。只在占位符之外的文本上工作。
func (c *inlineCtx) linkifyBareExcludingPlaceholders(s string) string {
	var b strings.Builder
	i := 0
	for i < len(s) {
		if s[i] == ph {
			j := i + 1
			for j < len(s) && s[j] != ph {
				j++
			}
			b.WriteString(s[i : j+1])
			i = j + 1
			continue
		}
		j := i
		for j < len(s) && s[j] != ph {
			j++
		}
		b.WriteString(c.linkifyChunk(s[i:j]))
		i = j
	}
	return b.String()
}

// ---------------------------------------------------------------------------
// ④ 强调 / 删除线（定界符栈）
// ---------------------------------------------------------------------------

type piece struct {
	text        string
	delim       byte // '*' '_' '~'，0 表示普通文本
	open, close bool
	run         int // 剩余定界符字符数
	pre         []string
	post        []string
}

func (c *inlineCtx) processEmphasis(src string) string {
	// 先把裸 URL 链接化（在纯文本段内，且处于保护文本之外）
	src = c.linkifyBareExcludingPlaceholders(src)

	var pieces []piece
	var buf strings.Builder
	flush := func() {
		if buf.Len() > 0 {
			pieces = append(pieces, piece{text: buf.String()})
			buf.Reset()
		}
	}

	for i := 0; i < len(src); {
		ch := src[i]
		if ch != '*' && ch != '_' && ch != '~' {
			buf.WriteByte(ch)
			i++
			continue
		}
		j := i
		for j < len(src) && src[j] == ch {
			j++
		}
		run := j - i
		before := c.runeBefore(src, i)
		after := c.runeAfter(src, j)
		p := piece{delim: ch, run: run}
		lf := !isSpaceForFlank(after) && (!isPunctForFlank(after) || isSpaceForFlank(before) || isPunctForFlank(before))
		rf := !isSpaceForFlank(before) && (!isPunctForFlank(before) || isSpaceForFlank(after) || isPunctForFlank(after))
		switch ch {
		case '*':
			p.open, p.close = lf, rf
		case '_':
			p.open = lf && (!rf || isPunctForFlank(before))
			p.close = rf && (!lf || isPunctForFlank(after))
		case '~':
			// GFM：删除线只认双波浪线
			p.open, p.close = lf && run >= 2, rf && run >= 2
		}
		flush()
		pieces = append(pieces, p)
		i = j
	}
	flush()

	// 配对
	for ci := 0; ci < len(pieces); ci++ {
		cl := &pieces[ci]
		if cl.delim == 0 || !cl.close || cl.run <= 0 {
			continue
		}
		for oi := ci - 1; oi >= 0; oi-- {
			op := &pieces[oi]
			if op.delim != cl.delim || !op.open || op.run <= 0 || op.delim == 0 {
				continue
			}
			// CommonMark "rule of three"：若一方兼具开合能力，则两侧定界符长度之和
			// 为 3 的倍数时不得配对（除非两者都是 3 的倍数）。
			// 缺这条会把 `**…vis_*()…**` 里落单的 `*` 错配进强调，正文被吃掉或搬位。
			if (op.close || cl.open) && (op.run+cl.run)%3 == 0 && (op.run%3 != 0 || cl.run%3 != 0) {
				continue
			}
			use := 1
			if cl.delim == '~' {
				use = 2
			} else if op.run >= 2 && cl.run >= 2 {
				use = 2
			}
			if cl.run < use {
				continue
			}
			op.run -= use
			cl.run -= use
			// 标签必须走占位符池：收尾阶段会把剩余 '<' 统一转义
			switch {
			case cl.delim == '~':
				op.post = append(op.post, c.putSynthetic("<del>"))
				cl.pre = append([]string{c.putSynthetic("</del>")}, cl.pre...)
			case use == 2:
				op.post = append(op.post, c.putSynthetic("<strong>"))
				cl.pre = append([]string{c.putSynthetic("</strong>")}, cl.pre...)
			default:
				op.post = append(op.post, c.putSynthetic("<em>"))
				cl.pre = append([]string{c.putSynthetic("</em>")}, cl.pre...)
			}
			if cl.run <= 0 {
				break
			}
		}
	}

	var out strings.Builder
	for _, p := range pieces {
		if p.delim == 0 {
			out.WriteString(p.text)
			continue
		}
		for _, t := range p.pre {
			out.WriteString(t)
		}
		out.WriteString(strings.Repeat(string(p.delim), p.run))
		for _, t := range p.post {
			out.WriteString(t)
		}
	}
	return out.String()
}

func (c *inlineCtx) linkifyChunk(s string) string {
	var b strings.Builder
	i := 0
	for i < len(s) {
		if s[i] == 'h' && (strings.HasPrefix(s[i:], "http://") || strings.HasPrefix(s[i:], "https://")) &&
			(i == 0 || (!isAlphaNum(s[i-1]) && s[i-1] != '/' && s[i-1] != ':')) {
			j := i
			for j < len(s) && !strings.ContainsRune(" \t\n<>\"", rune(s[j])) {
				j++
			}
			url := s[i:j]
			trimmed := strings.TrimRight(url, ".,;:!?")
			for strings.HasSuffix(trimmed, ")") && strings.Count(trimmed, ")") > strings.Count(trimmed, "(") {
				trimmed = trimmed[:len(trimmed)-1]
			}
			b.WriteString(c.put(trimmed, `<a href="`+escAttr(trimmed)+`">`+escHTML(trimmed)+`</a>`))
			b.WriteString(escHTML(url[len(trimmed):]))
			i = j
			continue
		}
		b.WriteByte(s[i])
		i++
	}
	return b.String()
}

// isSpaceForFlank / isPunctForFlank：按 CommonMark 的 **Unicode** 口径判定
// （空白 = Zs 类别；标点 = P* 或 S* 类别）。
// 中文文档里 `；：，。""（）` 都是 Po/Pi/Pf，若只按 ASCII 判定，夹在全角标点之间的
// 强调（如 `**如实标注为"…"**；`）会被误判成不能闭合 —— 这是中文文档最容易翻车的一处。
func isSpaceForFlank(r rune) bool {
	switch r {
	case ' ', '\t', '\n', '\r', '\f', '\v', 0x00A0, 0x3000:
		return true
	}
	return unicode.Is(unicode.Zs, r)
}

func isPunctForFlank(r rune) bool {
	if r < 128 {
		return isASCIIPunct(byte(r))
	}
	return unicode.IsPunct(r) || unicode.IsSymbol(r)
}

// runeBefore 取 s[i] 之前用于侧翼判定的字符。
// 紧邻占位符时，用的是**被替换掉的源码末字符**（代码跨度 → 反引号＝标点），
// 而不是占位符本身——否则 `注意**`null`…**` 这类写法会被误判成可以开合。
func (c *inlineCtx) runeBefore(s string, i int) rune {
	if i <= 0 {
		return '\n'
	}
	if s[i-1] == ph {
		j := i - 2
		for j >= 0 && s[j] != ph {
			j--
		}
		if j >= 0 {
			if idx, err := strconv.Atoi(s[j+1 : i-1]); err == nil {
				if e, ok := c.stashAt(idx); ok {
					return rune(e.last)
				}
			}
		}
		return 'A'
	}
	j := i - 1
	for j > 0 && s[j]&0xC0 == 0x80 {
		j--
	}
	r, _ := utf8.DecodeRuneInString(s[j:])
	return r
}

func (c *inlineCtx) runeAfter(s string, i int) rune {
	if i >= len(s) {
		return '\n'
	}
	if s[i] == ph {
		j := i + 1
		for j < len(s) && s[j] != ph {
			j++
		}
		if idx, err := strconv.Atoi(s[i+1 : j]); err == nil {
			if e, ok := c.stashAt(idx); ok {
				return rune(e.first)
			}
		}
		return 'A'
	}
	r, _ := utf8.DecodeRuneInString(s[i:])
	return r
}

// ---------------------------------------------------------------------------
// ⑤ 收尾
// ---------------------------------------------------------------------------

func (c *inlineCtx) finalize(s string) string {
	var b strings.Builder
	i := 0
	for i < len(s) {
		ch := s[i]
		switch ch {
		case ph:
			j := i + 1
			for j < len(s) && s[j] != ph {
				j++
			}
			idx, err := strconv.Atoi(s[i+1 : j])
			if err == nil {
				if e, ok := c.stashAt(idx); ok {
					b.WriteString(e.html)
					i = j + 1
					continue
				}
			}
			// 不属于本次 stash（外层扫描的占位符）→ 原样保留待外层还原
			b.WriteString(s[i : j+1])
			i = j + 1
		case '\n':
			trailing := 0
			k := i
			for k > 0 && s[k-1] == ' ' {
				trailing++
				k--
			}
			if trailing >= 2 {
				out := b.String()
				out = strings.TrimRight(out, " ")
				b.Reset()
				b.WriteString(out)
				b.WriteString("<br>\n")
			} else {
				out := b.String()
				if trailing == 1 {
					out = strings.TrimRight(out, " ")
					b.Reset()
					b.WriteString(out)
				}
				b.WriteString("\n")
			}
			i++
		case '<':
			b.WriteString("&lt;")
			i++
		case '>':
			b.WriteString("&gt;")
			i++
		case '&':
			if n := entityLen(s[i:]); n > 0 {
				b.WriteString(s[i : i+n])
				i += n
			} else {
				b.WriteString("&amp;")
				i++
			}
		default:
			b.WriteByte(ch)
			i++
		}
	}
	return b.String()
}

// entityLen 返回合法 HTML 实体的长度（0 表示不合法）。
func entityLen(s string) int {
	if len(s) < 3 || s[0] != '&' {
		return 0
	}
	if s[1] == '#' {
		j := 2
		if j < len(s) && (s[j] == 'x' || s[j] == 'X') {
			j++
			start := j
			for j < len(s) && isHex(s[j]) {
				j++
			}
			if j > start && j < len(s) && s[j] == ';' && j-start <= 6 {
				return j + 1
			}
			return 0
		}
		start := j
		for j < len(s) && s[j] >= '0' && s[j] <= '9' {
			j++
		}
		if j > start && j < len(s) && s[j] == ';' && j-start <= 7 {
			return j + 1
		}
		return 0
	}
	j := 1
	for j < len(s) && isAlphaNum(s[j]) {
		j++
	}
	if j > 1 && j < len(s) && s[j] == ';' && j-1 <= 31 {
		return j + 1
	}
	return 0
}

func isHex(b byte) bool {
	return (b >= '0' && b <= '9') || (b >= 'a' && b <= 'f') || (b >= 'A' && b <= 'F')
}

// ---------------------------------------------------------------------------
// 转义与文本提取
// ---------------------------------------------------------------------------

func escHTML(s string) string {
	var b strings.Builder
	for i := 0; i < len(s); i++ {
		switch s[i] {
		case '&':
			b.WriteString("&amp;")
		case '<':
			b.WriteString("&lt;")
		case '>':
			b.WriteString("&gt;")
		default:
			b.WriteByte(s[i])
		}
	}
	return b.String()
}

func escAttr(s string) string {
	var b strings.Builder
	for i := 0; i < len(s); i++ {
		switch s[i] {
		case '&':
			b.WriteString("&amp;")
		case '<':
			b.WriteString("&lt;")
		case '>':
			b.WriteString("&gt;")
		case '"':
			b.WriteString("&quot;")
		default:
			b.WriteByte(s[i])
		}
	}
	return b.String()
}
