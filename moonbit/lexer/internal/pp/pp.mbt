///|
/// 条件编译状态（`open_line` 为 ⑥-23 修复：Rust 不记录开启行，E1013 报在 EOF 行）。
priv struct CondState {
  mut active : Bool
  mut has_else : Bool
  mut taken : Bool
  open_line : Int
}

///|
/// include 栈帧（显式栈——替代 Rust 的 `#__vitro_push_dir/pop_dir` 哨兵注入，
/// 勘察 ③-2 抛弃项）。`cond_boundary` 承担 U1#11 的条件栈边界记账。
priv struct IncludeFrame {
  dir : String
  cond_boundary : Int
  file_id : Int
}

///|
/// 预处理器 pass（独立于词法扫描——勘察 ⑤-1 目标架构）。
/// 管线：BOM 剥离 → `\` 续行拼接 → 注释剥离（字符串感知、跨逻辑行）→
/// 逐逻辑行（指令 / 活跃普通行词法化）→ include 递归（LineMap 记账）→
/// 全量宏展开。
struct Pp[P] {
  provider : P
  cpp_mode : Bool
  macros : Map[String, @token.MacroDef]
  errors : Array[@token.LexError]
  warnings : Array[@token.LexWarning]
  trace : Array[@token.TraceEntry]
  /// include 段的行号映射（主文件 token 由 token.loc.file_id=0 直接判定；
  /// 仿古坐标下主/头段区间重叠是 Rust splice 补偿缺陷的信息丢失，LineMap
  /// 对重叠区间返回首个命中段——精确归属以 file_id 为准，登记于包注释）。
  spans : Array[@token.LineSpan]
  /// 主文件目录（quote 候选链终点）。
  base_dir : String
  include_stack : Array[IncludeFrame]
  cond_stack : Array[CondState]
  /// include-once key 集合（`stub:<名>` 或规范路径）。
  processed : Set[String]
  /// 规范路径 → file_id（LineMap/诊断归属单源）。
  file_ids : Map[String, Int]
  mut next_file_id : Int
  /// 文件内容缓存（环检测 DFS 与实际拼接共用一次读取）。
  content_cache : Map[String, String]
}

///|
pub fn[P] Pp::new(
  provider~ : P,
  cpp_mode~ : Bool,
  base_dir? : String = ".",
) -> Pp[P] {
  {
    provider,
    cpp_mode,
    macros: builtin_macro_table(),
    errors: [],
    warnings: [],
    trace: [],
    spans: [],
    base_dir,
    include_stack: [],
    cond_stack: [],
    processed: Set([]),
    file_ids: Map([]),
    next_file_id: 1,
    content_cache: Map([]),
  }
}

///|
/// 逻辑行（`\` 续行拼接产物）。`byte_off` 为物理首行起点在源内偏移
///（拼接后行内偏移按拼接文本计——续行形状不参与 L1/L2 对拍，口径自由）。
priv struct LogicalLine {
  text : String
  head_line : Int
  byte_off : Int
}

///|
fn[P : @token.SourceProvider] Pp::err(
  self : Pp[P],
  message : String,
  line : Int,
  code~ : @diag.ErrorCode,
) -> Unit {
  self.errors.push({
    message,
    loc: { line, column: 0, file_id: self.current_file_id(), },
    code,
  })
}

