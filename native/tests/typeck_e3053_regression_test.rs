#![allow(clippy::unwrap_used, clippy::expect_used)]

//! W0-4 / R-2026-09-12 回归锚：char 初始化器的 E3053（W3053）误报修复。
//!
//! 红留痕（2026-09-13 实测）：`char s[5]={72,101,...}` + `char c='A'` +
//! `char t[]={'x','y',0}` 共 **9 条**"被隐式转换为 char，可能会丢失精度"轰炸
//! 完全合法的代码（clang 零警告）——`'A'` 在 C 语义中是 int，char 特征在
//! `resolve_literal` 提升时丢失，`check_scalar_assignable` 只见 Int→Char。
//!
//! 修复（本批次）：① 初始化路径的字符常量 / char 值域内整常量豁免
//! （`is_char_safe_initializer` + `char_narrow_suppress` 成对置位）；
//! ② `report_warning` 对 W3053 做（行, 码）去重，列表轰炸最多 1 条。
//! 真实截断（超值域常量、int 变量赋值）**仍必须报警**——豁免不得过度。

use vitro_native::compiler::lexer::Lexer;
use vitro_native::compiler::parser::Parser;
use vitro_native::compiler::typeck::TypeChecker;

const W3053: i32 = 3053;

fn typeck_warnings(source: &str) -> Vec<i32> {
    let (tokens, lex_errors) = Lexer::new(source).tokenize();
    assert!(lex_errors.is_empty(), "词法错误：{:?}", lex_errors);
    let (maybe_program, parse_errors) = Parser::new(tokens).parse();
    assert!(parse_errors.is_empty(), "语法错误：{:?}", parse_errors);
    let mut program = maybe_program.expect("program");
    let (type_errors, warnings, _hints) = TypeChecker::default().check(&mut program);
    assert!(type_errors.is_empty(), "不应有类型错误：{:?}", type_errors);
    warnings.iter().map(|w| w.code).collect()
}

#[test]
fn char_constant_and_in_range_init_is_silent() {
    // 修复前：9 条 W3053（5 + 1 + 3）轰炸；修复后：零警告
    let src = r#"
int main() {
    char s[5] = {72, 101, 108, 108, 111};
    char c = 'A';
    char t[] = {'x', 'y', 0};
    return 0;
}
"#;
    let codes = typeck_warnings(src);
    let count = codes.iter().filter(|&&c| c == W3053).count();
    assert_eq!(count, 0, "合法 char 初始化器不得触发 W3053（实际警告码：{:?}）", codes);
}

#[test]
fn real_truncation_still_warns() {
    // 超值域常量（300 截断）与 int 变量赋值（运行时值未知）必须仍报
    let src = r#"
int main() {
    int i = 65;
    char c = 300;
    char d = i;
    return 0;
}
"#;
    let codes = typeck_warnings(src);
    let count = codes.iter().filter(|&&c| c == W3053).count();
    assert_eq!(count, 2, "真实截断路径应各报一条，得到 {}", count);
}

#[test]
fn same_line_list_bombing_deduped_to_one() {
    // 3 个超域常量同列表同行：去重后恰好 1 条（修复前 3 条）
    let src = r#"
int main() {
    char a[3] = {300, 400, 500};
    return 0;
}
"#;
    let codes = typeck_warnings(src);
    let count = codes.iter().filter(|&&c| c == W3053).count();
    assert_eq!(count, 1, "同行同码应去重为 1 条，得到 {}", count);
}

#[test]
fn assignment_path_not_suppressed() {
    // 豁免只作用于初始化器；普通赋值语句 `c = i;` 不豁免
    let src = r#"
int main() {
    int i = 300;
    char c;
    c = i;
    return 0;
}
"#;
    let codes = typeck_warnings(src);
    let count = codes.iter().filter(|&&c| c == W3053).count();
    assert_eq!(count, 1, "赋值路径的 char 窄化仍应报警，得到 {}", count);
}
