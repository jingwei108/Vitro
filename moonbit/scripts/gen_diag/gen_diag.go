// gen_diag 从 Rust oracle 源生成 MoonBit vitro/engine/diag 包的机器单源部分。
//
// 纪律（第一阶段计划 T2）：
//   - ErrorCode 枚举禁手抄——本脚本是唯一产出口；
//   - fail loud：任何解析失配 / 白名单不对账 / 卡片数异常立即 exit 1，
//     禁静默 default；
//   - 幂等：同源重复运行产物字节一致（无时间戳；源文件 sha256 前 8 位
//     随产物落款——源变则产物变，配合 -check 构成漂移防线）；
//   - -check：产物与现存文件逐字节比对，不一致 exit 1（CI 幂等锚）。
//
// 用法：
//
//	cd moonbit && go run ./scripts/gen_diag          # 生成 / 覆盖产物
//	cd moonbit && go run ./scripts/gen_diag -check   # 校验产物未漂移
//
// 输入（Rust 冻结区，只读）：
//   - ../native/crates/vitro_shared/src/error_codes.rs
//   - ../native/src/diagnostics/error_catalog/{lexer,parser,semantic,cpp}.rs
//
// 输出（MoonBit 活跃区）：
//   - diag/error_code_gen.mbt   （enum + code/severity/lang/name/display/all/from_code + 卡片码清单）
//   - diag/catalog_gen.mbt      （77 卡片 + catalog_of 穷尽 137 臂无兜底臂）
package main

import (
	"bytes"
	"crypto/sha256"
	"flag"
	"fmt"
	"os"
	"os/exec"
	"path/filepath"
	"sort"
	"strconv"
	"strings"
)

const (
	expectedArms    = 137 // 计划 T2 基线：137 臂（2026-09-19 对源实测）
	expectedCatalog = 77  // 计划 T2 基线：catalog 77 条
)

type arm struct {
	name string // Rust 变体名（如 E1001_UnknownChar）
	code int
}

type entry struct {
	code         int
	emoji        string
	title        string
	explanation  string
	commonCauses []string
}

// armNames 供 catalog 渲染按码取变体名（main 解析后填充）。
var armNames = map[int]string{}

func fatalf(format string, args ...any) {
	fmt.Fprintf(os.Stderr, "gen_diag: FAIL: "+format+"\n", args...)
	os.Exit(1)
}

func main() {
	check := flag.Bool("check", false, "只校验产物未漂移，不写入")
	codesPath := flag.String("codes", "../native/crates/vitro_shared/src/error_codes.rs", "error_codes.rs 路径")
	catalogDir := flag.String("catalog", "../native/src/diagnostics/error_catalog", "catalog 目录")
	outDir := flag.String("out", "diag", "输出目录")
	flag.Parse()

	src, err := os.ReadFile(*codesPath)
	if err != nil {
		fatalf("读源失败 %s: %v", *codesPath, err)
	}
	// 行尾规范化后取 sha：工作区 LF/CRLF 形态（git autocrlf、checkout 差异）
	// 不得影响产物落款——否则换行尾即 -check 假红。
	srcHash := fmt.Sprintf("%x", sha256.Sum256(bytes.ReplaceAll(src, []byte("\r\n"), []byte("\n"))))[:8]

	arms := parseArms(string(src))
	if len(arms) != expectedArms {
		fatalf("枚举臂数 %d ≠ 基线 %d——源已变更，请人工核对后更新 expectedArms 并登记差异", len(arms), expectedArms)
	}
	for _, a := range arms {
		armNames[a.code] = a.name
	}
	wCodes, hCodes := parseWhitelists(string(src))
	crossCheckWhitelists(arms, wCodes, hCodes)

	entries := parseCatalog(*catalogDir)
	if len(entries) != expectedCatalog {
		fatalf("卡片数 %d ≠ 基线 %d——源已变更，请人工核对后更新 expectedCatalog 并登记差异", len(entries), expectedCatalog)
	}
	crossCheckCatalog(entries)

	genCode := renderErrorCode(arms, wCodes, hCodes, entries, srcHash)
	genCatalog := renderCatalog(entries, srcHash)

	writeProducts(*outDir, map[string]string{
		"error_code_gen.mbt": genCode,
		"catalog_gen.mbt":    genCatalog,
	}, *check)

	if *check {
		fmt.Println("gen_diag: check OK（产物与源一致，2 文件未漂移）")
	} else {
		fmt.Println("gen_diag: 生成完成（2 文件）")
	}
}

