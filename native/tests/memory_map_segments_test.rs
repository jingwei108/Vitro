#![allow(clippy::unwrap_used, clippy::expect_used)]

//! 内存地图三段式（C2）与指针目标名跨帧解析（D3）回归。
//!
//! 两项都来自对端 SharpTutor《C# 扩展期需求清单》（锚定 `10591ad`）：
//!
//! - **C2**：`memory.regions` 的 `regions` 统一为带 `kind` 的**三段式**
//!   （`global` / `stack` / `heap`），并为栈/全局区域补 `name` / `alloc_line`
//!   ——此前只有 heap/vfs region 携带这两个字段，三段式内存地图无法标注
//!   "第 N 行分配"（学生定位 UAF/悬空指针的关键信息）；
//! - **D3**：`pointer_snapshots[].target_name` 支持**跨帧解析**——`swap(int *a, int *b)`
//!   体内 `a` 指向调用者的 `x`，只看当前帧永远解不出名字（S3 §6 观测 #2：`target_addr`
//!   正确而 `target_name == ""`）。

use vitro_native::session::{CompileUnit, Session};
use vitro_native::session_api;

const SWAP_SRC: &str = r#"#include <stdio.h>

int GLOBAL_N = 7;

void swap(int *a, int *b) {
    int temp = *a;
    *a = *b;
    *b = temp;
}

int main() {
    int x = 3;
    int y = 8;
    swap(&x, &y);
    printf("%d %d\n", x, y);
    return 0;
}
"#;

/// 编译并推进到"`swap` 帧已建立"的那一步，返回（session, 该步 payload）。
fn run_until_swap_frame() -> (Session, serde_json::Value) {
    let mut session = Session::default();
    session.compile.compile_units = vec![CompileUnit {
        filename: "main.c".to_string(),
        source: SWAP_SRC.to_string(),
    }];
    let compiled = session_api::compile(&mut session);
    assert!(compiled["ok"].as_bool().unwrap_or(false), "编译失败：{compiled}");
    assert_eq!(session_api::step_begin(&mut session), 0, "step_begin 应成功");

    for _ in 0..2_000 {
        let step = session_api::step_next(&mut session).expect("step_next 不应失败");
        for p in step["payloads"].as_array().cloned().unwrap_or_default() {
            if p["func_name"] == "swap" && !p["pointer_snapshots"].as_array().map(Vec::is_empty).unwrap_or(true) {
                return (session, p);
            }
        }
        if step["finished"].as_bool().unwrap_or(false) || step["trapped"].as_bool().unwrap_or(false) {
            break;
        }
    }
    panic!("未推进到 swap 帧内的指针快照步");
}

/// D3：`target_name` 跨帧解析 —— `swap` 体内的 `a`/`b` 解析出调用者的 `x`/`y`。
#[test]
fn test_pointer_target_name_resolves_across_frames() {
    let (session, p) = run_until_swap_frame();
    let ptrs = p["pointer_snapshots"].as_array().cloned().unwrap_or_default();
    assert!(ptrs.len() >= 2, "swap 体内应有两个指针快照，实际 {ptrs:?}");

    let mut names: Vec<(String, u32, String)> = ptrs
        .iter()
        .map(|pt| {
            (
                pt["name"].as_str().unwrap_or("").to_string(),
                pt["target_addr"].as_u64().unwrap_or(0) as u32,
                pt["target_name"].as_str().unwrap_or("").to_string(),
            )
        })
        .collect();
    names.sort();

    assert_eq!(
        names,
        vec![
            ("a".to_string(), 1048568, "x".to_string()),
            ("b".to_string(), 1048572, "y".to_string()),
        ],
        "跨帧解析失败：swap 体内的 a/b 应解析出调用者 main 的 x/y（此前恒为空串）"
    );

    // 目标地址确实落在 main 帧的栈区域内（与 C2 的三段式对账，S3 的"三视图一致"契约）
    let regions = session_api::memory_regions(&session);
    let stack_regions: Vec<&serde_json::Value> = regions["regions"]
        .as_array()
        .map(|a| a.iter().filter(|r| r["kind"] == "stack").collect())
        .unwrap_or_default();
    assert!(stack_regions.iter().any(|r| r["name"] == "main"), "缺少 main 栈区域：{regions}");
    for (_, addr, _) in &names {
        let covered = stack_regions.iter().any(|r| {
            let base = r["addr"].as_u64().unwrap_or(0) as u32;
            let size = r["size"].as_i64().unwrap_or(0) as u32;
            *addr >= base && *addr < base + size
        });
        assert!(covered, "指针目标地址 {addr} 未落在任何栈区域内：{regions}");
    }
}

