#![allow(clippy::unwrap_used, clippy::expect_used)]

//! C++ 指针赋值的方向语义（2026-09-11 PR 清单 P1-6）。
//!
//! C++ 允许**向上转型**（`Derived* → Base*`）隐式发生：它是多态的基础写法，
//! Clang++ 在 `-Wall -Wextra` 下对此零警告。Vitro 此前把它报成
//! "不兼容的指针类型赋值：Base* ← Derived*" 并建议"隐式类型转换可能导致数据截断"
//!（复用了标量转换码 W3053），属教学误导。
//!
//! 本测试走 `.cpp` 完整管线（C++ 模式由扩展名决定），断言：
//! 1. 向上转型不产生指针类型诊断；
//! 2. 向下转型（`Base* → Derived*`）仍提示需要显式转换（C++ 隐式不允许）；
//! 3. 无关类型指针（`int* ← double*`）仍提示不兼容。

use vitro_native::session::{CompileUnit, Session};
use vitro_native::session_api;

/// 编译并返回诊断数组（每条含 code / severity / message）。
fn diagnostics_of(source: &str) -> Vec<serde_json::Value> {
    let mut session = Session::default();
    session.compile.compile_units = vec![CompileUnit {
        filename: "main.cpp".to_string(),
        source: source.to_string(),
    }];
    let result = session_api::compile(&mut session);
    result["diagnostics"].as_array().cloned().unwrap_or_default()
}

fn has_pointer_mismatch(diags: &[serde_json::Value]) -> bool {
    diags
        .iter()
        .any(|d| d["message"].as_str().unwrap_or("").contains("指针类型不兼容"))
}

const UPCAST_SRC: &str = r#"class Base {
public:
    virtual ~Base() {}
};
class Derived : public Base {
public:
    int v;
};
int main() {
    Base* b = new Derived();
    (void)b;
    return 0;
}
"#;

#[test]
fn test_upcast_pointer_assignment_has_no_pointer_diagnostic() {
    let diags = diagnostics_of(UPCAST_SRC);
    assert!(
        !has_pointer_mismatch(&diags),
        "向上转型（Derived* → Base*）不应报指针类型不兼容：{diags:?}"
    );
    // 也不得出现"数据截断"这类标量转换建议（那是 W3053 的文案）
    assert!(
        !diags
            .iter()
            .any(|d| d["message"].as_str().unwrap_or("").contains("截断")),
        "向上转型不应报数据截断：{diags:?}"
    );
}

#[test]
fn test_downcast_pointer_assignment_still_reports() {
    let src = r#"class Base {
public:
    virtual ~Base() {}
};
class Derived : public Base {
public:
    int v;
};
int main() {
    Base* b = new Derived();
    Derived* d = b;
    (void)d;
    return 0;
}
"#;
    let diags = diagnostics_of(src);
    assert!(
        has_pointer_mismatch(&diags),
        "向下转型（Base* → Derived*）C++ 不允许隐式转换，应提示需要显式转换：{diags:?}"
    );
}

#[test]
fn test_unrelated_pointer_types_still_report_incompatible() {
    // 无关类型指针仍应报不兼容（不再是"数据截断"的文案）
    let src = r#"int main() {
    int x = 0;
    double* p = (double*)&x;
    int* q = p;
    (void)q;
    return 0;
}
"#;
    let diags = diagnostics_of(src);
    assert!(
        has_pointer_mismatch(&diags),
        "无关类型指针赋值应报指针类型不兼容：{diags:?}"
    );
    assert!(
        !diags
            .iter()
            .any(|d| d["message"].as_str().unwrap_or("").contains("截断")),
        "指针不兼容不应再复用'数据截断'文案：{diags:?}"
    );
}
