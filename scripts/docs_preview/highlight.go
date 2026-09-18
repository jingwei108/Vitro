package main

// highlight.go —— 轻量语法高亮（零第三方依赖，按 docs/ 实际出现的语言裁剪）。
//
// 边界（诚实记录）：这不是 Pygments / Chroma 的等价物，只做词法级着色——
// 注释 / 字符串 / 数字 / 关键字 / 类型 / 函数名 / 预处理指令 / 属性键。
// 不做语义分析、不做宏展开、不分辨同形标识符，词表按语言常见子集手工维护。
//
// 语言覆盖（按 docs/ 实测的围栏信息串统计）：rust cpp c dart bash csharp powershell
// xml json yaml text python toml markdown cmake typescript tsx css html kotlin js，
// 另带 go / java / sql / ini / make / diff / lua / swift 等别名兜底。

import "strings"

type langDef struct {
	lineComments  []string
	blockComments [][2]string
	quotes        string
	keywords      map[string]bool
	types         map[string]bool
	preprocHash   bool // '#' 位于行首 = 预处理指令（C/C++）
	keyish        bool // name 紧跟 ':' 或 '=' 视为键（yaml/toml/ini/json）
	markup        bool // XML/HTML 专用扫描
	dashIdent     bool // 允许 '-' 出现在标识符中（PowerShell）
}

var (
	kwC      = set("auto break case const continue default do else enum extern for goto if inline register restrict return sizeof static struct switch typedef union volatile while __attribute__ __restrict __inline __asm asm")
	kwCPP    = set("alignas alignof and and_eq asm bitand bitor catch class compl concept consteval constexpr constinit const_cast co_await co_return co_yield decltype delete dynamic_cast explicit export false friend mutable namespace new noexcept not not_eq nullptr operator or or_eq private protected public reinterpret_cast requires static_assert static_cast template this thread_local throw true try typeid typename using virtual xor xor_eq override final import module")
	tyC      = set("void char short int long float double signed unsigned bool _Bool size_t ssize_t ptrdiff_t wchar_t FILE va_list int8_t int16_t int32_t int64_t uint8_t uint16_t uint32_t uint64_t uintptr_t NULL true false")
	tyCPP    = set("void char short int long float double signed unsigned bool size_t ssize_t wchar_t FILE va_list nullptr_t string string_view vector map unordered_map set unordered_set list deque pair tuple array optional variant unique_ptr shared_ptr weak_ptr function iterator const_iterator int8_t int16_t int32_t int64_t uint8_t uint16_t uint32_t uint64_t uintptr_t size_type value_type NULL true false")
	kwRust   = set("as async await break const continue crate dyn else enum extern false fn for if impl in let loop match mod move mut pub ref return self Self static struct super trait true type unsafe use where while yield")
	tyRust   = set("i8 i16 i32 i64 i128 isize u8 u16 u32 u64 u128 usize f32 f64 bool char str String Vec VecDeque Option Result Box Rc Arc RefCell Cell HashMap HashSet BTreeMap BTreeSet Cow Path PathBuf OsString Duration Instant Ordering Some None Ok Err Self Rc Weak Mutex RwLock AtomicUsize AtomicBool")
	kwGo     = set("break case chan const continue default defer else fallthrough for func go goto if import interface map package range return select struct switch type var")
	tyGo     = set("bool byte complex64 complex128 error float32 float64 int int8 int16 int32 int64 rune string uint uint8 uint16 uint32 uint64 uintptr any comparable nil true false iota")
	kwPy     = set("and as assert async await break class continue def del elif else except finally for from global if import in is lambda nonlocal not or pass raise return try while with yield None True False self match case")
	tyPy     = set("int float complex bool str bytes bytearray list tuple dict set frozenset object type range print len enumerate zip map filter sorted sum min max abs open isinstance issubclass super Exception ValueError TypeError KeyError IndexError RuntimeError")
	kwJS     = set("var let const function return if else for while do switch case break continue new delete typeof instanceof in of class extends super this null undefined true false async await yield import export default try catch finally throw void static get set with debugger")
	tyJS     = set("String Number Boolean Object Array Promise Math JSON Date RegExp Map Set WeakMap Symbol BigInt Error any unknown never void interface type enum namespace declare readonly as satisfies keyof infer Partial Required Record")
	kwCS     = set("using namespace class struct interface enum public private protected internal static readonly const virtual override abstract sealed async await new return if else for foreach while do switch case break continue try catch finally throw this base null true false var get set partial record where out ref in params delegate event is as typeof nameof lock checked unchecked fixed stackalloc")
	tyCS     = set("int uint long ulong short ushort byte sbyte bool string char double float decimal object dynamic void Task List Dictionary IEnumerable Span Memory Stream Exception Console Math")
	kwDart   = set("class extends implements with mixin abstract final const var void async await sync try catch on finally throw rethrow if else for while do switch case default return new this super static get set late required factory operator typedef enum assert is as in is_")
	tyDart   = set("int double num String bool List Map Set Iterable Future Stream Object Null dynamic Symbol Function Record")
	kwKotlin = set("fun val var class object interface if else when for while return try catch finally throw null true false this super override open abstract sealed data enum companion object init import package suspend inline lateinit vararg by where as is in out typealias")
	tyKotlin = set("Int Long Double Float Short Byte Boolean Char String Any Unit Nothing List MutableList Map MutableMap Set MutableSet Array Sequence Pair Triple")
	kwBash   = set("if then else elif fi case esac for while until do done function in return local export readonly declare source alias unset shift eval exit set trap")
	tyBash   = set("echo cd ls cat grep sed awk rm cp mv mkdir touch chmod curl wget git cargo go python node npm test true false")
	kwPS     = set("function param if else elseif foreach for while switch return try catch finally throw begin process end class enum filter in do until break continue new")
	tyPS     = set("Get-Item Get-ChildItem Set-Item Remove-Item Write-Host Write-Output New-Object Select-Object Where-Object ForEach-Object Test-Path Join-Path Split-Path")
	kwSQL    = set("select from where insert into values update set delete join left right inner outer on group by order having limit offset create table index drop alter primary key foreign references not null default unique distinct as and or in like between case when then else end union all asc desc")
	tySQL    = set("int integer bigint smallint varchar nvarchar text char date datetime timestamp decimal numeric float double boolean blob uuid")
)

