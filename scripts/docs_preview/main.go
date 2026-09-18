// Command docs_preview 把 docs/ 下的全部 Markdown 预渲染成一组纯静态 HTML，
// 双击 index.html 即可离线浏览（file:// 协议），样式对齐 GitHub 的 Markdown 视图。
//
// 用法（仓库根目录）：
//
//	go run ./scripts/docs_preview              # 增量构建（默认输出 tmp/docs_preview）
//	go run ./scripts/docs_preview -open        # 构建后打开首页
//	go run ./scripts/docs_preview -watch       # 常驻监听：新增 / 修改 / 删除 md 自动重建
//	go run ./scripts/docs_preview -force       # 忽略缓存全量重建
//	go run ./scripts/docs_preview -out DIR     # 自定义输出目录
//
// 产物（默认 tmp/docs_preview/，已被 .gitignore 忽略，不污染仓库）：
//
//	index.html               docs/README.md 的渲染结果（入口）
//	<原目录结构>/*.html       每篇 md 一个页面
//	<目录>/index.html        无 README 的目录 = 目录清单页
//	assets/                  样式 / 脚本 / 检索索引（embed 进二进制，构建时释放）
//	_report.txt              构建报告：文档数、断链明细（诚实记录，不静默吞掉）
//
// 纪律：零第三方依赖（仅标准库）；`go vet ./scripts/...` 必须干净。
//
// # 渲染口径与验证记录（2026-09-18）
//
// 自带一套手写 Markdown 解析器（CommonMark 子集 + GFM 表格/任务列表/删除线），
// 因为根 go.mod 约定"零第三方依赖不变"，不能引入 goldmark / chroma。
//
// **双轨对账**：与另一套独立实现（Python markdown-it-py）逐篇做可见文本对照
// （各自渲染 → 去标签 → 还原实体 → 折叠空白，口径完全相同），
// 结果为 **167 篇文本全部一致（167/167）**。对账脚本与隔离环境为一次性取证，
// 放在 `.workbuddy/tools/oracle/`（已 gitignore，不属于活性工具链）。
// 对账过程中发现并修掉的真缺陷（均已固化为 markdown_test.go 用例）：
//
//	① 强调标签走文本通道被收尾转义 → `<strong>` 变成 `&lt;strong&gt;`；
//	② 高亮器普通字节未转义 → 代码里的 `<u32>`、`<class T>` 被浏览器当标签吞掉；
//	③ 强调侧翼判定只认 ASCII 标点 → 夹在全角标点（；：""等 \p{P}）之间的强调无法闭合；
//	④ 缺 CommonMark "rule of three" → 落单 `*` 错配，正文被吃掉或搬位；
//	⑤ 转义与代码跨度分两趟处理 → `\`` 被误当代码跨度闭合符；
//	⑥ 标签名允许数字开头 → 正文里的 `<1MB` 被当 HTML 标签透传；
//	⑦ 表格列数不校验 / 表格不能打断段落 / 有序列表打断段落未限制以 1 开头；
//	⑧ 锚点 slug 用码点区间判 CJK → 中文标点（、。「」）被错误保留；
//	⑨ 链接改写曾用正则扫整页属性 → 把代码块里的 `&lt;base href="/"&gt;` 也改掉。
//
// 已知残余差异（仅 1 处，已定性）：`archive/ROADMAP_C_SUBSET_EXTENSION.md` 有一张
// **表头 6 格、分隔行 7 格**的病态表，按 GFM 规定整张表不成立——本工具与参考实现
// 同样不认它为表（输出裸竖线），行为一致。
package main

import (
	"crypto/sha1"
	"embed"
	"encoding/hex"
	"encoding/json"
	"flag"
	"fmt"
	"io/fs"
	"os"
	"os/exec"
	"path"
	"path/filepath"
	"regexp"
	"runtime"
	"sort"
	"strconv"
	"strings"
	"time"
)

//go:embed assets/style.css assets/app.js
var assetsFS embed.FS

