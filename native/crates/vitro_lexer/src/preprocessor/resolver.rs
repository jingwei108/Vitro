//! E2：include 解析器——ModuleGraph 语义（include-once + 依赖环检测）。
//!
//! 表面（标准 C）：`#include "list.h"`；内核：头文件内容拼接前先过本模块的
//! 判定——
//! - **include-once**：同一文件（含标准库存根）只拼接一次（守卫语义内置，
//!   写不写 `#ifndef` 守卫都正确；无守卫双 include 与 Clang 的差异已入 spec
//!   差异清单——Clang 会重定义报错，Vitro 静默跳过）；
//! - **环检测**：自定义头文件的 `#include "..."` 依赖图做静态 DFS（文本级，
//!   深度/规模封顶），A↔B 互包含在拼接前即报 `E1015` 并跳过该 include
//!   （fail-soft 带诊断，不静默吞）；
//! - 标准库存根（`runtime_libc/include/*.h`）视为叶子节点。

use std::collections::HashSet;
use std::path::PathBuf;

/// include 嵌套深度上限（U1#11）：静态环检测 DFS 的深度封顶与动态拼接的
/// 嵌套深度保险丝**共用**此值——两个口径分叉会重演"保险丝口径错"失效形态
///（DFS 封顶小于动态保险丝时无环深链会被误判为环，反之则漏拦）。
/// 量级对齐 clang `-fmax-include-depth=200` 的教学压缩（教学子集取 64）。
pub const MAX_INCLUDE_DEPTH: usize = 64;

/// 静态环检测 DFS 的访问节点数封顶（U1#11）：防病态依赖图拖垮编译；
/// 提高自 64 以覆盖教学项目级的头文件群（20 文件环实测检出）。
pub const MAX_INCLUDE_GRAPH_NODES: usize = 512;

/// include 解析状态（随单次 tokenize 存活）。
#[derive(Debug, Default)]
pub struct IncludeResolver {
    /// 已处理过的 include key（`stub:<name>` 或自定义头文件规范路径）——include-once。
    pub processed: HashSet<String>,
    /// 源文件所在目录（`#include "..."` 的解析根）。
    pub base_path: Option<PathBuf>,
    /// 当前文件目录栈（C quote-include 语义：优先"包含者所在目录"）。
    /// 哨兵指令 `#__vitro_push_dir` / `#__vitro_pop_dir` 在线性扫描中精确维护。
    pub dir_stack: Vec<PathBuf>,
}

impl IncludeResolver {
    pub fn new(base_path: Option<PathBuf>) -> Self {
        Self { processed: HashSet::new(), base_path, dir_stack: Vec::new() }
    }

    /// 当前"包含者目录"：目录栈顶；空栈时为源文件目录。
    fn current_dir(&self) -> Option<&PathBuf> {
        self.dir_stack.last().or(self.base_path.as_ref())
    }

    /// quote-include 候选链：当前文件目录 → 源文件目录。
    pub fn resolve_path(&self, path: &str) -> Option<PathBuf> {
        if let Some(dir) = self.current_dir() {
            let full = dir.join(path);
            if full.exists() {
                return Some(full);
            }
        }
        let full = self.base_path.as_ref()?.join(path);
        if full.exists() {
            return Some(full);
        }
        None
    }

    /// include key：存根用固定前缀（内容固定），自定义头文件用规范路径。
    fn key_for(&self, path: &str, is_stub: bool) -> Option<String> {
        if is_stub {
            return Some(format!("stub:{}", path));
        }
        let full = self.resolve_path(path)?;
        match full.canonicalize() {
            Ok(p) => Some(p.to_string_lossy().to_string()),
            Err(_) => None,
        }
    }

    /// 判定一次 include 是否应当拼接内容。
    ///
    /// 返回 `Ok(())` = 拼接；`Err(Some(环描述))` = 环检测命中（报 E1015 后跳过）；
    /// `Err(None)` = include-once 命中（静默跳过）。
    ///
    /// `key_for` 失败（文件不存在等）时仍放行（`Ok(())`）——fail-closed 由调用方
    /// `handle_include` 的存在性诊断承担（U1#11 H-1 修复前是静默 `return`，
    /// 现在报 E1021，本函数无需重复报错路径）。
    pub fn should_include(&mut self, path: &str, is_stub: bool) -> Result<(), Option<String>> {
        let Some(key) = self.key_for(path, is_stub) else {
            return Ok(());
        };
        if self.processed.contains(&key) {
            return Err(None);
        }
        if !is_stub {
            if let Some(cycle) = self.detect_cycle_from_key(&key) {
                self.processed.insert(key);
                return Err(Some(cycle));
            }
        }
        self.processed.insert(key);
        Ok(())
    }