// ---- 解析：error_codes.rs ----

func parseArms(src string) []arm {
	start := strings.Index(src, "pub enum ErrorCode {")
	if start < 0 {
		fatalf("未找到 pub enum ErrorCode 块")
	}
	end := strings.Index(src[start:], "\n}")
	if end < 0 {
		fatalf("enum 块未闭合")
	}
	body := src[start : start+end]
	var arms []arm
	seen := map[int]string{}
	for _, line := range strings.Split(body, "\n") {
		t := strings.TrimSpace(line)
		if t == "" || strings.HasPrefix(t, "//") {
			continue
		}
		eq := strings.Index(t, " = ")
		if eq < 0 || !strings.HasSuffix(t, ",") {
			continue
		}
		name := t[:eq]
		if !validVariantName(name) {
			continue
		}
		numStr := strings.TrimSuffix(t[eq+3:], ",")
		code, err := strconv.Atoi(numStr)
		if err != nil {
			fatalf("臂 %q 编号解析失败: %v", name, err)
		}
		if prev, dup := seen[code]; dup {
			fatalf("编号 %d 重复：%s 与 %s", code, prev, name)
		}
		seen[code] = name
		arms = append(arms, arm{name: name, code: code})
	}
	return arms
}

// validVariantName：E/W/H 前缀 + 4 位编号 + '_' + 名（如 E1001_UnknownChar），或 Unknown。
func validVariantName(name string) bool {
	if name == "Unknown" {
		return true
	}
	if len(name) < 7 || name[5] != '_' {
		return false
	}
	switch name[0] {
	case 'E', 'W', 'H':
	default:
		return false
	}
	for i := 1; i <= 4; i++ {
		if name[i] < '0' || name[i] > '9' {
			return false
		}
	}
	return name[6] != '_'
}

func parseWhitelists(src string) (w, h []int) {
	w, h = extractList(src, "WARN_CODES"), extractList(src, "HINT_CODES")
	if len(w) == 0 || len(h) == 0 {
		fatalf("WARN_CODES/HINT_CODES 白名单解析为空——源形态变更？")
	}
	return w, h
}

func extractList(src, name string) []int {
	marker := name + ": &[i32] = &["
	i := strings.Index(src, marker)
	if i < 0 {
		fatalf("未找到 %s 声明", name)
	}
	rest := src[i+len(marker):]
	end := strings.Index(rest, "]")
	if end < 0 {
		fatalf("%s 未闭合", name)
	}
	var out []int
	for _, part := range strings.Split(rest[:end], ",") {
		part = strings.TrimSpace(part)
		if part == "" {
			continue
		}
		v, err := strconv.Atoi(part)
		if err != nil {
			fatalf("%s 成员 %q 解析失败: %v", name, part, err)
		}
		out = append(out, v)
	}
	return out
}

// crossCheckWhitelists 复刻 Rust 侧 test_whitelist_matches_source_variants 语义：
// 白名单与变体名前缀双向一致——新增 W/H 码漏登记、或白名单腐化，立即红。
func crossCheckWhitelists(arms []arm, w, h []int) {
	var wVar, hVar []int
	for _, a := range arms {
		if a.name == "Unknown" {
			continue
		}
		switch a.name[0] {
		case 'W':
			wVar = append(wVar, a.code)
		case 'H':
			hVar = append(hVar, a.code)
		}
	}
	if !equalInts(w, wVar) {
		fatalf("W 白名单 %v 与 W 变体集 %v 不一致——漏登记的 W 码会静默显示 E 前缀（P3 伪造复发）", w, wVar)
	}
	if !equalInts(h, hVar) {
		fatalf("H 白名单 %v 与 H 变体集 %v 不一致", h, hVar)
	}
}

func equalInts(a, b []int) bool {
	sa, sb := append([]int{}, a...), append([]int{}, b...)
	sort.Ints(sa)
	sort.Ints(sb)
	if len(sa) != len(sb) {
		return false
	}
	for i := range sa {
		if sa[i] != sb[i] {
			return false
		}
	}
	return true
}

// ---- 解析：error_catalog/*.rs ----