func set(s string) map[string]bool {
	m := map[string]bool{}
	for _, w := range strings.Fields(s) {
		m[w] = true
	}
	return m
}

func langDefFor(lang string) *langDef {
	switch strings.ToLower(strings.TrimSpace(lang)) {
	case "c", "h":
		return &langDef{lineComments: []string{"//"}, blockComments: [][2]string{{"/*", "*/"}}, quotes: "\"'", keywords: kwC, types: tyC, preprocHash: true}
	case "cpp", "c++", "cc", "cxx", "hpp", "hh", "hxx":
		return &langDef{lineComments: []string{"//"}, blockComments: [][2]string{{"/*", "*/"}}, quotes: "\"'", keywords: union(kwC, kwCPP), types: union(tyC, tyCPP), preprocHash: true}
	case "rust", "rs":
		return &langDef{lineComments: []string{"//"}, blockComments: [][2]string{{"/*", "*/"}}, quotes: "\"", keywords: kwRust, types: tyRust}
	case "go", "golang":
		return &langDef{lineComments: []string{"//"}, blockComments: [][2]string{{"/*", "*/"}}, quotes: "\"'`", keywords: kwGo, types: tyGo}
	case "python", "py":
		return &langDef{lineComments: []string{"#"}, quotes: "\"'", keywords: kwPy, types: tyPy}
	case "javascript", "js", "jsx", "typescript", "ts", "tsx", "mjs", "cjs":
		return &langDef{lineComments: []string{"//"}, blockComments: [][2]string{{"/*", "*/"}}, quotes: "\"'`", keywords: kwJS, types: tyJS}
	case "csharp", "cs", "c#":
		return &langDef{lineComments: []string{"//"}, blockComments: [][2]string{{"/*", "*/"}}, quotes: "\"'", keywords: kwCS, types: tyCS}
	case "dart":
		return &langDef{lineComments: []string{"//"}, blockComments: [][2]string{{"/*", "*/"}}, quotes: "\"'", keywords: kwDart, types: tyDart}
	case "kotlin", "kt", "kts":
		return &langDef{lineComments: []string{"//"}, blockComments: [][2]string{{"/*", "*/"}}, quotes: "\"'", keywords: kwKotlin, types: tyKotlin}
	case "java":
		return &langDef{lineComments: []string{"//"}, blockComments: [][2]string{{"/*", "*/"}}, quotes: "\"'", keywords: union(kwJS, kwCS), types: union(tyJS, tyCS)}
	case "bash", "sh", "shell", "zsh", "console":
		return &langDef{lineComments: []string{"#"}, quotes: "\"'", keywords: kwBash, types: tyBash}
	case "powershell", "ps1", "pwsh":
		return &langDef{lineComments: []string{"#"}, blockComments: [][2]string{{"<#", "#>"}}, quotes: "\"'", keywords: kwPS, types: tyPS, dashIdent: true}
	case "sql":
		return &langDef{lineComments: []string{"--"}, blockComments: [][2]string{{"/*", "*/"}}, quotes: "\"'", keywords: kwSQL, types: tySQL}
	case "json", "jsonc":
		return &langDef{lineComments: []string{"//"}, quotes: "\"", types: set("true false null"), keyish: true}
	case "yaml", "yml":
		return &langDef{lineComments: []string{"#"}, quotes: "\"'", types: set("true false null yes no on off"), keyish: true}
	case "toml", "ini", "cfg":
		return &langDef{lineComments: []string{"#", ";"}, quotes: "\"'", keyish: true}
	case "cmake":
		return &langDef{lineComments: []string{"#"}, quotes: "\"", types: set("if else elseif endif foreach endforeach function endfunction macro endmacro set unset project add_executable add_library target_link_libraries include message option list find_package target_include_directories")}
	case "make", "makefile":
		return &langDef{lineComments: []string{"#"}, quotes: "\"", types: set("all clean install test build run")}
	case "xml", "html", "xaml", "svg", "razor", "cshtml":
		return &langDef{lineComments: []string{}, blockComments: [][2]string{{"<!--", "-->"}}, markup: true}
	case "css", "scss", "less":
		return &langDef{lineComments: []string{"//"}, blockComments: [][2]string{{"/*", "*/"}}, quotes: "\"'", keyish: true}
	case "lua":
		return &langDef{lineComments: []string{"--"}, quotes: "\"'", types: set("function end if then else elseif for while do return local nil true false and or not")}
	case "swift":
		return &langDef{lineComments: []string{"//"}, blockComments: [][2]string{{"/*", "*/"}}, quotes: "\"'", keywords: set("func var let class struct enum protocol extension if else guard for while switch case return try catch throw import init deinit self super nil true false async await actor some any where in as is")}
	case "diff", "patch":
		return &langDef{}
	}
	return nil
}