///|
/// L2 完整管线入口（主文件与头文件递归共用；仿古行号基准由调用方传入——
/// 头文件递归时 `main_line_base` = `L - N - 2`，使头文件第 k 行 token 的
/// line = `L - N - 2 + k`，逐字节复刻 Rust splice 补偿后的主坐标系行号）。
pub fn[P : @token.SourceProvider] Pp::run_pp(
  self : Pp[P],
  src : String,
  main_line_base? : Int = 1,
  main_byte_base? : Int = 0,
  main_file_id? : Int = 0,
  span_main_first? : Int? = None,
  span_file_line? : Int = 1,
  emit_eof? : Bool = false,
) -> Array[@token.Token] {
  // BOM 剥离（修复 ⑥-8：Rust 把 BOM 当普通字符报 E1001；已知差异，L1 不剥保持可比）
  let src = strip_bom(src)
  let physical = split_physical_lines(src, main_byte_base)
  let logical = splice_continuations(physical)
  let stripped = strip_comments_stateful(logical)
  let tokens : Array[@token.Token] = []
  // 每行"行后有换行"（E1003+E1002 双诊断判据）：非末行恒真；末行 = 源尾
  // 是否带换行（Rust 整源扫描语义；续行拼接形状属 D1 差异域，取首行口径）
  let src_tail_newline = src.length() > 0 && src[src.length() - 1] == '\n'
  for line in stripped.lines {
    let trailing_nl = if line.head_line < physical.length() {
      true
    } else {
      src_tail_newline
    }
    let trimmed = leading_trim(line.text)
    if trimmed == "" {
      continue
    }
    if trimmed[0].to_int() == '#' {
      self.handle_directive(
        trimmed.substring(start=1),
        line.head_line + main_line_base - 1,
        tokens,
      )
      continue
    }
    if self.is_skipping() {
      continue
    }
    let line_tokens = @scanner.scan_fragment(
      line.text,
      base_byte_off=line.byte_off,
      // 仿古主坐标（对拍）与文件内真实行号（归属）双基准——S2 审阅 P1-6
      base_real_line=line.head_line,
      trailing_newline=trailing_nl,
      base_line=line.head_line + main_line_base - 1,
      base_column=1,
      file_id=main_file_id,
      cpp_mode=self.cpp_mode,
      body_mode=false,
      errors=self.errors,
    )
    for t in line_tokens {
      tokens.push(t)
    }
  }
  // 头文件内注释未闭合：跨文件注释态重置并报错（Rust 会把注释吞进主文件，
  // 差异登记）；主文件同样在末尾报告
  if stripped.ended_in_comment {
    let last_line = if stripped.lines.length() > 0 {
      stripped.lines[stripped.lines.length() - 1].head_line + main_line_base - 1
    } else {
      main_line_base
    }
    self.err("块注释未闭合", last_line, code=E1010_UnterminatedComment)
  }
  if emit_eof {
    // 主文件铺底段（S2 审阅 P1-6：此前 spans 恒缺主文件段，lookup 全错）——
    // unshift 保证它先于 include 段（lookup 尾向优先 = 最近拼接段优先）
    let bedrock_last = if stripped.lines.length() > 0 {
      stripped.lines[stripped.lines.length() - 1].head_line + main_line_base - 1
    } else {
      main_line_base
    }
    self.spans.insert(0, {
      main_first: 1,
      main_last: bedrock_last,
      file_id: main_file_id,
      file_line: 1,
    })
    // Eof 位置复刻 Rust 扫描器全消费点：物理行数 N + 尾换行 +1；列 = 尾行
    // 标量数 +1（无尾换行时）。空源 N=0 → line 1（Rust 初始位）
    let n = physical.length()
    let tail_newline = src.length() > 0 && src[src.length() - 1] == '\n'
    let eof_line = if n == 0 { 1 } else if tail_newline { n + 1 } else { n }
    let eof_col = if n == 0 || tail_newline {
      1
    } else {
      scalar_count(physical[n - 1].text) + 1
    }
    tokens.push({
      ty: Eof,
      text: "",
      loc: { line: eof_line, column: eof_col, file_id: main_file_id, },
      byte_off: main_byte_base + @utf8.encode(src).length(),
      real_line: if n == 0 {
        1
      } else if tail_newline {
        n + 1
      } else {
        n
      },
    })
  }
  if span_main_first is Some(first) {
    let last = if stripped.lines.length() > 0 {
      stripped.lines[stripped.lines.length() - 1].head_line + main_line_base - 1
    } else {
      main_line_base
    }
    self.spans.push({
      main_first: first,
      main_last: last,
      file_id: main_file_id,
      file_line: span_file_line,
    })
  }
  tokens
}

