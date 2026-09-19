///|
/// S2 差分出口（开发工具，native only）：批量产出 L1/L2 token TSV——
/// 与 Rust `vitro_cli dump-tokens` 逐字节同构，供 Go 差分驱动比对。
///
/// 用法：moon run cmd/dump_tokens -- <corpus_dir|file.c> <out_dir> <raw|pp|both>
fn main raise {
  let args = @env.args()
  if args.length() < 4 {
    println("用法: dump_tokens <corpus_dir|file.c> <out_dir> <raw|pp|both>")
    fail("参数不足")
  }
  let target = args[1]
  let out_dir = args[2]
  let mode = args[3]
  let want_raw = mode == "raw" || mode == "both"
  let want_pp = mode == "pp" || mode == "both"
  if !want_raw && !want_pp {
    fail("模式必须是 raw|pp|both，得到 '\{mode}'")
  }
  if !@fs.path_exists(out_dir) {
    @fs.create_dir(out_dir)
  }
  let files : Array[String] = []
  let target_is_dir = @fs.is_dir(target) catch { _ => false }
  // Vfs 目录级收集**一次**共享（性能修复 2026-09-19：此前 dump_one 对每个
  // 文件都重新 collect_files 整个目录——2400 例语料 O(N²) ≈ 576 万次文件
  // 操作，MoonBit 侧 dump 实测 4m43s vs Rust 2.5s；修复后按目录收集一次）
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
    files.sort() // 确定性顺序（与 Rust 侧 sort 对齐）
  } else {
    files.push(target)
  }
  for path in files {
    let src = @fs.read_file_to_string(path)
    let stem = basename_no_ext(path)
    if want_raw {
      let r = @lexer.tokenize_raw(src)
      @fs.write_string_to_file(
        "\{out_dir}/\{stem}.l1.tsv",
        @lexer.dump_raw_tsv(r),
      )
    }
    if want_pp {
      // 自定义 include：语料目录文件注入 Vfs（对齐 Rust 真实 fs 语义——
      // quote 候选链在"源文件目录"可见同名头文件）。目录模式用共享收集；
      // 单文件模式现场收集其所在目录
      let vfs = if target_is_dir {
        shared_vfs
      } else {
        let v : Map[String, String] = Map([])
        collect_files(dirname_of(path), v)
        v
      }
      let r = @lexer.tokenize_with_vfs(
        src,
        files=vfs,
        base_dir=dirname_of(path),
      )
      @fs.write_string_to_file(
        "\{out_dir}/\{stem}.l2.tsv",
        @lexer.dump_lex_tsv(r),
      )
    }
  }
  println("dump-tokens: \{files.length()} 个 .c 文件 -> \{out_dir}")
}

///|
/// 路径的目录部分（无分隔符返回 "."）。
fn dirname_of(path : String) -> String {
  let mut i = path.length() - 1
  while i >= 0 {
    let c = path[i].to_int()
    if c == '/' || c == '\\' {
      return path.substring(start=0, end=i)
    }
    i -= 1
  }
  "."
}

///|
fn basename_no_ext(path : String) -> String {
  let mut name = path
  let mut i = path.length() - 1
  while i >= 0 {
    match path[i].to_int() {
      c if c == '/' || c == '\\' => {
        name = path.substring(start=i + 1)
        break
      }
      c if c == '.' => if name == path { name = path.substring(start=0, end=i) }
      _ => ()
    }
    i -= 1
  }
  let mut dot : Int? = None
  let mut k = name.length() - 1
  while k >= 0 {
    if name[k] == '.' {
      dot = Some(k)
      break
    }
    k -= 1
  }
  match dot {
    Some(d) if d > 0 => name.substring(start=0, end=d)
    _ => name
  }
}

///|
/// 递归收集目录下文本文件（含子目录——e2_has_include_nest 形态；不可读项
/// 跳过）。
fn collect_files(dir : String, vfs : Map[String, String]) -> Unit raise {
  for name in @fs.read_dir(dir) {
    let full = "\{dir}/\{name}"
    let is_dir = @fs.is_dir(full) catch { _ => false }
    if is_dir {
      collect_files(full, vfs)
    } else {
      let content = @fs.read_file_to_string(full) catch { _ => continue }
      vfs[full] = content
    }
  }
}
