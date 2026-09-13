#![allow(clippy::unwrap_used, clippy::expect_used)]

use vitro_native::compiler::lexer::Lexer;
use vitro_native::compiler::parser::Parser;
use vitro_native::compiler::typeck::TypeChecker;

fn type_check(
    src: &str,
) -> (
    Vec<vitro_native::compiler::typeck::TypeError>,
    Vec<vitro_native::compiler::typeck::TypeError>,
    Vec<vitro_native::compiler::typeck::TypeError>,
) {
    let (tokens, _) = Lexer::new(src).tokenize();
    let (maybe_program, parse_errors) = Parser::new(tokens).parse();
    assert!(parse_errors.is_empty(), "Parse errors: {:?}", parse_errors);
    let mut program = maybe_program.unwrap();
    TypeChecker::default().check(&mut program)
}

#[test]
fn test_type_checker_no_errors() {
    let (errors, _warnings, _hints) = type_check("int main() { int x = 10; return 0; }");
    assert!(errors.is_empty(), "Type errors: {:?}", errors);
}

#[test]
fn test_type_checker_undeclared_var() {
    let (errors, _, _) = type_check("int main() { return x; }");
    assert!(!errors.is_empty(), "Expected error for undeclared variable");
    assert!(errors[0].message.contains("未声明"), "Expected 'undeclared' message");
}

#[test]
fn test_type_checker_type_mismatch() {
    let (errors, _, _) = type_check("int main() { int x = \"hello\"; return 0; }");
    assert!(!errors.is_empty(), "Expected error for type mismatch");
}

#[test]
fn test_type_checker_void_return() {
    let (errors, _, _) = type_check("void foo() { } int main() { foo(); return 0; }");
    assert!(errors.is_empty(), "Void function should not require return");
}

#[test]
fn test_type_checker_function_arg_count() {
    let (errors, _, _) = type_check("int foo(int a) { return a; } int main() { foo(); return 0; }");
    assert!(!errors.is_empty(), "Expected error for wrong arg count");
}

#[test]
fn test_type_checker_struct_member_access() {
    let src = r#"
        struct Point { int x; int y; };
        int main() {
            struct Point p;
            p.x = 10;
            return p.x;
        }
    "#;
    let (errors, _, _) = type_check(src);
    assert!(errors.is_empty(), "Type errors: {:?}", errors);
}

#[test]
fn test_type_checker_pointer_arithmetic() {
    let (errors, _, _) = type_check("int main() { int *p; p = p + 1; return 0; }");
    assert!(errors.is_empty(), "Pointer arithmetic should be valid");
}

#[test]
fn test_type_checker_array_index() {
    let (errors, _, _) = type_check("int main() { int arr[5]; arr[0] = 1; return arr[0]; }");
    assert!(errors.is_empty(), "Array index should be valid");
}

#[test]
fn test_type_checker_float_implicit_cast() {
    let (errors, _warnings, _) = type_check("int main() { float f = 10; return 0; }");
    assert!(errors.is_empty(), "int to float implicit cast should be allowed");
    // Should have a hint but not an error
}

#[test]
fn test_type_checker_pointer_assign() {
    let (errors, _, _) = type_check("int main() { int x; int *p = &x; *p = 5; return x; }");
    assert!(errors.is_empty(), "Pointer assignment should be valid");
}

#[test]
fn test_type_checker_function_pointer() {
    let src = r#"
        int add(int a, int b) { return a + b; }
        int main() {
            int (*fp)(int, int) = add;
            return fp(1, 2);
        }
    "#;
    let (errors, _, _) = type_check(src);
    assert!(errors.is_empty(), "Function pointer should be valid: {:?}", errors);
}

#[test]
fn test_type_checker_union_member_access() {
    let src = r#"
        union U { int i; float f; };
        int main() {
            union U u;
            u.i = 10;
            return u.i;
        }
    "#;
    let (errors, _, _) = type_check(src);
    assert!(errors.is_empty(), "Union member access should be valid");
}

