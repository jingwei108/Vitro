///|
/// S3 差分出口（开发工具，native only）：批量产出解析层 dump——AST JSON
///（E1 面，@ast.ast_dump_json 与 Rust serde 输出经 Go canonicalizer 归一
/// 后逐字节比对）+ 诊断序列 TSV（E2 面：code/line/column/保序，message
/// 单列）+ 活性计数（勘察 6-3 观测义务）。
///
/// 用法：moon run cmd/dump_ast -- <corpus_dir|file.c> <out_dir>
///
/// 输出（每输入 <name>.c）：
/// - <name>.ast.json：与 Rust serve `ast.dump` 响应同构——
///   {"ok":bool,"parse_error_count":N,"ast":{...}|null,
///    "parse_errors":[{code,line,column,message},...],"stall_count":N}
/// - 病态样本对齐：两侧崩溃即硬失败（E3 由 Go 驱动断言退出码）
fn main raise {
  let args = @env.args()
  if args.length() < 3 {
    println("用法: dump_ast <corpus_dir|file.c> <out_dir>")
    fail("参数不足")
  }
  let target = args[1]
  let out_dir = args[2]
  if !@fs.path_exists(out_dir) {
    @fs.create_dir(out_dir)
  }
  let files : Array[String] = []
  let target_is_dir = @fs.is_dir(target) catch { _ => false }
  // 自定义 include 的 Vfs 目录级收集一次共享（S2 §7.5 性能教训）
  let shared_vfs : Map[String, String] = if target_is_dir {
    let vfs : Map[String, String] = Map([])
    collect_files(target, vfs)
    vfs
  } else {
    Map([])
  }
  if target_is_dir {
    for name in @fs.read_dir(target) {
      if name.length() > 2 && name.substring(start=name.length() - 2) == ".c" {
        files.push("\{target}/\{name}")
      }
    }
    files.sort()
  } else {
    files.push(target)
  }
  for path in files {
    let src = @fs.read_file_to_string(path)
    let stem = basename_no_ext(path)
    let lexed = if target_is_dir {
      @lexer.tokenize_with_vfs(src, files=shared_vfs)
    } else {
      @lexer.tokenize(src)
    }
    let out = dump_one(lexed)
    @fs.write_string_to_file("\{out_dir}/\{stem}.ast.json", out)
  }
  println("dump_ast: \{files.length()} 个输入 -> \{out_dir}")
}

///|
/// 单输入 dump：词法错误 → ok:false 同构（AST 无从谈起）；解析层带
/// 完整 parse_errors（E2 面——Rust 侧同批补齐，见防线维护提交）。
fn dump_one(lexed : @lexer.LexResult) -> String {
  let buf = StringBuilder::new()
  if !lexed.errors.is_empty() {
    buf.write_string("{\"ok\": false, \"lex_error_count\": ")
    buf.write_string(lexed.errors.length().to_string())
    buf.write_string(", \"ast\": null, \"parse_errors\": [], \"stall_count\": 0}")
    return buf.to_string()
  }
  let r = @parser.parse(lexed.tokens)
  buf.write_string("{\"ok\": ")
  buf.write_string(if r.program is Some(_) { "true" } else { "false" })
  buf.write_string(", \"parse_error_count\": ")
  buf.write_string(r.errors.length().to_string())
  buf.write_string(", \"ast\": ")
  match r.program {
    Some(p) => {
      buf.write_string(@ast.ast_dump_json(p))
    }
    None => buf.write_string("null")
  }
  buf.write_string(", \"parse_errors\": [")
  for i, e in r.errors {
    if i > 0 {
      buf.write_string(", ")
    }
    buf.write_string(
      "{\"code\": \{e.code}, \"line\": \{e.line}, \"column\": \{e.column}, \"message\": \{json_escape(e.message)}}",
    )
  }
  buf.write_string("], \"stall_count\": \{r.stall_count}}")
  buf.to_string()
}

///|
/// JSON 字符串字面量转义（诊断 message 单列——E2 只比 code/line/column，
/// message 为人工确认面）。
fn json_escape(s : String) -> String {
  let buf = StringBuilder::new()
  buf.write_char('\"')
  for c in s {
    match c {
      '\"' => buf.write_string("\\\"")
      '\\' => buf.write_string("\\\\")
      '\n' => buf.write_string("\\n")
      '\r' => buf.write_string("\\r")
      '\t' => buf.write_string("\\t")
      _ => {
        // 控制字符防御性转义（serde_json 口径；诊断 message 中的 \x4
        // 类字节原样写出即非法 JSON）
        let code = c.to_int()
        if code < 32 {
          buf.write_string("\\u00")
          let hex = "0123456789abcdef"
          buf.write_string(hex.substring(start=code / 16, end=code / 16 + 1))
          buf.write_string(hex.substring(start=code % 16, end=code % 16 + 1))
        } else {
          buf.write_char(c)
        }
      }
    }
  }
  buf.write_char('\"')
  buf.to_string()
}

fn basename_no_ext(path : String) -> String {
  let mut name = path
  let mut i = path.length() - 1
  while i >= 0 {
    let slash = '/' .to_int()
    let bslash = 92 // '\'
    let dot = '.' .to_int()
    match path[i].to_int() {
      c if c == slash || c == bslash => {
        name = path.substring(start=i + 1)
        break
      }
      c if c == dot => if name == path { name = path.substring(start=0, end=i) }
      _ => ()
    }
    i -= 1
  }
  name
}

fn collect_files(dir : String, vfs : Map[String, String]) -> Unit raise {
  for name in @fs.read_dir(dir) {
    let full = "\{dir}/\{name}"
    let is_dir = @fs.is_dir(full) catch { _ => false }
    if is_dir {
      collect_files(full, vfs)
    } else {
      let content = @fs.read_file_to_string(full) catch { _ => continue }
      vfs.set(full, content)
    }
  }
}
