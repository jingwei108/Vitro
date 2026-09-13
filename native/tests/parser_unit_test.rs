#![allow(clippy::unwrap_used, clippy::expect_used)]

use vitro_native::compiler::ast::{BinaryOp, Expr, Stmt, TypeKind};
use vitro_native::compiler::lexer::Lexer;
use vitro_native::compiler::parser::Parser;

fn parse(
    src: &str,
) -> (
    Option<vitro_native::compiler::ast::ProgramNode>,
    Vec<vitro_native::compiler::parser::ParseError>,
) {
    let (tokens, _) = Lexer::new(src).tokenize();
    Parser::new(tokens).parse()
}

#[test]
fn test_parser_empty_main() {
    let (program, errors) = parse("int main() { return 0; }");
    assert!(errors.is_empty(), "Parse errors: {:?}", errors);
    let program = program.unwrap();
    assert_eq!(program.funcs.len(), 1);
    assert_eq!(program.funcs[0].name, "main");
    assert_eq!(program.funcs[0].params.len(), 0);
}

#[test]
fn test_parser_function_with_params() {
    let (program, errors) = parse("int add(int a, int b) { return a + b; }");
    assert!(errors.is_empty(), "Parse errors: {:?}", errors);
    let program = program.unwrap();
    assert_eq!(program.funcs[0].params.len(), 2);
    assert_eq!(program.funcs[0].params[0].name, "a");
    assert_eq!(program.funcs[0].params[1].name, "b");
}

#[test]
fn test_parser_variable_decl() {
    let (program, errors) = parse("int main() { int x = 10; return 0; }");
    assert!(errors.is_empty(), "Parse errors: {:?}", errors);
    let program = program.unwrap();
    let body = program.funcs[0].body.as_ref().unwrap();
    if let Stmt::Block { stmts, .. } = body {
        if let Stmt::VarDecl { name, init, .. } = &stmts[0] {
            assert_eq!(name, "x");
            assert!(init.is_some());
        } else {
            panic!("Expected VarDecl");
        }
    } else {
        panic!("Expected Block");
    }
}

#[test]
fn test_parser_struct_decl() {
    let (program, errors) = parse("struct Point { int x; int y; }; int main() { return 0; }");
    assert!(errors.is_empty(), "Parse errors: {:?}", errors);
    let program = program.unwrap();
    assert_eq!(program.structs.len(), 1);
    assert_eq!(program.structs[0].name, "Point");
    assert_eq!(program.structs[0].fields.len(), 2);
}

#[test]
fn test_parser_struct_multi_field_decl() {
    // 修复：struct 体应支持逗号分隔的多字段声明（如 int u, v, w;）
    let (program, errors) = parse("struct Edge { int u, v, w; }; int main() { return 0; }");
    assert!(errors.is_empty(), "Parse errors: {:?}", errors);
    let program = program.unwrap();
    assert_eq!(program.structs.len(), 1);
    assert_eq!(program.structs[0].name, "Edge");
    assert_eq!(program.structs[0].fields.len(), 3);
    assert_eq!(program.structs[0].fields[0].name, "u");
    assert_eq!(program.structs[0].fields[1].name, "v");
    assert_eq!(program.structs[0].fields[2].name, "w");
}

#[test]
fn test_parser_typedef_struct() {
    let (program, errors) = parse("typedef struct { int x; } Point; int main() { Point p; return 0; }");
    assert!(errors.is_empty(), "Parse errors: {:?}", errors);
    let program = program.unwrap();
    assert_eq!(program.structs.len(), 1);
}

#[test]
fn test_parser_typedef_enum_anon() {
    // 修复：typedef enum { A, B } Tag; 应被正确解析
    let (program, errors) = parse("typedef enum { Link, Thread } PointerTag; int main() { return 0; }");
    assert!(errors.is_empty(), "Parse errors: {:?}", errors);
    let program = program.unwrap();
    assert_eq!(program.globals.len(), 2);
    assert_eq!(program.globals[0].name, "Link");
    assert_eq!(program.globals[1].name, "Thread");
}

#[test]
fn test_parser_enum_anon() {
    // 修复：enum { NAME, PARENS }; 匿名枚举声明应被支持
    let (program, errors) = parse("enum { NAME, PARENS, BRACKETS }; int main() { return 0; }");
    assert!(errors.is_empty(), "Parse errors: {:?}", errors);
    let program = program.unwrap();
    assert_eq!(program.globals.len(), 3);
    assert_eq!(program.globals[0].name, "NAME");
    assert_eq!(program.globals[1].name, "PARENS");
    assert_eq!(program.globals[2].name, "BRACKETS");
}

#[test]
fn test_parser_if_else() {
    let (program, errors) = parse("int main() { if (1) { return 1; } else { return 0; } }");
    assert!(errors.is_empty(), "Parse errors: {:?}", errors);
    let program = program.unwrap();
    let body = program.funcs[0].body.as_ref().unwrap();
    if let Stmt::Block { stmts, .. } = body {
        if let Stmt::If {
            cond, then_stmt: _, else_stmt, ..
        } = &stmts[0]
        {
            assert!(matches!(cond, Expr::Literal { value: 1, .. }));
            assert!(else_stmt.is_some());
        } else {
            panic!("Expected If statement");
        }
    } else {
        panic!("Expected Block");
    }
}

