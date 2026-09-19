///|
/// 宏体参数替换 + `#`/`##` 处理（C99 §6.10.3.1/§6.10.3.3 教学子集）：
/// - `#` 后跟参数：实参 token 的**原始拼写**（未经宏展开）串化为字符串字面量；
/// - `a ## b`：左操作数**最后一个** token 与右操作数**第一个** token 拼接，
///   其余原序保留；拼接结果必须词法化为**恰好一个**合法 token（失败记
///   E1016，fail-soft 保留左操作数）；
/// - 两个操作数都不做宏展开（C 规则特例）。
/// 诚实放弃（进 spec §2.11）：空实参 placemarker、拼接出预处理数字的边界、
/// `##` 递归构造元编程。
priv struct SubstituteResult {
  tokens : Array[@token.Token]
  paste_err : String?
}

///|
/// 对象宏形态（无参数表）。
fn substitute_params(
  body : Array[@token.Token],
  params : Array[String],
  args : Array[Array[@token.Token]]?,
) -> SubstituteResult {
  substitute_params_ext(body, params, args, args)
}

///|
/// 双实参形态：`raw_args` 供 `#`/`##` 操作数（C 规则：不展开），
/// `expanded_args` 供普通参数位置（C99 §6.10.3.1：实参先行展开）。
fn substitute_params_ext(
  body : Array[@token.Token],
  params : Array[String],
  raw_args : Array[Array[@token.Token]]?,
  expanded_args : Array[Array[@token.Token]]?,
) -> SubstituteResult {
  let out : Array[@token.Token] = []
  let mut paste_err : String? = None
  let mut i = 0
  while i < body.length() {
    let t = body[i]
    // `#` 字符串化：必须紧跟参数名
    if t.ty is Hash {
      if i + 1 < body.length() && body[i + 1].ty is Identifier {
        let n = body[i + 1]
        match arg_of(n.text, params, raw_args) {
          Some(raw) => {
            let parts : Array[String] = []
            for x in raw {
              parts.push(x.text)
            }
            out.push({
              ty: String,
              text: parts.join(" "),
              loc: t.loc,
              byte_off: t.byte_off,
              real_line: t.real_line,
            })
            i += 2
            continue
          }
          None => {
            paste_err = Some(
              "# 字符串化的操作数必须是宏参数名，得到 `\{n.text}`",
            )
            i += 2
            continue
          }
        }
      }
      paste_err = Some("# 后必须紧跟宏参数名")
      i += 1
      continue
    }
    // `##` 拼接：左取 out 末尾（参数位置替换的是原始实参），右取下一操作数首 token
    if t.ty is HashHash {
      let left = out.pop()
      let right_tokens : Array[@token.Token] = if i + 1 < body.length() {
        let n = body[i + 1]
        if n.ty is Identifier {
          match arg_of(n.text, params, raw_args) {
            Some(a) => a
            None => [n]
          }
        } else {
          [n]
        }
      } else {
        []
      }
      match (left, right_tokens.length() > 0) {
        (Some(l), true) => {
          let combined = "\{l.text}\{right_tokens[0].text}"
          let scratch : Array[@token.LexError] = []
          let ptoks = @scanner.scan_fragment(
            combined,
            base_byte_off=0,
            base_line=l.loc.line,
            base_column=l.loc.column,
            file_id=l.loc.file_id,
            body_mode=false,
            errors=scratch,
          )
          // Rust 口径：`Lexer::new(&combined).tokenize()`（非 body 模式，`#`
          // 被当指令消费）——本侧行内 stray `#` 报 E1001 使拼接判失败，行为
          // 同样落入 E1016 fail-soft（差异登记：仅 `#` 参与拼接的病态形状）
          if scratch.length() == 0 &&
            ptoks.length() == 1 &&
            !(ptoks[0].ty is Unknown) {
            out.push({
              ty: ptoks[0].ty,
              text: ptoks[0].text,
              loc: l.loc,
              byte_off: l.byte_off,
              real_line: l.real_line,
            })
            for k in 1..<right_tokens.length() {
              out.push(right_tokens[k])
            }
          } else {
            paste_err = Some(
              "`\{l.text}` 拼接结果 `\{combined}` 不是单个合法 token",
            )
            out.push(l)
            for k in 1..<right_tokens.length() {
              out.push(right_tokens[k])
            }
          }
        }
        _ => paste_err = Some("## 缺少拼接操作数")
      }
      i += 2
      continue
    }
    // 普通参数：替换为先行展开的实参
    if t.ty is Identifier {
      match arg_of(t.text, params, expanded_args) {
        Some(exp) => {
          for x in exp {
            out.push(x)
          }
          i += 1
          continue
        }
        None => ()
      }
    }
    out.push(t)
    i += 1
  }
  { tokens: out, paste_err, }
}

///|
fn arg_of(
  name : String,
  params : Array[String],
  src : Array[Array[@token.Token]]?,
) -> Array[@token.Token]? {
  let src = match src {
    Some(s) => s
    None => return None
  }
  let mut idx : Int? = None
  for k in 0..<params.length() {
    if params[k] == name {
      idx = Some(k)
      break
    }
  }
  match idx {
    Some(k) => if k < src.length() { Some(src[k]) } else { None }
    None => None
  }
}
