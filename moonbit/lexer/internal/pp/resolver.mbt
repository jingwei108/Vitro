///|
/// include 嵌套深度上限（U1#11）：静态环检测 DFS 的深度封顶与动态拼接的
/// 嵌套深度保险丝**共用**此值（两个口径分叉会重演"保险丝口径错"失效形态）。
/// 量级对齐 clang `-fmax-include-depth=200` 的教学压缩。
const MAX_INCLUDE_DEPTH : Int = 64

///|
/// 静态环检测 DFS 的访问节点数封顶（U1#11）：防病态依赖图拖垮编译；
/// 覆盖教学项目级头文件群（20 文件环实测检出）。
const MAX_INCLUDE_GRAPH_NODES : Int = 512

///|
/// quote-include 候选链：当前文件目录（include 栈顶）→ 源文件目录（base_dir）。
/// 路径规范化为 `/` 分隔（VFS 语义纯字符串；差分驱动喂数统一正斜杠）。
fn[P : @token.SourceProvider] Pp::resolve_path(
  self : Pp[P],
  path : String,
) -> String? {
  let cur_dir : String = if self.include_stack.length() > 0 {
    self.include_stack[self.include_stack.length() - 1].dir
  } else {
    self.base_dir
  }
  if cur_dir != "" {
    let full = normalize_path(cur_dir + "/" + path)
    match self.try_read(full) {
      Some(_) => return Some(full)
      None => ()
    }
  }
  if self.base_dir != "" {
    let full = normalize_path(self.base_dir + "/" + path)
    match self.try_read(full) {
      Some(_) => return Some(full)
      None => ()
    }
  }
  None
}

///|
/// 试读（NotFound/UnsupportedHost 均为未命中；内容进缓存）。
fn[P : @token.SourceProvider] Pp::try_read(
  self : Pp[P],
  path : String,
) -> String? {
  if self.content_cache.contains(path) {
    return self.content_cache.get(path)
  }
  let content = self.provider.read(path) catch { _ => return None }
  self.content_cache[path] = content
  Some(content)
}

///|
/// 缓存读取（include 实际拼接用——与环检测/候选链共用一次读取）。
fn[P : @token.SourceProvider] Pp::read_cached(
  self : Pp[P],
  path : String,
) -> String? {
  self.try_read(path)
}

///|
/// 路径规范化：折叠 `//`、`.`；`..` 回退一层（超出根不回退——VFS 近似）。
fn normalize_path(p : String) -> String {
  let parts : Array[String] = []
  for seg in p.split("/") {
    match seg {
      "" | "." => ()
      ".." => if parts.length() > 0 { ignore(parts.pop()) }
      _ => parts.push(seg.to_string())
    }
  }
  parts.join("/")
}

///|
/// 静态依赖环检测：从 `start_key`（规范路径）出发沿自定义头文件的
/// `#include "..."` 边 DFS；回到起点即返回环描述 `a.h -> b.h -> a.h`。
/// **visited 路径局部**（勘察 M2 修复：Rust 的 visited 从不移除，菱形依赖下
/// 漏报环——`visited.insert` 后在返回前撤销）。深度/规模双封顶。
fn[P : @token.SourceProvider] Pp::detect_cycle(
  self : Pp[P],
  start_key : String,
) -> String? {
  let visited : Array[String] = []
  self.walk_cycle(start_key, start_key, visited, 0)
}

///|
fn[P : @token.SourceProvider] Pp::walk_cycle(
  self : Pp[P],
  full : String,
  start_key : String,
  visited : Array[String],
  depth : Int,
) -> String? {
  // S2 审阅 P3 登记：封顶时返回 None（= 判"无环"）是 Rust walk_cycle 的
  // 复刻口径（对拍一致）；超限无诊断属两侧共同现状，随 S8 差异台账统一裁定
  if depth > MAX_INCLUDE_DEPTH || visited.length() > MAX_INCLUDE_GRAPH_NODES {
    return None
  }
  if full == start_key && depth > 0 {
    return Some("") // 回到起点（环闭合）——由调用链拼描述
  }
  if visited.contains(full) {
    return None
  }
  visited.push(full) // 路径局部：返回前撤销（修复 M2 菱形漏报）
  let text = match self.try_read(full) {
    Some(t) => t
    None => {
      ignore(visited.pop())
      return None
    }
  }
  let dir = dirname_of(full)
  let mut result : String? = None
  for dep in custom_includes_of(text) {
    // 相对"当前头文件目录"解析（与 quote-include 候选链一致），未命中跳过
    let dep_full : String? = if dir != "" {
      Some(normalize_path(dir + "/" + dep))
    } else {
      None
    }
    let dep_resolved = match dep_full {
      Some(f) =>
        if self.try_read(f) is Some(_) {
          Some(f)
        } else {
          self.resolve_path(dep)
        }
      None => self.resolve_path(dep)
    }
    match dep_resolved {
      Some(f) =>
        match self.walk_cycle(f, start_key, visited, depth + 1) {
          Some(rest) => {
            // 环闭合沿调用链向上拼接描述（路径分隔符跨平台，不做启发过滤）
            let desc = if rest == "" { dep } else { "\{dep} -> \{rest}" }
            result = Some(desc)
            break
          }
          None => ()
        }
      None => ()
    }
  }
  ignore(visited.pop())
  result
}

///|
/// 提取头文件文本中的自定义 `#include "..."` 依赖（`<...>` 存根不算依赖边）。
fn custom_includes_of(text : String) -> Array[String] {
  let out : Array[String] = []
  for line_raw in text.split("\n") {
    let t = leading_trim(line_raw.to_string())
    if t.length() == 0 || t[0].to_int() != '#' {
      continue
    }
    let rest = leading_trim(t.substring(start=1))
    if !rest_starts_with_include(rest) {
      continue
    }
    let inc_after = leading_trim(rest.substring(start=7))
    if inc_after.length() > 0 && inc_after[0].to_int() == '"' {
      let mut i = 1
      while i < inc_after.length() {
        if inc_after[i].to_int() == '"' {
          out.push(inc_after.substring(start=1, end=i))
          break
        }
        i += 1
      }
    }
  }
  out
}

///|
fn rest_starts_with_include(rest : String) -> Bool {
  rest.length() >= 7 && rest.substring(start=0, end=7) == "include"
}