const (
	repoSlug = "jingwei108/vitro"
	repoURL  = "https://github.com/" + repoSlug
	// 渲染器版本：改动渲染逻辑后 +1（缓存整体失效）。
	// v2：行内强调/裸链接标签改走占位符池（此前被收尾转义成 &lt;strong&gt; 之类）；
	//     断链清单改为随缓存持久化（此前全缓存命中时报告恒为 0 处，属静默假绿）。
	// v3：高亮器普通字节改为转义写出（此前代码里的 <u32> / <class T> 被当标签吞掉，内容静默消失）。
	rendererVersion = 10
)

var (
	rootDir    string
	docsDir    string
	repoBranch = "HEAD"
)

// ---------------------------------------------------------------------------
// 文档与页面
// ---------------------------------------------------------------------------

type doc struct {
	rel     string // docs 相对路径（POSIX）
	out     string // 输出相对路径（POSIX）
	title   string
	mtime   time.Time
	size    int64
	lines   int
	html    string
	outline []outlineItem
	text    string
	source  string // 原始 markdown，仅用于缓存比对
	sha1    string
}

type outlineItem struct {
	Level int    `json:"l"`
	ID    string `json:"i"`
	Text  string `json:"t"`
}

type cacheEntry struct {
	Sha1    string        `json:"sha1"`
	Title   string        `json:"title"`
	HTML    string        `json:"html"`
	Outline []outlineItem `json:"outline"`
	Text    string        `json:"text"`
	// 该文档里指向 docs 之外目标的链接（随缓存持久化，保证缓存命中的构建也能如实报告）
	Missing []string `json:"missing,omitempty"`
}

type cacheFile struct {
	Version int                    `json:"renderer"`
	Entries map[string]*cacheEntry `json:"entries"`
}

func main() {
	out := flag.String("out", filepath.Join("tmp", "docs_preview"), "输出目录")
	doOpen := flag.Bool("open", false, "构建完成后用默认浏览器打开首页")
	watch := flag.Bool("watch", false, "常驻监听，docs/ 变动即重建")
	force := flag.Bool("force", false, "忽略缓存全量重建")
	quiet := flag.Bool("quiet", false, "只输出摘要")
	flag.Parse()

	self, err := os.Executable()
	if err != nil {
		die("无法确定工作目录: %v", err)
	}
	rootDir = findRepoRoot(filepath.Dir(self))
	docsDir = filepath.Join(rootDir, "docs")
	if info, err := os.Stat(docsDir); err != nil || !info.IsDir() {
		die("找不到文档目录：%s", docsDir)
	}
	repoBranch = detectBranch()

	outDir := *out
	if !filepath.IsAbs(outDir) {
		outDir = filepath.Join(rootDir, outDir)
	}

	if *watch {
		watchLoop(outDir, *force, *quiet)
		return
	}
	if err := build(outDir, *doOpen, *force, *quiet); err != nil {
		die("%v", err)
	}
}

func die(format string, args ...any) {
	fmt.Fprintf(os.Stderr, "[docs_preview] "+format+"\n", args...)
	os.Exit(1)
}

func findRepoRoot(start string) string {
	dir := start
	for i := 0; i < 8; i++ {
		if _, err := os.Stat(filepath.Join(dir, "go.mod")); err == nil {
			return dir
		}
		parent := filepath.Dir(dir)
		if parent == dir {
			break
		}
		dir = parent
	}
	cwd, err := os.Getwd()
	if err != nil {
		return start
	}
	return cwd
}

func detectBranch() string {
	cmd := exec.Command("git", "rev-parse", "--abbrev-ref", "HEAD")
	cmd.Dir = rootDir
	if out, err := cmd.Output(); err == nil {
		if s := strings.TrimSpace(string(out)); s != "" {
			return s
		}
	}
	return "HEAD"
}

// ---------------------------------------------------------------------------
// 收集文档
// ---------------------------------------------------------------------------