#[test]
fn test_type_checker_string_literal_to_char_ptr() {
    let (errors, _, _) = type_check("int main() { char *s = \"hello\"; return 0; }");
    assert!(errors.is_empty(), "String literal to char* should be valid");
}

#[test]
fn test_type_checker_ternary_string_literals() {
    // 修复：三目运算符两个分支均为字符串字面量（不同长度数组）时应统一为 char*
    let src = r#"
        #include <stdio.h>
        int main() {
            printf("%s", 1 ? " " : "");
            return 0;
        }
    "#;
    let (errors, _, _) = type_check(src);
    assert!(errors.is_empty(), "Ternary with string literals should be valid: {:?}", errors);
}

#[test]
fn test_type_checker_struct_return_by_value() {
    let src = r#"
        struct S { int a; int b; };
        struct S foo() {
            struct S s;
            return s;
        }
        int main() { return 0; }
    "#;
    let (errors, _, _) = type_check(src);
    assert!(errors.is_empty(), "Struct return by value should be allowed: {:?}", errors);
}

#[test]
fn test_type_checker_struct_pointer_return_allowed() {
    let src = r#"
        struct S { int a; };
        struct S* foo() {
            return 0;
        }
        int main() { return 0; }
    "#;
    let (errors, _, _) = type_check(src);
    assert!(errors.is_empty(), "Struct pointer return should be allowed: {:?}", errors);
}

#[test]
fn test_type_checker_incompatible_pointer_assignment_warns() {
    // B39: 不兼容的具体指针类型赋值应报告 warning，但不影响编译。
    // P1-6：改用专用码 W3067（"指针类型不兼容"），不再复用标量转换码 W3053
    //（后者的建议文案是"可能导致数据截断"，用在指针上属教学误导）。
    let (errors, warnings, _) = type_check("int main() { int x; int *p = (double *)&x; return 0; }");
    assert!(
        errors.is_empty(),
        "Incompatible pointer assignment should not be an error: {:?}",
        errors
    );
    assert!(
        warnings.iter().any(|w| w.message.contains("指针类型不兼容")),
        "Expected warning for incompatible pointer assignment, got: {:?}",
        warnings
    );
}

#[test]
fn test_type_checker_pointer_add_assign_valid() {
    let (errors, _, _) = type_check("int main() { int a[5]; int *p = a; p += 2; return 0; }");
    assert!(errors.is_empty(), "Pointer += int should be valid: {:?}", errors);
}

#[test]
fn test_type_checker_pointer_sub_assign_valid() {
    let (errors, _, _) = type_check("int main() { int a[5]; int *p = a + 4; p -= 1; return 0; }");
    assert!(errors.is_empty(), "Pointer -= int should be valid: {:?}", errors);
}

#[test]
fn test_type_checker_void_pointer_add_assign_valid() {
    let (errors, _, _) = type_check("int main() { char buf[10]; void *p = buf; p += 3; return 0; }");
    assert!(errors.is_empty(), "void* += int should be valid: {:?}", errors);
}

#[test]
fn test_type_checker_pointer_mul_assign_error() {
    let (errors, _, _) = type_check("int main() { int *p; p *= 2; return 0; }");
    assert!(!errors.is_empty(), "Pointer *= should report an error");
    assert!(errors.iter().any(|e| e.code == 3045), "Expected E3045_CompoundAssignType");
}

#[test]
fn test_type_checker_pointer_add_assign_pointer_error() {
    let (errors, _, _) = type_check("int main() { int *p, *q; p += q; return 0; }");
    assert!(!errors.is_empty(), "Pointer += pointer should report an error");
    assert!(errors.iter().any(|e| e.code == 3045), "Expected E3045_CompoundAssignType");
}

#[test]
fn test_type_checker_function_pointer_add_assign_error() {
    let src = r#"
        int f(int x) { return x; }
        int main() {
            int (*fp)(int) = f;
            fp += 1;
            return 0;
        }
    "#;
    let (errors, _, _) = type_check(src);
    assert!(!errors.is_empty(), "Function pointer += should report an error");
    assert!(errors.iter().any(|e| e.code == 3045), "Expected E3045_CompoundAssignType");
}