func parseCatalog(dir string) []entry {
	files := []string{"lexer.rs", "parser.rs", "semantic.rs", "cpp.rs"}
	var entries []entry
	seen := map[int]bool{}
	for _, f := range files {
		path := filepath.Join(dir, f)
		src, err := os.ReadFile(path)
		if err != nil {
			fatalf("读卡片源失败 %s: %v", path, err)
		}
		for _, e := range parseEntryBlocks(string(src), f) {
			if seen[e.code] {
				fatalf("卡片码 %d 重复（%s）", e.code, f)
			}
			seen[e.code] = true
			entries = append(entries, e)
		}
	}
	sort.Slice(entries, func(i, j int) bool { return entries[i].code < entries[j].code })
	return entries
}

// parseEntryBlocks 逐块提取 (code, ErrorInfo { ... })——大括号配对扫描
// （字符串字面量感知，块尾形态有 "}),", "},\\n)", "})\\n]" 三种，无法单点 Index）。
func parseEntryBlocks(src, from string) []entry {
	var out []entry
	for {
		i := strings.Index(src, "ErrorInfo {")
		if i < 0 {
			break
		}
		depth := 0
		end := -1
		for j := i + len("ErrorInfo "); j < len(src); j++ {
			c := src[j]
			if c == '"' {
				_, next := scanRustString(src, j)
				j = next - 1
				continue
			}
			if c == '{' {
				depth++
			} else if c == '}' {
				depth--
				if depth == 0 {
					end = j
					break
				}
			}
		}
		if end < 0 {
			fatalf("%s: ErrorInfo 块大括号未配对", from)
		}
		block := src[i : end+1]
		src = src[end+1:]
		out = append(out, entry{
			code:         mustIntField(block, "code", from),
			emoji:        mustStrField(block, "emoji", from),
			title:        mustStrField(block, "title", from),
			explanation:  mustStrField(block, "explanation", from),
			commonCauses: mustStrArrayField(block, "common_causes", from),
		})
	}
	return out
}

func mustIntField(block, field, from string) int {
	i := strings.Index(block, field+":")
	if i < 0 {
		fatalf("%s: 块内缺字段 %s", from, field)
	}
	rest := strings.TrimSpace(block[i+len(field)+1:])
	end := strings.IndexAny(rest, ",}")
	if end < 0 {
		fatalf("%s: 字段 %s 无结束符", from, field)
	}
	v, err := strconv.Atoi(strings.TrimSpace(rest[:end]))
	if err != nil {
		fatalf("%s: 字段 %s 数值解析失败: %v", from, field, err)
	}
	return v
}

// scanRustString 从 s[pos]（须指向开引号）扫描一个 Rust 字符串字面量，
// 返回解码值与收尾下一位。
func scanRustString(s string, pos int) (string, int) {
	if pos >= len(s) || s[pos] != '"' {
		fatalf("扫描起点不是引号: %q", safeHead(s, pos))
	}
	var b strings.Builder
	for i := pos + 1; i < len(s); i++ {
		c := s[i]
		if c == '\\' && i+1 < len(s) {
			switch s[i+1] {
			case 'n':
				b.WriteByte('\n')
				i++ // 与 post 的 i++ 合计跳过转义对的 2 字节
			case 't':
				b.WriteByte('\t')
				i++
			case 'r':
				b.WriteByte('\r')
				i++
			case '"', '\\':
				b.WriteByte(s[i+1])
				i++
			case '\n':
				// Rust 续行转义：跳过换行与后续空白（semantic.rs 实存）。
				// 只推进到 j-1，post 的 i++ 恰落在续行后首字节——此前
				// 此 case 后又有统一 i++，双跳吞掉首字节（"向"丢 E5 实证，
				// E4 对拍 77 条唯一差异即此）。
				j := i + 2
				for j < len(s) && (s[j] == ' ' || s[j] == '	' || s[j] == '\r') {
					j++
				}
				i = j - 1
			default:
				fatalf("未处理的 Rust 转义 0x%02x @ %q", s[i+1], safeHead(s, pos))
			}
			continue
		}
		if c == '"' {
			return b.String(), i + 1
		}
		b.WriteByte(c)
	}
	fatalf("字符串未闭合")
	return "", 0
}

func safeHead(s string, pos int) string {
	head := s[pos:]
	if len(head) > 16 {
		head = head[:16]
	}
	return head
}

func mustStrField(block, field, from string) string {
	i := strings.Index(block, field+":")
	if i < 0 {
		fatalf("%s: 块内缺字段 %s", from, field)
	}
	pos := strings.IndexByte(block[i:], '"')
	if pos < 0 {
		fatalf("%s: 字段 %s 无字符串字面量", from, field)
	}
	v, _ := scanRustString(block, i+pos)
	return v
}