func union(a, b map[string]bool) map[string]bool {
	m := make(map[string]bool, len(a)+len(b))
	for k := range a {
		m[k] = true
	}
	for k := range b {
		m[k] = true
	}
	return m
}

func isIdentByte(b byte) bool {
	return b >= 'a' && b <= 'z' || b >= 'A' && b <= 'Z' || b == '_' || b >= 0x80
}

func isDigitByte(b byte) bool { return b >= '0' && b <= '9' }

func span(class, text string) string {
	return `<span class="` + class + `">` + escHTML(text) + `</span>`
}

// writeRawByte 写出未着色的普通字节：必须转义，否则代码里的 `<u32>`、`<class T>`
// 会被浏览器当成 HTML 标签吃掉（内容静默消失）。
func writeRawByte(b *strings.Builder, c byte) {
	switch c {
	case '&':
		b.WriteString("&amp;")
	case '<':
		b.WriteString("&lt;")
	case '>':
		b.WriteString("&gt;")
	default:
		b.WriteByte(c)
	}
}

// highlightCode 输出已转义的高亮 HTML（不含 <pre> 外壳）。
func highlightCode(lang, code string) string {
	def := langDefFor(lang)
	if def == nil {
		return escHTML(code)
	}
	if def.markup {
		return highlightMarkup(def, code)
	}
	if lang == "diff" || lang == "patch" {
		return highlightDiff(code)
	}

	var b strings.Builder
	i := 0
	atLineStart := true
	for i < len(code) {
		ch := code[i]

		// 行注释
		matched := false
		for _, lc := range def.lineComments {
			if strings.HasPrefix(code[i:], lc) {
				j := strings.IndexByte(code[i:], '\n')
				if j < 0 {
					j = len(code) - i
				}
				b.WriteString(span("tok-c", code[i:i+j]))
				i += j
				matched = true
				break
			}
		}
		if matched {
			continue
		}
		// 块注释
		for _, bc := range def.blockComments {
			if strings.HasPrefix(code[i:], bc[0]) {
				j := strings.Index(code[i+len(bc[0]):], bc[1])
				if j < 0 {
					j = len(code) - i
				} else {
					j += len(bc[0]) + len(bc[1])
				}
				b.WriteString(span("tok-c", code[i:i+j]))
				i += j
				matched = true
				break
			}
		}
		if matched {
			continue
		}
		// C/C++ 预处理指令
		if def.preprocHash && ch == '#' && atLineStart {
			j := strings.IndexByte(code[i:], '\n')
			if j < 0 {
				j = len(code) - i
			}
			b.WriteString(span("tok-p", code[i:i+j]))
			i += j
			continue
		}
		// 字符串
		if strings.IndexByte(def.quotes, ch) >= 0 && ch != 0 {
			j := scanString(code, i, ch)
			b.WriteString(span("tok-s", code[i:j]))
			i = j
			continue
		}
		// 数字
		if isDigitByte(ch) && (i == 0 || !isIdentByte(code[i-1])) {
			j := scanNumber(code, i)
			b.WriteString(span("tok-m", code[i:j]))
			i = j
			continue
		}
		// 标识符
		if isIdentByte(ch) {
			j := i
			for j < len(code) && (isIdentByte(code[j]) || isDigitByte(code[j]) ||
				(def.dashIdent && code[j] == '-' && j+1 < len(code) && isIdentByte(code[j+1]))) {
				j++
			}
			word := code[i:j]
			next := nextNonSpace(code, j)
			switch {
			case def.keywords[word]:
				b.WriteString(span("tok-k", word))
			case def.types[word]:
				b.WriteString(span("tok-t", word))
			case next < len(code) && code[next] == '(':
				b.WriteString(span("tok-f", word))
			case next < len(code) && code[next] == '!' && next+1 < len(code) && code[next+1] == '(':
				b.WriteString(span("tok-f", word))
			case def.keyish && next < len(code) && (code[next] == ':' || code[next] == '='):
				b.WriteString(span("tok-v", word))
			default:
				b.WriteString(word)
			}
			i = j
			continue
		}
		// shell / powershell 变量
		if ch == '$' && i+1 < len(code) && (isIdentByte(code[i+1]) || code[i+1] == '{' || code[i+1] == '(') {
			j := i + 1
			if code[j] == '{' {
				for j < len(code) && code[j] != '}' {
					j++
				}
				if j < len(code) {
					j++
				}
			} else if code[j] == '(' {
				depth := 0
				for j < len(code) {
					if code[j] == '(' {
						depth++
					}
					if code[j] == ')' {
						depth--
						if depth == 0 {
							j++
							break
						}
					}
					j++
				}
			} else {
				for j < len(code) && (isIdentByte(code[j]) || isDigitByte(code[j])) {
					j++
				}
			}
			b.WriteString(span("tok-v", code[i:j]))
			i = j
			continue
		}

		if ch == '\n' {
			atLineStart = true
			b.WriteByte(ch)
			i++
			continue
		}
		if ch != ' ' && ch != '\t' {
			atLineStart = false
		}
		writeRawByte(&b, ch)
		i++
	}
	return b.String()
}