    /// `__has_include(<p>)` / `__has_include("p")`（U1#11 H-2/H-3 修复）：
    /// 与 `#include` 的解析口径**单源**——
    /// - `<p>`：只查标准库存根（与收紧后的 `#include <p>` 对齐，不搜文件系统）；
    /// - `"p"`：存根优先，未命中走 quote 候选链（当前文件目录 → 源文件目录，
    ///   即 [`Self::resolve_path`]）。修复前只查 `base_path`，头文件内部与紧随
    ///   其后的 `#include` 判定互相矛盾（T14 实锤）。
    pub fn has_include(&self, path: &str, is_angle: bool) -> bool {
        if Self::load_stub(path).is_some() {
            return true;
        }
        if is_angle {
            return false;
        }
        self.resolve_path(path).is_some()
    }

    /// 静态依赖环检测：从已解析的 `start_key`（规范路径）出发，沿自定义头文件
    /// 的 `#include "..."` 边 DFS；能回到 `start_key` 即返回环描述
    /// （`a.h -> b.h -> a.h`）。访问集 + 深度双重封顶。
    fn detect_cycle_from_key(&self, start_key: &str) -> Option<String> {
        let start = PathBuf::from(start_key);
        let mut visited: HashSet<String> = HashSet::new();
        self.walk_cycle(&start, start_key, &mut visited, 0)
    }

    fn walk_cycle(
        &self,
        full: &PathBuf,
        start_key: &str,
        visited: &mut HashSet<String>,
        depth: usize,
    ) -> Option<String> {
        // U1#11：封顶 16/64 → MAX_INCLUDE_DEPTH/MAX_INCLUDE_GRAPH_NODES——旧封顶下
        // 20 文件环在 depth=17 处静默截断、零诊断（环被 include-once 断链兜住，
        // 学生看到的只是头文件内容没生效）。
        if depth > MAX_INCLUDE_DEPTH || visited.len() > MAX_INCLUDE_GRAPH_NODES {
            return None;
        }
        let key = full.to_string_lossy().to_string();
        if key == start_key && depth > 0 {
            return Some(String::new()); // 回到起点（环闭合）
        }
        if !visited.insert(key) {
            return None;
        }
        let text = std::fs::read_to_string(full).ok()?;
        let dir = full.parent().map(|p| p.to_path_buf());
        for dep in custom_includes_of(&text) {
            // 相对"当前头文件目录"解析（与 quote-include 候选链一致）
            let dep_full = match dir.as_ref().map(|d| d.join(&dep)) {
                Some(f) if f.exists() => f,
                _ => match self.base_path.as_ref().map(|b| b.join(&dep)) {
                    Some(f) if f.exists() => f,
                    _ => continue,
                },
            };
            // 环闭合沿调用链向上拼接描述（路径分隔符跨平台，不做启发过滤）
            if let Some(rest) = self.walk_cycle(&dep_full, start_key, visited, depth + 1) {
                if rest.is_empty() {
                    return Some(dep.clone());
                }
                return Some(format!("{} -> {}", dep, rest));
            }
        }
        None
    }

    /// 标准库头文件存根（唯一真相：runtime_libc/include/*.h）。
    pub fn load_stub(path: &str) -> Option<&'static str> {
        match path {
            "stdio.h" => Some(include_str!("../../../../runtime_libc/include/stdio.h")),
            "stdlib.h" => Some(include_str!("../../../../runtime_libc/include/stdlib.h")),
            "ctype.h" => Some(include_str!("../../../../runtime_libc/include/ctype.h")),
            "math.h" => Some(include_str!("../../../../runtime_libc/include/math.h")),
            "string.h" => Some(include_str!("../../../../runtime_libc/include/string.h")),
            "stdarg.h" => Some(include_str!("../../../../runtime_libc/include/stdarg.h")),
            "limits.h" => Some(include_str!("../../../../runtime_libc/include/limits.h")),
            "stdbool.h" => Some(include_str!("../../../../runtime_libc/include/stdbool.h")),
            "stddef.h" => Some(include_str!("../../../../runtime_libc/include/stddef.h")),
            "stdint.h" => Some(include_str!("../../../../runtime_libc/include/stdint.h")),
            "time.h" => Some(include_str!("../../../../runtime_libc/include/time.h")),
            "assert.h" => Some(include_str!("../../../../runtime_libc/include/assert.h")),
            "errno.h" => Some(include_str!("../../../../runtime_libc/include/errno.h")),
            "float.h" => Some(include_str!("../../../../runtime_libc/include/float.h")),
            _ => None,
        }
    }
}

/// 提取头文件文本中的自定义 `#include "..."` 依赖（`<...>` 存根不算依赖边）。
fn custom_includes_of(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    for line in text.lines() {
        let t = line.trim_start();
        if !t.starts_with('#') {
            continue;
        }
        let rest = t[1..].trim_start();
        if !rest.starts_with("include") {
            continue;
        }
        let after = rest["include".len()..].trim_start();
        if let Some(p) = after.strip_prefix('"').and_then(|r| r.split('"').next()) {
            out.push(p.to_string());
        }
    }
    out
}