#[test]
fn test_parser_binary_expr() {
    let (program, errors) = parse("int main() { return 1 + 2 * 3; }");
    assert!(errors.is_empty(), "Parse errors: {:?}", errors);
    let program = program.unwrap();
    let body = program.funcs[0].body.as_ref().unwrap();
    if let Stmt::Block { stmts, .. } = body {
        if let Stmt::Return { value: Some(ret), .. } = &stmts[0] {
            if let Expr::Binary { op, .. } = ret {
                assert_eq!(*op, BinaryOp::Add);
            } else {
                panic!("Expected binary expression");
            }
        } else {
            panic!("Expected return statement");
        }
    } else {
        panic!("Expected Block");
    }
}

#[test]
fn test_parser_array_decl() {
    let (program, errors) = parse("int main() { int arr[5]; return 0; }");
    assert!(errors.is_empty(), "Parse errors: {:?}", errors);
    let program = program.unwrap();
    let body = program.funcs[0].body.as_ref().unwrap();
    if let Stmt::Block { stmts, .. } = body {
        if let Stmt::VarDecl { name, var_type: ty, .. } = &stmts[0] {
            assert_eq!(name, "arr");
            assert!(ty.is_array());
        } else {
            panic!("Expected VarDecl");
        }
    } else {
        panic!("Expected Block");
    }
}

#[test]
fn test_parser_pointer_decl() {
    let (program, errors) = parse("int main() { int *p; return 0; }");
    assert!(errors.is_empty(), "Parse errors: {:?}", errors);
    let program = program.unwrap();
    let body = program.funcs[0].body.as_ref().unwrap();
    if let Stmt::Block { stmts, .. } = body {
        if let Stmt::VarDecl { name, var_type: ty, .. } = &stmts[0] {
            assert_eq!(name, "p");
            assert!(ty.is_pointer());
        } else {
            panic!("Expected VarDecl");
        }
    } else {
        panic!("Expected Block");
    }
}

#[test]
fn test_parser_function_pointer_cast_type() {
    // 修复：cast 表达式中的函数指针抽象声明符（如 int (*)(void *, void *)）
    // 此前 parse_type_only 只能解析基础类型 + 星号，无法处理函数指针。
    let src = r#"
int numcmp(char *s1, char *s2);
int main() {
    int (*fp)(void *, void *) = (int (*)(void *, void *))numcmp;
    return 0;
}
"#;
    let (program, errors) = parse(src);
    assert!(errors.is_empty(), "Parse errors: {:?}", errors);
    let program = program.unwrap();
    let body = program.funcs[1].body.as_ref().unwrap();
    if let Stmt::Block { stmts, .. } = body {
        if let Stmt::VarDecl { init: Some(init_expr), .. } = &stmts[0] {
            if let Expr::Cast { target_type, .. } = init_expr {
                assert!(
                    target_type.is_function_pointer(),
                    "Expected function pointer type, got {:?}",
                    target_type
                );
            } else {
                panic!("Expected Cast expression, got {:?}", init_expr);
            }
        } else {
            panic!("Expected VarDecl with init");
        }
    } else {
        panic!("Expected Block");
    }
}

#[test]
fn test_parser_error_undeclared_no_panic() {
    // Parser should handle missing semicolons gracefully
    let (_, errors) = parse("int main() { int x return 0; }");
    // This will have parse errors but should not panic
    assert!(!errors.is_empty()); // should have parse errors but not panic
}

#[test]
fn test_parser_asm_statement() {
    // 回归测试：GCC 风格内联汇编占位语法 __asm__("...")
    let (program, errors) = parse("int main() { __asm__(\"nop\"); return 0; }");
    assert!(errors.is_empty(), "Parse errors: {:?}", errors);
    let program = program.unwrap();
    let body = program.funcs[0].body.as_ref().unwrap();
    if let Stmt::Block { stmts, .. } = body {
        assert_eq!(stmts.len(), 2, "Expected __asm__ expr-stmt and return");
    } else {
        panic!("Expected Block");
    }
}

#[test]
fn test_parser_static_assert_top_level() {
    // 回归测试：C11 _Static_assert(expr, "msg") 顶层语法消费
    let (program, errors) = parse("_Static_assert(1 == 1, \"ok\"); int main() { return 0; }");
    assert!(errors.is_empty(), "Parse errors: {:?}", errors);
    let program = program.unwrap();
    assert_eq!(program.funcs.len(), 1);
    assert_eq!(program.funcs[0].name, "main");
}

#[test]
fn test_parser_typeof_decl() {
    // 回归测试：GCC typeof(expr) 类型推断语法
    let (program, errors) = parse("int main() { typeof(1) x = 5; return 0; }");
    assert!(errors.is_empty(), "Parse errors: {:?}", errors);
    let program = program.unwrap();
    let body = program.funcs[0].body.as_ref().unwrap();
    if let Stmt::Block { stmts, .. } = body {
        if let Stmt::VarDecl { name, var_type, .. } = &stmts[0] {
            assert_eq!(name, "x");
            // Parser 将 typeof 包装为 Type::Typeof，其 kind() 当前映射为 TypeKind::Auto
            assert!(matches!(var_type.kind(), TypeKind::Auto), "Expected typeof wrapper type");
        } else {
            panic!("Expected VarDecl");
        }
    } else {
        panic!("Expected Block");
    }
}