func collectDocs() ([]*doc, []string, error) {
	var docs []*doc
	err := filepath.WalkDir(docsDir, func(p string, d fs.DirEntry, err error) error {
		if err != nil {
			return err
		}
		if d.IsDir() || !strings.HasSuffix(strings.ToLower(d.Name()), ".md") {
			return nil
		}
		rel, err := filepath.Rel(docsDir, p)
		if err != nil {
			return err
		}
		rel = filepath.ToSlash(rel)
		info, err := d.Info()
		if err != nil {
			return err
		}
		data, err := os.ReadFile(p)
		if err != nil {
			return err
		}
		sum := sha1.Sum(data)
		outPath := strings.TrimSuffix(rel, filepath.Ext(rel)) + ".html"
		if rel == "README.md" {
			outPath = "index.html"
		}
		entry := &doc{
			rel:    rel,
			out:    outPath,
			size:   info.Size(),
			mtime:  info.ModTime(),
			source: string(data),
			sha1:   hex.EncodeToString(sum[:]),
			title:  strings.TrimSuffix(filepath.Base(rel), ".md"),
		}
		entry.lines = strings.Count(entry.source, "\n") + 1
		docs = append(docs, entry)
		return nil
	})
	if err != nil {
		return nil, nil, err
	}
	sort.Slice(docs, func(i, j int) bool { return docs[i].rel < docs[j].rel })

	var dirs []string
	dirs = append(dirs, ".")
	err = filepath.WalkDir(docsDir, func(p string, d fs.DirEntry, err error) error {
		if err != nil || !d.IsDir() || p == docsDir {
			return err
		}
		rel, err := filepath.Rel(docsDir, p)
		if err != nil {
			return err
		}
		dirs = append(dirs, filepath.ToSlash(rel))
		return nil
	})
	if err != nil {
		return nil, nil, err
	}
	sort.Slice(dirs, func(i, j int) bool { return naturalLess(dirs[i], dirs[j]) })
	return docs, dirs, nil
}

// ---------------------------------------------------------------------------
// 链接改写
//
// 口径：只在**行内解析期**按 token 改写 markdown 链接的目标（见 inlineCtx.mapHref）。
// 早期版本曾在渲染完成后用正则扫整页 href/src 属性——那会把代码块里的
// `&lt;base href="/"&gt;`、正文里写的 `href="..."` 也一起改掉（内容被篡改），
// 现已废弃该做法。
// ---------------------------------------------------------------------------

var schemeRe = regexp.MustCompile(`^[a-zA-Z][a-zA-Z0-9+.\-]*:`)

type rewriter struct {
	docMap  map[string]string
	dirMap  map[string]string
	missing [][2]string
}

// mapHref 返回改写后的链接目标。
func (r *rewriter) mapHref(value, docRel string) string {
	docDir := path.Dir(docRel)
	if docDir == "." {
		docDir = ""
	}
	value = strings.TrimSpace(value)
	if value == "" || strings.HasPrefix(value, "#") {
		return value
	}
	if strings.HasPrefix(value, "//") || schemeRe.MatchString(value) {
		return value
	}
	if strings.HasPrefix(value, "/") {
		rel := strings.TrimPrefix(value, "/")
		return repoURLFor(rel, isDir(filepath.Join(rootDir, filepath.FromSlash(rel))))
	}

	pathPart := value
	if i := strings.IndexByte(pathPart, '#'); i >= 0 {
		pathPart = pathPart[:i]
	}
	if i := strings.IndexByte(pathPart, '?'); i >= 0 {
		pathPart = pathPart[:i]
	}
	frag := value[len(pathPart):]
	if pathPart == "" {
		return value
	}

	decoded, err := urlUnescape(pathPart)
	if err != nil {
		decoded = pathPart
	}
	target := path.Clean(path.Join(docDir, decoded))

	if strings.HasPrefix(target, "..") {
		repoRel := path.Clean(path.Join("docs", target))
		return repoURLFor(repoRel, isDir(filepath.Join(rootDir, filepath.FromSlash(repoRel))))
	}

	abs := filepath.Join(docsDir, filepath.FromSlash(target))
	if info, err := os.Stat(abs); err == nil && info.IsDir() {
		if page, ok := r.dirMap[target]; ok {
			return relHref(path.Dir(page), page) + frag
		}
		return repoURLFor(path.Join("docs", target), true)
	}

	if strings.HasSuffix(strings.ToLower(decoded), ".md") {
		if page, ok := r.docMap[target]; ok {
			pd := path.Dir(page)
			if pd == "." {
				pd = ""
			}
			return relHref(pd, page) + frag
		}
		r.missing = append(r.missing, [2]string{docRel, value})
		return repoURLFor(path.Join("docs", target), false)
	}
	if _, err := os.Stat(abs); err == nil {
		r.missing = append(r.missing, [2]string{docRel, value})
	}
	return repoURLFor(path.Join("docs", target), false)
}