func mustStrArrayField(block, field, from string) []string {
	i := strings.Index(block, field+": &[")
	if i < 0 {
		fatalf("%s: 块内缺字段 %s", from, field)
	}
	rest := block[i+len(field)+3:]
	var out []string
	for {
		pos := strings.IndexByte(rest, '"')
		if pos < 0 {
			return out
		}
		v, next := scanRustString(rest, pos)
		out = append(out, v)
		rest = rest[next:]
		t := strings.TrimLeft(rest, " \t\n\r")
		if strings.HasPrefix(t, "]") {
			return out
		}
	}
}

func crossCheckCatalog(entries []entry) {
	for _, e := range entries {
		if _, ok := armNames[e.code]; !ok {
			fatalf("卡片码 %d 不在 ErrorCode 枚举内——卡片源与枚举源失配", e.code)
		}
	}
}

// ---- 渲染 ----

// escapeMbt 按 MoonBit 字符串字面量规则转义。
func escapeMbt(s string) string {
	var b strings.Builder
	for _, r := range s {
		switch r {
		case '\\':
			b.WriteString(`\\`)
		case '"':
			b.WriteString(`\"`)
		case '\n':
			b.WriteString(`\n`)
		case '\t':
			b.WriteString(`\t`)
		case '\r':
			b.WriteString(`\r`)
		default:
			b.WriteRune(r)
		}
	}
	return b.String()
}

func severityOf(a arm, wSet, hSet map[int]bool) string {
	switch {
	case wSet[a.code]:
		return "Warning"
	case hSet[a.code]:
		return "Hint"
	default:
		return "Error"
	}
}

func langOf(a arm) string {
	if a.code >= 4000 {
		return "Cpp"
	}
	return "C"
}

func prefixOf(a arm, wSet, hSet map[int]bool) string {
	switch {
	case wSet[a.code]:
		return "W"
	case hSet[a.code]:
		return "H"
	default:
		return "E"
	}
}