func nextNonSpace(s string, i int) int {
	for i < len(s) && (s[i] == ' ' || s[i] == '\t') {
		i++
	}
	return i
}

// scanString 处理转义、多行与原样字符串（“ ` “ 在 Go/JS 中不转义）。
func scanString(s string, i int, q byte) int {
	j := i + 1
	for j < len(s) {
		c := s[j]
		if c == '\\' && q != '`' {
			j += 2
			continue
		}
		if c == q {
			return j + 1
		}
		if c == '\n' && q != '`' {
			return j
		}
		j++
	}
	return len(s)
}

func scanNumber(s string, i int) int {
	j := i
	if strings.HasPrefix(s[i:], "0x") || strings.HasPrefix(s[i:], "0X") ||
		strings.HasPrefix(s[i:], "0b") || strings.HasPrefix(s[i:], "0B") {
		j += 2
		for j < len(s) && (isHex(s[j]) || s[j] == '_' || s[j] == '\'') {
			j++
		}
		return j
	}
	for j < len(s) {
		c := s[j]
		if isDigitByte(c) || c == '.' || c == '_' || c == '\'' {
			j++
			continue
		}
		if (c == 'e' || c == 'E' || c == 'x' || c == 'p' || c == 'P') && j+1 < len(s) {
			j++
			if j < len(s) && (s[j] == '+' || s[j] == '-') {
				j++
			}
			continue
		}
		if c == 'f' || c == 'F' || c == 'u' || c == 'U' || c == 'l' || c == 'L' ||
			c == 'i' || c == 'z' || c == 'd' {
			j++
			continue
		}
		break
	}
	return j
}