func repoURLFor(repoRel string, isDir bool) string {
	verb := "blob"
	if isDir {
		verb = "tree"
	}
	return repoURL + "/" + verb + "/" + repoBranch + "/" + encodePath(repoRel)
}

func isDir(p string) bool {
	info, err := os.Stat(p)
	return err == nil && info.IsDir()
}

// encodePath 逐段百分号编码（保留 / 与 ..）。
func encodePath(p string) string {
	segs := strings.Split(filepath.ToSlash(p), "/")
	for i, s := range segs {
		segs[i] = encodeSegment(s)
	}
	return strings.Join(segs, "/")
}

// relHref 输出根内相对路径。
func relHref(fromDir, target string) string {
	rel := pathRel(fromDir, target)
	return encodePath(rel)
}

func pathRel(fromDir, target string) string {
	if fromDir == "" || fromDir == "." {
		return target
	}
	from := strings.Split(fromDir, "/")
	to := strings.Split(target, "/")
	i := 0
	for i < len(from) && i < len(to)-1 && from[i] == to[i] {
		i++
	}
	var parts []string
	for k := i; k < len(from); k++ {
		parts = append(parts, "..")
	}
	parts = append(parts, to[i:]...)
	return strings.Join(parts, "/")
}

func encodeSegment(s string) string {
	var b strings.Builder
	for i := 0; i < len(s); i++ {
		c := s[i]
		if c >= 'a' && c <= 'z' || c >= 'A' && c <= 'Z' || c >= '0' && c <= '9' ||
			c == '-' || c == '_' || c == '.' || c == '~' {
			b.WriteByte(c)
			continue
		}
		fmt.Fprintf(&b, "%%%02X", c)
	}
	return b.String()
}

func urlUnescape(s string) (string, error) {
	if !strings.Contains(s, "%") {
		return s, nil
	}
	var b strings.Builder
	for i := 0; i < len(s); i++ {
		if s[i] == '%' && i+2 < len(s) {
			v, err := strconv.ParseUint(s[i+1:i+3], 16, 8)
			if err == nil {
				b.WriteByte(byte(v))
				i += 2
				continue
			}
		}
		b.WriteByte(s[i])
	}
	return b.String(), nil
}

// ---------------------------------------------------------------------------
// 构建
// ---------------------------------------------------------------------------