func renderErrorCode(arms []arm, w, h []int, entries []entry, srcHash string) string {
	wSet, hSet := map[int]bool{}, map[int]bool{}
	for _, c := range w {
		wSet[c] = true
	}
	for _, c := range h {
		hSet[c] = true
	}
	entrySet := map[int]bool{}
	for _, e := range entries {
		entrySet[e.code] = true
	}

	sorted := append([]arm{}, arms...)
	sort.Slice(sorted, func(i, j int) bool { return sorted[i].code < sorted[j].code })

	var b strings.Builder
	fmt.Fprintf(&b, "// GENERATED by moonbit/scripts/gen_diag —— 禁手改（再生成：cd moonbit && go run ./scripts/gen_diag）。\n"+
		"// 源：native/crates/vitro_shared/src/error_codes.rs（sha256/%s）\n"+
		"// 基线：137 臂 / catalog 77 条（漂移时本脚本 fail loud）。\n\n"+
		"///|\n"+
		"/// 诊断错误码（137 臂，生成自 Rust oracle——禁手抄，T2 纪律）。\n"+
		"/// 段位：1xxx 词法 / 2xxx 语法 / 3xxx 语义 / 4xxx C++ 专属。\n"+
		"/// severity 由 W/H 白名单（与变体前缀双向对账）决定；lang 按 4xxx 段位。\n"+
		"pub(all) enum ErrorCode {\n", srcHash)
	for _, a := range sorted {
		fmt.Fprintf(&b, "  %s\n", a.name)
	}
	b.WriteString("} derive(Debug, Eq, Hash, Compare)\n")

	b.WriteString("\n///|\n" +
		"/// 编号（穷尽 match，无下划线兜底——增删臂即编译红）。\n" +
		"pub fn ErrorCode::code(self : ErrorCode) -> Int {\n  match self {\n")
	for _, a := range sorted {
		fmt.Fprintf(&b, "    %s => %d\n", a.name, a.code)
	}
	b.WriteString("  }\n}\n")

	b.WriteString("\n///|\n" +
		"/// 严重级（穷尽；W 白名单 11 码 / H 白名单 1 码，生成时与变体前缀双向对账——\n" +
		"/// P3 语义单源：W/H 数值区间与 E 交织，不可按数值段判定）。\n" +
		"pub fn ErrorCode::severity(self : ErrorCode) -> Severity {\n  match self {\n")
	for _, a := range sorted {
		fmt.Fprintf(&b, "    %s => %s\n", a.name, severityOf(a, wSet, hSet))
	}
	b.WriteString("  }\n}\n")

	b.WriteString("\n///|\n" +
		"/// 语言域（穷尽）：Cpp = 仅 C++ 形态触发（4xxx 段）；C = C 共通域\n" +
		"/// （C++ 中的 C 子集同样触发）。\n" +
		"pub fn ErrorCode::lang(self : ErrorCode) -> SourceLang {\n  match self {\n")
	for _, a := range sorted {
		fmt.Fprintf(&b, "    %s => %s\n", a.name, langOf(a))
	}
	b.WriteString("  }\n}\n")

	b.WriteString("\n///|\n" +
		"/// 变体名（与 Rust 逐字一致——稳定标识，诊断序列差分锚的 key）。\n" +
		"pub fn ErrorCode::name(self : ErrorCode) -> String {\n  match self {\n")
	for _, a := range sorted {
		line := fmt.Sprintf("    %s => \"%s\"", a.name, a.name)
		if len(line) > 80 {
			// 与 moon fmt 折行规范一致（>80 列折值到下一行），避免 fmt 与 gen 互踩
			fmt.Fprintf(&b, "    %s =>\n      \"%s\"\n", a.name, a.name)
		} else {
			fmt.Fprintf(&b, "%s\n", line)
		}
	}
	b.WriteString("  }\n}\n")

	b.WriteString("\n///|\n" +
		"/// 显示码串（P3 段位前缀单源：E1001 / W3053 / H3057——与 Rust\n" +
		"/// code_prefix + 码值拼接同形）。\n" +
		"pub fn ErrorCode::display_code(self : ErrorCode) -> String {\n  match self {\n")
	for _, a := range sorted {
		fmt.Fprintf(&b, "    %s => \"%s%d\"\n", a.name, prefixOf(a, wSet, hSet), a.code)
	}
	b.WriteString("  }\n}\n")

	b.WriteString("\n///|\n" +
		"/// 全量清单（137，编号升序）——覆盖率断言与差分遍历的载体。\n" +
		"pub fn ErrorCode::all() -> Array[ErrorCode] {\n  [\n")
	for _, a := range sorted {
		fmt.Fprintf(&b, "    %s,\n", a.name)
	}
	b.WriteString("  ]\n}\n")

	b.WriteString("\n///|\n" +
		"/// 编号 → 码；未知编号返回 None。\n" +
		"pub fn ErrorCode::from_code(value : Int) -> ErrorCode? {\n  match value {\n")
	for _, a := range sorted {
		fmt.Fprintf(&b, "    %d => Some(%s)\n", a.code, a.name)
	}
	b.WriteString("    _ => None\n  }\n}\n")

	var withC, withoutC []int
	for _, a := range sorted {
		if entrySet[a.code] {
			withC = append(withC, a.code)
		} else {
			withoutC = append(withoutC, a.code)
		}
	}
	b.WriteString("\n///|\n" +
		"/// 有教学卡片的码清单（升序）。\n" +
		"pub fn catalog_codes() -> Array[Int] {\n  [\n" +
		wrapInts(withC) + "\n  ]\n}\n")
	b.WriteString("\n///|\n" +
		"/// 无卡片码清单（升序；含 Unknown=0）——「码无卡片」可断言清单（T2）。\n" +
		"pub fn codes_without_catalog() -> Array[Int] {\n  [\n" +
		wrapInts(withoutC) + "\n  ]\n}\n")
	return b.String()
}

// wrapInts 与 moon fmt 一致的 80 列贪心装行（前缀 4 空格，元素 "N," 逗号+空格分隔）。
func wrapInts(nums []int) string {
	var lines []string
	cur := "    "
	for _, n := range nums {
		tok := strconv.Itoa(n) + ","
		if cur != "    " && len(cur)+1+len(tok) > 84 {
			lines = append(lines, cur)
			cur = "    "
		}
		if cur == "    " {
			cur += tok
		} else {
			cur += " " + tok
		}
	}
	if cur != "    " {
		lines = append(lines, cur)
	}
	return strings.Join(lines, "\n")
}