///|
/// 主文件管线收尾（E1013 按开启行报告——⑥-23 修复）+ 宏展开。
pub fn[P : @token.SourceProvider] Pp::finish_pp(
  self : Pp[P],
  tokens : Array[@token.Token],
) -> @token.LexResult {
  if self.cond_stack.length() > 0 {
    // 报最内层未闭合组的开启行（Rust 报 EOF 行——差异登记）
    let st = self.cond_stack[self.cond_stack.length() - 1]
    self.err(
      "未闭合的 #ifdef / #ifndef 块",
      st.open_line,
      code=E1013_UnclosedConditional,
    )
    self.cond_stack.clear()
  }
  let expanded = self.expand_tokens_all(tokens)
  {
    tokens: expanded,
    errors: self.errors,
    warnings: self.warnings,
    trace: self.trace,
    line_map: { spans: self.spans, },
  }
}

///|
fn[P] Pp::current_file_id(self : Pp[P]) -> Int {
  if self.include_stack.length() > 0 {
    self.include_stack[self.include_stack.length() - 1].file_id
  } else {
    0
  }
}

///|
fn[P] Pp::is_skipping(self : Pp[P]) -> Bool {
  for s in self.cond_stack {
    if !s.active {
      return true
    }
  }
  false
}

///|
/// **父级**跳过态（排除栈顶）——`#elif` / `#else` 的判定依据（Rust `parent_skipping`）。
fn[P] Pp::parent_skipping(self : Pp[P]) -> Bool {
  let n = self.cond_stack.length()
  for i = 0; i + 1 < n; i = i + 1 {
    if !self.cond_stack[i].active {
      return true
    }
  }
  false
}

///|
/// BOM（EF BB BF）剥离——仅 L2 管线（L1 raw 保持 Rust 同构的"当普通字符"行为）。
fn strip_bom(src : String) -> String {
  let b = @utf8.encode(src)
  if b.length() >= 3 && b[0] == b'\xEF' && b[1] == b'\xBB' && b[2] == b'\xBF' {
    return @utf8.decode(b[3:]) catch { _ => "" }
  }
  src
}

///|
/// 物理行拆分（含 \r\n 的 \r 剥离；记录行起点偏移）。
fn split_physical_lines(src : String, byte_base : Int) -> Array[LogicalLine] {
  let bytes = @utf8.encode(src)
  let lines : Array[LogicalLine] = []
  let mut start = 0
  let mut line_no = 1
  let mut i = 0
  while i < bytes.length() {
    if bytes[i] == b'\n' {
      // S2 审阅 P2：保留行尾 \r（Rust 整源扫描不剥——字符串值含 \r、
      // 列号含 \r 计数；scanner 的空白跳过吃它，token 语义不受影响）
      let end_ = i
      lines.push({
        text: @utf8.decode(bytes[start:end_]) catch {
          _ => ""
        },
        head_line: line_no,
        byte_off: byte_base + start,
      })
      line_no += 1
      start = i + 1
    }
    i += 1
  }
  if start < bytes.length() {
    lines.push({
      text: @utf8.decode(bytes[start:]) catch {
        _ => ""
      },
      head_line: line_no,
      byte_off: byte_base + start,
    })
  }
  lines
}

///|
/// `\` 续行拼接（phase 2——修复 ⑥-4：Rust 无此阶段，多行宏报 E1001；已知差异）。
/// 规则：物理行（\r 已剥）以 `\` 结尾 → 去掉 `\` 与下一行拼接。
fn splice_continuations(lines : Array[LogicalLine]) -> Array[LogicalLine] {
  let out : Array[LogicalLine] = []
  let mut i = 0
  while i < lines.length() {
    let mut cur = lines[i]
    while ends_with_backslash(cur.text) && i + 1 < lines.length() {
      i += 1
      let next = lines[i]
      cur = {
        text: drop_last_char(cur.text) + next.text,
        head_line: cur.head_line,
        byte_off: cur.byte_off,
      }
    }
    out.push(cur)
    i += 1
  }
  out
}