func build(outDir string, doOpen, force, quiet bool) error {
	start := time.Now()
	docs, dirs, err := collectDocs()
	if err != nil {
		return err
	}
	if len(docs) == 0 {
		return fmt.Errorf("docs/ 下没有 Markdown 文件")
	}

	docMap := map[string]string{}
	for _, d := range docs {
		docMap[d.rel] = d.out
	}
	dirMap := map[string]string{}
	for _, rel := range dirs {
		if rel == "." {
			dirMap[rel] = "index.html"
			continue
		}
		dirMap[rel] = rel + "/index.html"
	}

	cachePath := filepath.Join(outDir, ".cache.json")
	entries := map[string]*cacheEntry{}
	if !force {
		if data, err := os.ReadFile(cachePath); err == nil {
			var cf cacheFile
			if json.Unmarshal(data, &cf) == nil && cf.Version == rendererVersion && cf.Entries != nil {
				entries = cf.Entries
			}
		}
	}

	rw := &rewriter{docMap: docMap, dirMap: dirMap}
	rebuilt := 0

	for _, d := range docs {
		if hit, ok := entries[d.rel]; ok && hit.Sha1 == d.sha1 && !force {
			d.title, d.html, d.outline, d.text = hit.Title, hit.HTML, hit.Outline, hit.Text
			continue
		}
		rw.missing = nil
		ctx := &inlineCtx{rw: rw, rel: d.rel}
		body := renderMarkdown(ctx, d.source)
		d.html = body
		d.outline = extractOutline(body, 3)
		d.text = htmlToText(body)
		if t := firstHeadingText(body); t != "" {
			d.title = t
		}
		missing := make([]string, 0, len(rw.missing))
		for _, p := range rw.missing {
			missing = append(missing, p[1])
		}
		entries[d.rel] = &cacheEntry{Sha1: d.sha1, Title: d.title, HTML: d.html,
			Outline: d.outline, Text: d.text, Missing: missing}
		rebuilt++
	}

	// 落盘：文档页 + 目录页
	seen := map[string]bool{}
	for _, d := range docs {
		pageDir := path.Dir(d.out)
		if pageDir == "." {
			pageDir = ""
		}
		page := renderPage(pageTemplateData{
			title:   d.title,
			source:  "docs/" + d.rel,
			assets:  strings.Repeat("../", depthOf(pageDir)) + "assets/",
			nav:     renderNav(docs, dirs, dirMap, pageDir, d.rel),
			chips:   chipsFor(d.rel),
			mtime:   d.mtime.Format("2006-01-02 15:04"),
			lines:   strconv.Itoa(d.lines),
			size:    humanSize(d.size),
			body:    d.html,
			outline: renderOutline(d.outline),
			ghURL:   repoURL + "/blob/" + repoBranch + "/" + encodePath("docs/"+d.rel),
			crumbs:  "docs/" + d.rel,
		})
		if err := writeFile(filepath.Join(outDir, filepath.FromSlash(d.out)), page); err != nil {
			return err
		}
		seen[d.out] = true
	}

	for _, rel := range dirs {
		var children []*doc
		for _, d := range docs {
			if path.Dir(d.rel) == rel && path.Base(d.rel) != "README.md" {
				children = append(children, d)
			}
		}
		var subdirs []string
		for _, dd := range dirs {
			if dd == "." {
				continue
			}
			if (rel == "." && !strings.Contains(dd, "/")) || path.Dir(dd) == rel {
				subdirs = append(subdirs, dd)
			}
		}
		pageOut := dirMap[rel]
		if rel == "." && docMap["README.md"] != "" && docMap["README.md"] == pageOut {
			continue // 根目录已有 README 作为入口页
		}
		body := renderDirPage(children, subdirs, rel)
		pageDir := path.Dir(pageOut)
		if pageDir == "." {
			pageDir = ""
		}
		title := dirLabel(rel) + "/ 目录"
		page := renderPage(pageTemplateData{
			title:   title,
			source:  "docs/" + dirLabel(rel),
			assets:  strings.Repeat("../", depthOf(pageDir)) + "assets/",
			nav:     renderNav(docs, dirs, dirMap, pageDir, ""),
			chips:   `<span class="chip chip-root">目录清单</span>`,
			mtime:   "—",
			lines:   "—",
			size:    "—",
			body:    body,
			outline: renderOutline(extractOutline(body, 3)),
			ghURL:   repoURL + "/tree/" + repoBranch + "/docs" + dashPath(rel),
			crumbs:  "docs/" + dashPath(rel),
		})
		if err := writeFile(filepath.Join(outDir, filepath.FromSlash(pageOut)), page); err != nil {
			return err
		}
		seen[pageOut] = true
	}

	// 资源（embed → 释放）
	assetsOut := filepath.Join(outDir, "assets")
	if err := os.MkdirAll(assetsOut, 0o755); err != nil {
		return err
	}
	for _, name := range []string{"style.css", "app.js"} {
		data, err := assetsFS.ReadFile("assets/" + name)
		if err != nil {
			return err
		}
		if err := os.WriteFile(filepath.Join(assetsOut, name), data, 0o644); err != nil {
			return err
		}
	}
	if err := os.WriteFile(filepath.Join(assetsOut, "search-index.js"), []byte(buildSearchIndex(docs)), 0o644); err != nil {
		return err
	}

	// 清理上一轮遗留页（只删本工具登记过的产物，绝不动其它文件）
	removed := cleanupStale(outDir, seen)

	if err := writeJSON(cachePath, cacheFile{Version: rendererVersion, Entries: entries}); err != nil {
		return err
	}

	var broken [][2]string
	for _, d := range docs {
		if e, ok := entries[d.rel]; ok {
			for _, tgt := range e.Missing {
				broken = append(broken, [2]string{d.rel, tgt})
			}
		}
	}
	broken = dedupePairs(broken)
	report := []string{
		"Vitro docs 本地预览构建报告",
		"生成时间：" + time.Now().Format("2006-01-02 15:04:05"),
		"输出目录：" + outDir,
		fmt.Sprintf("文档总数：%d（本次重渲染 %d，缓存命中 %d）", len(docs), rebuilt, len(docs)-rebuilt),
		fmt.Sprintf("目录页：%d，清理陈旧页：%d", len(dirs), removed),
		fmt.Sprintf("指向仓库内 docs 之外目标的链接（按 GitHub 地址输出）：%d", len(broken)),
	}
	if len(broken) > 0 {
		report = append(report, "", "链接明细（源文档 → 目标；docs 外的源码/脚本文件只能指向 GitHub）：")
		for _, p := range broken {
			report = append(report, "  "+p[0]+" → "+p[1])
		}
	}
	reportText := strings.Join(report, "\n") + "\n"
	if err := writeFile(filepath.Join(outDir, "_report.txt"), reportText); err != nil {
		return err
	}

	if !quiet {
		for _, line := range report[:6] {
			fmt.Println("[docs_preview] " + line)
		}
	}
	fmt.Printf("[docs_preview] 完成，耗时 %.2fs → 双击打开：%s\n",
		time.Since(start).Seconds(), filepath.Join(outDir, "index.html"))

	if doOpen {
		openInBrowser(filepath.Join(outDir, "index.html"))
	}
	return nil
}

