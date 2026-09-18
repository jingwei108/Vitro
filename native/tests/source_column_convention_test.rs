//! P6 列号口径冻结的防漂移锚（2026-09-19）：§1 探针矩阵的精确断言。
//!
//! 口径档案：docs/current/07-质量与裁定/列号口径冻结.md。缺陷**不修**
//! （S1 vitro/source 双坐标根治）；本锚冻结**现状行为**——词法/解析
//! 重构无意触碰 column 语义时立即红。MoonBit 侧 E3 锚对拍时，本矩阵
//! 中 −4/+1 两类形状列入已登记差异，不判缺陷。

use serde_json::Value;

/// 编译源码，返回首条诊断的 (line, column, code 串)。
fn first_diag(source: &str) -> (i64, i64, String) {
    let mut session = vitro_native::session::Session::default();
    let params = serde_json::json!({ "source": source });
    let out: Value = vitro_native::session_api::diagnostics_probe(&mut session, &params);
    let d = out["diagnostics"].as_array().and_then(|a| a.first()).unwrap_or_else(|| {
        panic!("期望至少一条诊断，实际无：{source:?}")
    });
    (
        d["line"].as_i64().unwrap_or(-1),
        d["column"].as_i64().unwrap_or(-1),
        d["code"].as_str().unwrap_or("").to_string(),
    )
}

#[test]
fn test_lex_path_column_convention() {
    // 词法路径：非法字符的 1-based 字符列 + 1（advance 后报错）；字符列语义
    let cases: &[(&str, &str, i64, i64)] = &[
        ("int @;", "E1001", 1, 6),
        ("int a = @;", "E1001", 1, 10),
        // 中按 1 字符计（字符列而非字节列）；@ 字符列 13
        ("int a = \"中\" @;", "E1001", 1, 14),
        // 中/文各按 1 字符计，逐字符报错
        ("int main(){ 中文 x; }", "E1001", 1, 14),
    ];
    for (src, code, line, col) in cases {
        let (l, c, k) = first_diag(src);
        assert_eq!((l, c, k.as_str()), (*line, *col, *code), "词法探针失配：{src:?}");
    }
}

#[test]
fn test_parse_path_column_convention() {
    // 解析路径：current token 的 column 字段——ASCII token 全对（1-based
    // 字符列），含非 ASCII 的 String token 偏 −4（make_token 字符计数减
    // 字节数的混算 + off-by-one，口径档案 §1.2）
    let cases: &[(&str, &str, i64, i64)] = &[
        ("int main(){ int \"ab\"; }", "E2005", 1, 17),
        ("int main(){ char* \"abc\"; }", "E2005", 1, 19),
        ("int  \"ab\";", "E2005", 1, 6),
        ("int main(){ int \"aaaaaaaaaa\"; }", "E2005", 1, 17),
        // 原始锚（P6 现象）：正确字符列 17，实报 13（−4）
        ("int main(){ int \"中文\"; }", "E2005", 1, 13),
        ("int main(){ int 5x; }", "E2005", 1, 17),
    ];
    for (src, code, line, col) in cases {
        let (l, c, k) = first_diag(src);
        assert_eq!((l, c, k.as_str()), (*line, *col, *code), "解析探针失配：{src:?}");
    }
}