///|
fn ends_with_backslash(s : String) -> Bool {
  s.length() > 0 && s[s.length() - 1] == '\\'
}

///|
/// 去掉行尾反斜杠（ASCII 单 code unit——续行形状保证行尾为 `\`）。
fn drop_last_char(s : String) -> String {
  // 按字节长度切（String::length 是 UTF-16 码元数——非 ASCII 行按其切片会
  // 落进多字节序列中间，decode 抛错；此前兜底空串 = 整行前缀静默丢失，
  // S2 审阅 P1-1 实锤：中文注释续行把注释吞成代码行。兜底返回原串 s
  //（防御路径：行尾保证为 ASCII 反斜杠，正常不触发）
  let b = @utf8.encode(s)
  @utf8.decode(b[:b.length() - 1]) catch {
    _ => s
  }
}

///|
/// 前导空白裁剪（空格/制表——行级指令判定的标准前处理）。
fn leading_trim(s : String) -> String {
  let mut i = 0
  while i < s.length() {
    let c = s[i]
    if c == ' ' || c == '\t' {
      i += 1
    } else {
      break
    }
  }
  if i == 0 {
    s
  } else {
    s.substring(start=i)
  }
}

///|
priv struct StrippedLines {
  lines : Array[LogicalLine]
  ended_in_comment : Bool
}

///|
/// 注释剥离（phase 3：注释 → 等宽空格，**字符串/字符字面量感知**；跨逻辑行
/// 维护块注释态）。行内 `//` 截断。引号态不跨行（未闭合字符串/字符的后续行
/// 按普通行剥离——该形状已被扫描器报 E1002/E1003，剥离口径不影响诊断）。
fn strip_comments_stateful(lines : Array[LogicalLine]) -> StrippedLines {
  let out : Array[LogicalLine] = []
  let mut in_block = false
  for line in lines {
    let bytes = @utf8.encode(line.text)
    let buf : Array[Byte] = []
    let mut i = 0
    let mut state : Int = 0 // 0 normal, 1 string, 2 char
    while i < bytes.length() {
      let c = bytes[i]
      if in_block {
        if c == b'*' && i + 1 < bytes.length() && bytes[i + 1] == b'/' {
          buf.push(b' ')
          buf.push(b' ')
          i += 2
          in_block = false
        } else {
          buf.push(b' ')
          i += 1
        }
        continue
      }
      match state {
        0 =>
          if c == b'/' && i + 1 < bytes.length() && bytes[i + 1] == b'/' {
            break // 行内注释截断
          } else if c == b'/' && i + 1 < bytes.length() && bytes[i + 1] == b'*' {
            buf.push(b' ')
            buf.push(b' ')
            i += 2
            in_block = true
          } else {
            if c == b'"' {
              state = 1
            } else if c == b'\'' {
              state = 2
            }
            buf.push(c)
            i += 1
          }
        _ =>
          if c == b'\\' && i + 1 < bytes.length() {
            buf.push(c)
            buf.push(bytes[i + 1])
            i += 2
          } else {
            if (state == 1 && c == b'"') || (state == 2 && c == b'\'') {
              state = 0
            }
            buf.push(c)
            i += 1
          }
      }
    }
    out.push({
      text: @utf8.decode(Bytes::from_array(buf)) catch {
        _ => ""
      },
      head_line: line.head_line,
      byte_off: line.byte_off,
    })
  }
  { lines: out, ended_in_comment: in_block, }
}

///|
/// 标量字符计数（Rust char 计数口径）。
fn scalar_count(s : String) -> Int {
  let mut n = 0
  for _ in s {
    n += 1
  }
  n
}