func dashPath(rel string) string {
	if rel == "." || rel == "" {
		return ""
	}
	return "/" + rel
}

func depthOf(dir string) int {
	if dir == "" || dir == "." {
		return 0
	}
	return strings.Count(dir, "/") + 1
}

func writeFile(p, content string) error {
	if err := os.MkdirAll(filepath.Dir(p), 0o755); err != nil {
		return err
	}
	return os.WriteFile(p, []byte(content), 0o644)
}

func writeJSON(p string, v any) error {
	data, err := json.Marshal(v)
	if err != nil {
		return err
	}
	return writeFile(p, string(data))
}

// cleanupStale 依据上一轮 manifest 删除本轮不再产出的页面。
func cleanupStale(outDir string, seen map[string]bool) int {
	manifestPath := filepath.Join(outDir, ".manifest.json")
	var prev struct{ Files []string }
	removed := 0
	if data, err := os.ReadFile(manifestPath); err == nil {
		if json.Unmarshal(data, &prev) == nil {
			for _, f := range prev.Files {
				if seen[f] {
					continue
				}
				full := filepath.Join(outDir, filepath.FromSlash(f))
				if info, err := os.Stat(full); err == nil && !info.IsDir() {
					if os.Remove(full) == nil {
						removed++
					}
				}
			}
		}
	}
	files := make([]string, 0, len(seen))
	for f := range seen {
		files = append(files, f)
	}
	sort.Strings(files)
	_ = writeJSON(manifestPath, struct {
		Files []string `json:"files"`
	}{Files: files})
	return removed
}

func buildSearchIndex(docs []*doc) string {
	type head struct {
		I string `json:"i"`
		T string `json:"t"`
	}
	type entry struct {
		P string `json:"p"`
		T string `json:"t"`
		S string `json:"s"`
		H []head `json:"h"`
		X string `json:"x"`
	}
	list := make([]entry, 0, len(docs))
	for _, d := range docs {
		heads := make([]head, 0, len(d.outline))
		for _, h := range d.outline {
			heads = append(heads, head{I: h.ID, T: h.Text})
		}
		list = append(list, entry{P: d.out, T: d.title, S: "docs/" + d.rel, H: heads, X: d.text})
	}
	data, _ := json.Marshal(list)
	s := strings.ReplaceAll(string(data), "</", `<\/`)
	return "window.__VITRO_DOCS_INDEX = " + s + ";\n"
}

func dedupePairs(pairs [][2]string) [][2]string {
	seen := map[string]bool{}
	var out [][2]string
	for _, p := range pairs {
		k := p[0] + "\x00" + p[1]
		if seen[k] {
			continue
		}
		seen[k] = true
		out = append(out, p)
	}
	sort.Slice(out, func(i, j int) bool {
		if out[i][0] != out[j][0] {
			return out[i][0] < out[j][0]
		}
		return out[i][1] < out[j][1]
	})
	return out
}