func renderCatalog(entries []entry, srcHash string) string {
	var b strings.Builder
	fmt.Fprintf(&b, "// GENERATED by moonbit/scripts/gen_diag —— 禁手改（再生成：cd moonbit && go run ./scripts/gen_diag）。\n"+
		"// 源：native/src/diagnostics/error_catalog/{lexer,parser,semantic,cpp}.rs（枚举源 sha256/%s）\n"+
		"// 基线：77 条（漂移时本脚本 fail loud）。\n\n"+
		"///|\n"+
		"/// 教学卡片查询（穷尽 match 137 臂无下划线兜底；77 臂 Some、60 臂 None）。\n"+
		"/// 无卡片码是登记在案的缺口（codes_without_catalog），不是异常。\n"+
		"pub fn ErrorCode::catalog(self : ErrorCode) -> CatalogEntry? {\n  match self {\n", srcHash)
	entrySet := map[int]bool{}
	for _, e := range entries {
		entrySet[e.code] = true
		// 爆开形态与 moon fmt 输出逐字节一致：fmt 宽度按 UTF-8 字节计
		// （中文 3 字节），causes 整行 >84 字节即爆开（元素并排不再折，
		// 字符串字面量不可折）；Some(CatalogEntry::of(...)) 全参数逐行。
		causes := causesLit(e.commonCauses)
		causesLine := "          [" + causes + "],"
		var causesForm string
		if len(causesLine) <= 84 {
			causesForm = causesLine
		} else {
			causesForm = "          [\n            " + causes + ",\n          ],"
		}
		fmt.Fprintf(&b, "    %s =>\n      Some(\n        CatalogEntry::of(\n          %d,\n          \"%s\",\n          \"%s\",\n          \"%s\",\n%s\n        ),\n      )\n",
			armNames[e.code], e.code, escapeMbt(e.emoji), escapeMbt(e.title), escapeMbt(e.explanation), causesForm)
	}
	// 无卡片臂显式列 None（无兜底臂纪律：增删枚举臂即编译红）
	var noneCodes []int
	for code := range armNames {
		if !entrySet[code] {
			noneCodes = append(noneCodes, code)
		}
	}
	sort.Ints(noneCodes)
	for _, code := range noneCodes {
		fmt.Fprintf(&b, "    %s => None\n", armNames[code])
	}
	b.WriteString("  }\n}\n")
	return b.String()
}

func causesLit(causes []string) string {
	var parts []string
	for _, c := range causes {
		parts = append(parts, "\""+escapeMbt(c)+"\"")
	}
	return strings.Join(parts, ", ")
}

// ---- 写出 / 校验 ----

// writeProducts 写出全部产物并经 moon fmt 规范化（gen 与 fmt 不得互踩：
// 最终形态一律以 moon fmt 输出为准——gen 原始输出仅是中间态）。
// check 模式：写盘 → fmt → 与先前内容逐字节比对，不一致则还原并红。
func writeProducts(dir string, products map[string]string, check bool) {
	type item struct {
		path string
		cur  []byte
	}
	var items []item
	for name, content := range products {
		path := filepath.Join(dir, name)
		var cur []byte
		if check {
			var err error
			cur, err = os.ReadFile(path)
			if err != nil {
				fatalf("-check: 读产物失败 %s: %v", path, err)
			}
		}
		if err := os.MkdirAll(dir, 0o755); err != nil {
			fatalf("建目录失败 %s: %v", dir, err)
		}
		if err := os.WriteFile(path, []byte(content), 0o644); err != nil {
			fatalf("写产物失败 %s: %v", path, err)
		}
		items = append(items, item{path, cur})
	}
	// 统一 fmt（一次调用，路径按字典序稳定）
	sort.Slice(items, func(i, j int) bool { return items[i].path < items[j].path })
	args := []string{"fmt"}
	for _, it := range items {
		args = append(args, it.path)
	}
	if out, err := runMoon(args...); err != nil {
		fatalf("moon fmt 失败（moon 须在 PATH）: %v\n%s", err, out)
	}
	if !check {
		return
	}
	for _, it := range items {
		now, err := os.ReadFile(it.path)
		if err != nil {
			fatalf("-check: 回读产物失败 %s: %v", it.path, err)
		}
		if !bytes.Equal(now, it.cur) {
			// 还原原内容再报红（check 不留副作用）
			if werr := os.WriteFile(it.path, it.cur, 0o644); werr != nil {
				fatalf("-check: 还原产物失败 %s: %v", it.path, werr)
			}
			fatalf("-check: 产物漂移 %s——源已变更未再生成（cd moonbit && go run ./scripts/gen_diag）", it.path)
		}
	}
}

func runMoon(args ...string) (string, error) {
	cmd := exec.Command("moon", args...)
	var buf bytes.Buffer
	cmd.Stdout = &buf
	cmd.Stderr = &buf
	err := cmd.Run()
	return buf.String(), err
}