/// C2：三段式 `kind` + 栈/全局区域的 `name` / `alloc_line`。
#[test]
fn test_memory_regions_three_segments() {
    let (session, _p) = run_until_swap_frame();
    let regions = session_api::memory_regions(&session);

    let counts = &regions["region_counts"];
    assert!(
        counts["global"].as_u64().unwrap_or(0) >= 1,
        "应至少有一个全局区域（GLOBAL_N）：{regions}"
    );
    assert!(
        counts["stack"].as_u64().unwrap_or(0) >= 2,
        "swap 帧活跃时应有 main + swap 两个栈区域：{regions}"
    );

    let list = regions["regions"].as_array().cloned().unwrap_or_default();

    // 数组按地址升序（内存地图的自然顺序）
    let addrs: Vec<u64> = list.iter().map(|r| r["addr"].as_u64().unwrap_or(0)).collect();
    let mut sorted = addrs.clone();
    sorted.sort_unstable();
    assert_eq!(addrs, sorted, "regions 应按地址升序：{regions}");

    // 全局区域：name = 变量名、alloc_line = 声明行、alloc_by = static
    let g = list
        .iter()
        .find(|r| r["kind"] == "global" && r["name"] == "GLOBAL_N")
        .expect("缺少全局区域 GLOBAL_N");
    assert_eq!(g["ty"], "int", "全局区域应给出 C 风格类型名：{g}");
    assert_eq!(g["alloc_line"], 3, "全局区域 alloc_line 应为声明行（GLOBAL_N 在第 3 行）");
    assert_eq!(g["alloc_by"], "static");
    assert_eq!(g["is_heap"], false);

    // 栈区域：name = 函数名、alloc_line = 进入该帧的调用行（main 为 0）
    let swap_frame = list
        .iter()
        .find(|r| r["kind"] == "stack" && r["name"] == "swap")
        .expect("缺少 swap 栈区域");
    assert_eq!(swap_frame["alloc_line"], 14, "栈帧 alloc_line 应为调用点行（swap(&x, &y);）");
    assert_eq!(swap_frame["alloc_by"], "call");
    assert!(swap_frame["size"].as_i64().unwrap_or(0) > 0, "栈帧应有正的跨度：{swap_frame}");

    let main_frame = list
        .iter()
        .find(|r| r["kind"] == "stack" && r["name"] == "main")
        .expect("缺少 main 栈区域");
    assert_eq!(main_frame["alloc_line"], 0, "main 帧不是被调用出来的，alloc_line 为 0");

    // 堆区域仍带三色语义字段（既有契约不回归）
    for r in list.iter().filter(|r| r["kind"] == "heap") {
        assert!(r["is_freed"].is_boolean(), "堆区域须保留 is_freed：{r}");
        assert!(r["alloc_by"].is_string(), "堆区域须保留 alloc_by：{r}");
        assert_eq!(r["is_heap"], true);
    }
}

/// C2 回归护栏：栈/全局区域**只在导出层合成**，不得写回 `session.memory.regions`。
///
/// 否则 `total_allocated` / 碎片率会把栈帧算成"已分配堆内存"（堆统计口径被污染）。
#[test]
fn test_segments_do_not_pollute_heap_region_list() {
    let (session, _p) = run_until_swap_frame();
    let exported = session_api::memory_regions(&session);
    let exported_kinds: Vec<String> = exported["regions"]
        .as_array()
        .map(|a| a.iter().map(|r| r["kind"].as_str().unwrap_or("").to_string()).collect())
        .unwrap_or_default();
    assert!(exported_kinds.iter().any(|k| k == "stack"), "导出层应含栈区域");

    let heap_only = session.memory.regions.iter().all(|r| r.is_heap);
    assert!(heap_only, "session.memory.regions 只应承载堆区域（堆统计单源）");
    assert_eq!(
        session.memory.regions.len(),
        exported_kinds.iter().filter(|k| *k == "heap").count(),
        "导出层 heap 段条数应与内部 regions 一致"
    );
}