// highlightMarkup 处理 XML / HTML / XAML：标签名、属性名、属性值、注释、实体。
// 注意：这是**代码块内的着色**，所有结构字符都必须转义输出（`&lt;` 而不是 `<`），
// 否则代码里写的标签会被浏览器当真实标签吃掉。
func highlightMarkup(def *langDef, code string) string {
	var b strings.Builder
	i := 0
	for i < len(code) {
		if strings.HasPrefix(code[i:], "<!--") {
			j := strings.Index(code[i+4:], "-->")
			if j < 0 {
				j = len(code) - i
			} else {
				j += 7
			}
			b.WriteString(span("tok-c", code[i:i+j]))
			i += j
			continue
		}
		if code[i] == '<' {
			j := i + 1
			if j < len(code) && code[j] == '/' {
				j++
			}
			start := j
			for j < len(code) && (isIdentByte(code[j]) || isDigitByte(code[j]) || code[j] == '-' || code[j] == ':') {
				j++
			}
			name := code[start:j]
			b.WriteString("&lt;")
			if start > i+1 {
				b.WriteString("/")
			}
			b.WriteString(span("tok-t", name))
			i = j
			// 属性区
			for i < len(code) && code[i] != '>' {
				if code[i] == '"' || code[i] == '\'' {
					k := scanString(code, i, code[i])
					b.WriteString(span("tok-s", code[i:k]))
					i = k
					continue
				}
				if isIdentByte(code[i]) {
					k := i
					for k < len(code) && (isIdentByte(code[k]) || isDigitByte(code[k]) || code[k] == '-' || code[k] == ':') {
						k++
					}
					b.WriteString(span("tok-v", code[i:k]))
					i = k
					continue
				}
				if code[i] == '\n' {
					break
				}
				writeRawByte(&b, code[i])
				i++
			}
			continue
		}
		if code[i] == '&' {
			if n := entityLen(code[i:]); n > 0 {
				b.WriteString(span("tok-m", code[i:i+n]))
				i += n
				continue
			}
		}
		writeRawByte(&b, code[i])
		i++
	}
	_ = def
	return b.String()
}

func highlightDiff(code string) string {
	var b strings.Builder
	for _, line := range strings.Split(code, "\n") {
		switch {
		case strings.HasPrefix(line, "+"):
			b.WriteString(span("tok-ins", line))
		case strings.HasPrefix(line, "-"):
			b.WriteString(span("tok-del", line))
		case strings.HasPrefix(line, "@@"):
			b.WriteString(span("tok-v", line))
		default:
			b.WriteString(escHTML(line))
		}
		b.WriteByte('\n')
	}
	return strings.TrimSuffix(b.String(), "\n")
}
