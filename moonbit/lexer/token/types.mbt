///|
/// 词法单元。位置口径（L1/L2 差分契约，登记于包 README）：
/// - `loc.line`：主文件坐标系（include 拼接后仿古口径——与 Rust oracle 的
///   `line -= inserted_newlines` 补偿后行为一致；头文件 token 的真实归属由
///   `loc.file_id` + `line_map` 携带，这是新架构增量，不破坏对拍）；
/// - `loc.column`：**精确复刻** Rust 缺陷口径 `column - text 的 UTF-8 字节数`
///   （扫描按 Unicode 标量计数 × 反推按字节——⑥-1/⑥-2 已知偏差故意保留，
///   换取 col 字段逐字节可比；无缺陷坐标用 `byte_off`）；
/// - `byte_off`：token 首字节的源内偏移（0 起，BOM 剥离后坐标系），扫描器直记
///   ——L1 强锚字段；L2 展开产物钉宏调用点的 byte_off（与 line/column 钉点一致）。
pub(all) struct Token {
  ty : TokenType
  text : String
  loc : @source.SourceLoc
  byte_off : Int
  /// token 在**自身所属文件内**的真实行号（主文件 = loc.line；头文件 = 文件内
  /// 行号——与仿古 loc.line 的主坐标系解耦；宏展开产物钉调用点的 real_line）。
  /// S2 审阅 P1-6 修复：真实归属的权威通道（LineMap 对重叠仿古区间只能尽力）。
  real_line : Int
} derive(Eq, @debug.Debug)

///|
/// 词法错误（致命，收集不中断——与 Rust `LexerError` 同构）。
/// `code` 为类型化错误码（diag 包 137 臂），`code()` 取数字与 Rust `code : i32` 对齐。
pub(all) struct LexError {
  message : String
  loc : @source.SourceLoc
  code : @diag.ErrorCode
} derive(Eq, @debug.Debug)

///|
/// E2 白箱教学层：预处理阶段**非致命**提示（宏遮蔽 W1018 / 宏参数副作用 W1019）。
pub(all) struct LexWarning {
  message : String
  loc : @source.SourceLoc
  code : @diag.ErrorCode
} derive(Eq, @debug.Debug)

///|
/// 宏定义。对象式与函数式**显式分型**（勘察 ⑥-9 修复：Rust `params : Vec<String>`
/// 以 `is_empty()` 判对象宏，导致 `#define FOO() 42` 零参数函数式宏被误当对象宏）。
pub(all) enum MacroDef {
  Object(body~ : Array[Token])
  Function(params~ : Array[String], body~ : Array[Token])
} derive(Eq, @debug.Debug)

///|
/// 教学追踪（结构化——勘察 ⑤-6：Rust 侧是拼好的中文串；本侧结构化存储，
/// `render` 输出与 Rust wire format 逐字一致的中文串，serve 出口契约不变）。
pub(all) enum TraceEntry {
  Expand(name~ : String, args~ : String?, result~ : String, depth~ : Int)
  CondBranch(expr~ : String, value~ : Bool, reason~ : String)
} derive(Eq, @debug.Debug)

///|
/// 渲染为 Rust `preprocessor_trace` wire format（`session_api.rs` 消费的中文串）：
/// - 展开：`NAME(args) ⇒ 结果拼写`（对象宏无括号）；
/// - 分支：``#if `expr` → 真/假``。
pub fn TraceEntry::render(self : TraceEntry) -> String {
  match self {
    Expand(name~, args~, result~, ..) => {
      let call = match args {
        Some(a) => "\{name}(\{a})"
        None => name
      }
      "\{call} ⇒ \{result}"
    }
    CondBranch(expr~, value~, ..) => {
      let word = if value { "真" } else { "假" }
      "#if `\{expr}` → \{word}"
    }
  }
}

///|
/// LineMap 区间：主文件行号区间 `[main_first, main_last]` 映射到真实文件坐标。
/// 区间按拼接顺序单调递增、首尾相接铺满全源。
pub(all) struct LineSpan {
  main_first : Int
  main_last : Int
  file_id : Int
  file_line : Int
} derive(Eq, @debug.Debug)

///|
/// include 拼接后的行号映射（勘察 ⑤-1：预处理独立 pass 的结构性产物）。
/// Rust oracle 因 `Vec::splice` 行号补偿缺陷（③-3）无法提供此信息——本侧
/// token 的 `loc.line` 仍按仿古口径对拍，真实归属经 `lookup` 查询。
pub(all) struct LineMap {
  spans : Array[LineSpan]
} derive(Eq, @debug.Debug)

///|
/// 查主文件行号对应的真实文件坐标 `(file_id, 真实行号)`。
/// 未命中（区间缝隙）返回最近的左侧区间——区间铺满设计下不应发生，防御性兜底。
pub fn LineMap::lookup(self : LineMap, main_line : Int) -> (Int, Int) {
  // S2 审阅 P1-6 修正：仿古主坐标下主/头文件段区间重叠是行号补偿缺陷的
  // 固有信息丢失——尾向扫描取"最近拼接段"（include 段晚于主文件铺底段插入，
  // 后记录者优先）；**权威归属请用 Token.loc.file_id + Token.real_line**
  //（本查找仅为行号级尽力）
  let mut best : (Int, Int)? = None
  let mut hit = false
  for span in self.spans {
    if main_line >= span.main_first && main_line <= span.main_last {
      // 命中优先——不被后续"区间在此行之前"的段覆盖（P1-6 尾向语义的边界）
      best = Some(
        (span.file_id, span.file_line + (main_line - span.main_first)),
      )
      hit = true
    } else if span.main_last < main_line && !hit {
      best = Some(
        (span.file_id, span.file_line + (span.main_last - span.main_first)),
      )
    }
  }
  match best {
    Some(b) => b
    None => (0, main_line)
  }
}

///|
/// L1 结果：字符级原始词法（无行拼接、无指令消费、无宏展开；`#` 产出 Hash/HashHash）。
pub(all) struct RawResult {
  tokens : Array[Token]
  errors : Array[LexError]
} derive(Eq, @debug.Debug)

///|
/// L2 结果：完整管线（行拼接 → 指令消费 → include → 宏展开）。
/// 诊断与 token 流**单值同返**（勘察 8.3-1 硬约束：禁止"只看 tokens 不看 errors"
/// 的调用形态；差分驱动对 `errors`/`warnings` 计数强比对）。
pub(all) struct LexResult {
  tokens : Array[Token]
  errors : Array[LexError]
  warnings : Array[LexWarning]
  trace : Array[TraceEntry]
  line_map : LineMap
} derive(Eq, @debug.Debug)