func openInBrowser(p string) {
	abs, err := filepath.Abs(p)
	if err != nil {
		abs = p
	}
	var cmd *exec.Cmd
	switch runtime.GOOS {
	case "windows":
		cmd = exec.Command("rundll32", "url.dll,FileProtocolHandler", abs)
	case "darwin":
		cmd = exec.Command("open", abs)
	default:
		cmd = exec.Command("xdg-open", abs)
	}
	if err := cmd.Start(); err != nil {
		fmt.Fprintf(os.Stderr, "[docs_preview] 自动打开失败（%v），请手动双击 %s\n", err, abs)
	}
}

// ---------------------------------------------------------------------------
// 监听
// ---------------------------------------------------------------------------

func watchLoop(outDir string, force, quiet bool) {
	fmt.Printf("[docs_preview] 监听 %s（Ctrl+C 退出）：新增 / 修改 / 删除 md 自动重建。\n", docsDir)
	if err := build(outDir, false, force, quiet); err != nil {
		fmt.Fprintln(os.Stderr, "[docs_preview] "+err.Error())
	}
	prev := snapshot()
	for {
		time.Sleep(1500 * time.Millisecond)
		cur := snapshot()
		if snapEqual(prev, cur) {
			continue
		}
		added, changed, gone := diffSnapshot(prev, cur)
		fmt.Printf("[docs_preview] 变动：新增 %d / 修改 %d / 删除 %d\n", len(added), len(changed), len(gone))
		if err := build(outDir, false, false, quiet); err != nil {
			fmt.Fprintln(os.Stderr, "[docs_preview] "+err.Error())
		}
		prev = cur
	}
}

func snapshot() map[string]int64 {
	snap := map[string]int64{}
	_ = filepath.WalkDir(docsDir, func(p string, d fs.DirEntry, err error) error {
		if err != nil || d.IsDir() || !strings.HasSuffix(strings.ToLower(d.Name()), ".md") {
			return nil
		}
		if info, err := d.Info(); err == nil {
			snap[p] = info.ModTime().UnixNano()
		}
		return nil
	})
	return snap
}

func snapEqual(a, b map[string]int64) bool {
	if len(a) != len(b) {
		return false
	}
	for k, v := range a {
		if b[k] != v {
			return false
		}
	}
	return true
}

func diffSnapshot(a, b map[string]int64) (added, changed, gone []string) {
	for k, v := range b {
		if old, ok := a[k]; !ok {
			added = append(added, k)
		} else if old != v {
			changed = append(changed, k)
		}
	}
	for k := range a {
		if _, ok := b[k]; !ok {
			gone = append(gone, k)
		}
	}
	return
}

// ---------------------------------------------------------------------------
// 文本工具
// ---------------------------------------------------------------------------

// naturalLess 让 01- / 02- 这类数字前缀按数值排序，其余按码点序。
func naturalLess(a, b string) bool {
	pa, pb := splitNumeric(a), splitNumeric(b)
	for i := 0; i < len(pa) && i < len(pb); i++ {
		if pa[i] == pb[i] {
			continue
		}
		na, ea := strconv.Atoi(pa[i])
		nb, eb := strconv.Atoi(pb[i])
		if ea == nil && eb == nil {
			return na < nb
		}
		return pa[i] < pb[i]
	}
	return len(pa) < len(pb)
}

func splitNumeric(s string) []string {
	var parts []string
	var cur strings.Builder
	digit := false
	for i := 0; i < len(s); i++ {
		isDig := s[i] >= '0' && s[i] <= '9'
		if cur.Len() > 0 && isDig != digit {
			parts = append(parts, cur.String())
			cur.Reset()
		}
		digit = isDig
		cur.WriteByte(s[i])
	}
	if cur.Len() > 0 {
		parts = append(parts, cur.String())
	}
	return parts
}

func humanSize(n int64) string {
	switch {
	case n < 1024:
		return fmt.Sprintf("%d B", n)
	case n < 1024*1024:
		return fmt.Sprintf("%.1f KB", float64(n)/1024)
	default:
		return fmt.Sprintf("%.2f MB", float64(n)/1024/1024)
	}
}

func unescapeEntities(s string) string {
	repl := strings.NewReplacer("&amp;", "&", "&lt;", "<", "&gt;", ">", "&quot;", `"`, "&#39;", "'")
	return repl.Replace(s)
}
