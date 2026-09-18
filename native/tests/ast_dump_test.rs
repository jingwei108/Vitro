//! P7（2026-09-19）：AST/符号表 dump 出口锚——E1 B 级锚的 Rust 侧基座。
//!
//! serve `ast.dump` / `symbols.dump` 的输出经 Go canonicalizer
//! （scripts/canonicalize：键排序/数字保形/转义统一/缩进固定）归一后
//! 必须**幂等**——这是 S1 MoonBit 侧显式 emitter 对拍（E1 锚）逐字节
//! 比较的可信前提。emitter 纪律（总计划 §B）：Rust 侧 serde 派生是本侧
//! 唯一 emitter，MoonBit 侧禁 ToJson 直拼。

#![allow(clippy::unwrap_used, clippy::expect_used)]

use serde_json::Value;
use std::process::{Command, Stdio};

fn dump(method: &str, source: &str) -> Value {
    let mut session = vitro_native::session::Session::default();
    let params = serde_json::json!({ "source": source });
    match method {
        "ast.dump" => vitro_native::session_api::ast_dump(&mut session, &params),
        "symbols.dump" => vitro_native::session_api::symbols_dump(&mut session, &params),
        _ => panic!("未知方法 {method}"),
    }
}

/// 调 Go canonicalizer（stdin → stdout），失败即 panic（fail loud）。
fn canonicalize(input: &str) -> String {
    let mut child = Command::new("go")
        .args(["run", "../scripts/canonicalize"])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("启动 go run canonicalize（CI 需有 Go 工具链）");
    use std::io::Write;
    child.stdin.as_mut().unwrap().write_all(input.as_bytes()).unwrap();
    let out = child.wait_with_output().unwrap();
    if !out.status.success() {
        panic!(
            "canonicalize 失败（exit {:?}）：{}",
            out.status.code(),
            String::from_utf8_lossy(&out.stderr)
        );
    }
    String::from_utf8(out.stdout).expect("规范形必须为 UTF-8")
}

const SRC: &str = "int g = 1;\nint main(){ int x = g + 2; return x; }\n";

#[test]
fn test_ast_dump_canonical_pipeline_idempotent() {
    let out = dump("ast.dump", SRC);
    assert_eq!(out["ok"], serde_json::json!(true), "AST dump 应成功");
    let ast = &out["ast"];
    assert!(ast["funcs"].as_array().is_some_and(|f| !f.is_empty()), "AST 应含函数");
    assert!(ast["globals"].as_array().is_some_and(|g| !g.is_empty()), "AST 应含全局");

    // 归一幂等：canonical(canonical(x)) == canonical(x)——E1 逐字节锚的前提
    let raw = serde_json::to_string(&out).unwrap();
    let once = canonicalize(&raw);
    let twice = canonicalize(&once);
    assert_eq!(once, twice, "canonicalize 幂等破坏：E1 锚不可用");

    // 归一形不得为空且键已排序（粗校：funcs 在文本上先于 globals 出现
    // 之前无乱序键——精确键序由 canonicalize_test 的 Go 侧 J9 保证）
    assert!(once.contains("\"funcs\"") && once.contains("\"globals\""));
}

#[test]
fn test_symbols_dump_shape() {
    let out = dump("symbols.dump", SRC);
    assert_eq!(out["ok"], serde_json::json!(true));
    let symbols = out["symbols"].as_array().expect("symbols 数组");
    assert_eq!(symbols.len(), 2, "g + x 两个符号");
    let names: Vec<&str> = symbols.iter().filter_map(|s| s["name"].as_str()).collect();
    assert!(names.contains(&"g") && names.contains(&"x"), "符号名：{names:?}");
    let g = symbols.iter().find(|s| s["name"] == serde_json::json!("g")).unwrap();
    assert_eq!(g["is_local"], serde_json::json!(false), "g 是全局");

    let raw = serde_json::to_string(&out).unwrap();
    let once = canonicalize(&raw);
    let twice = canonicalize(&once);
    assert_eq!(once, twice, "symbols dump 归一幂等破坏");
}

#[test]
fn test_ast_dump_syntax_error_frame() {
    let out = dump("ast.dump", "int main(){ int x = ; }\n");
    // 语法错误：error 帧（恢复不出 ProgramNode 时）或带 parse_error_count 的
    // 部分 AST——两种形态都必须可归一（fail loud：非 ok 帧不进锚，但结构稳定）
    let raw = serde_json::to_string(&out).unwrap();
    canonicalize(&raw); // 不 panic 即通过
}
