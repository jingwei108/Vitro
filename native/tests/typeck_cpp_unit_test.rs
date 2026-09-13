#![allow(clippy::unwrap_used, clippy::expect_used)]

use vitro_native::compiler::ast::*;
use vitro_native::compiler::lexer::Lexer;
use vitro_native::compiler::parser::Parser;
use vitro_native::compiler::typeck::TypeChecker;

fn parse_and_typecheck_cpp(src: &str) -> (ProgramNode, Vec<vitro_native::compiler::typeck::TypeError>) {
    let (tokens, _) = Lexer::with_mode(src, true).tokenize();
    let (program, parse_errors) = Parser::with_mode(tokens, true).parse();
    if !parse_errors.is_empty() {
        panic!("Parse errors: {:?}", parse_errors);
    }
    let mut program = program.unwrap();
    let checker = TypeChecker::default();
    let (errors, _warnings, _hints) = checker.check(&mut program);
    (program, errors)
}

// ============================================================================
// Class Type Checking Tests
// ============================================================================

#[test]
fn test_class_basic_typecheck() {
    let src = r#"
class Point {
public:
    int x;
    int y;
};
int main() {
    Point p;
    p.x = 10;
    p.y = 20;
    return p.x + p.y;
}
"#;
    let (program, errors) = parse_and_typecheck_cpp(src);
    assert!(errors.is_empty(), "Type errors: {:?}", errors);
    assert_eq!(program.classes.len(), 1);
}

#[test]
fn test_class_private_access_denied() {
    let src = r#"
class Secret {
private:
    int code;
};
int main() {
    Secret s;
    s.code = 42;
    return 0;
}
"#;
    let (_program, errors) = parse_and_typecheck_cpp(src);
    let has_private_error = errors.iter().any(|e| e.message.contains("私有"));
    assert!(has_private_error, "Expected private access error, got: {:?}", errors);
}

#[test]
fn test_class_method_call() {
    let src = r#"
class Rect {
public:
    int width;
    int area() { return width * 2; }
};
int main() {
    Rect r;
    r.width = 5;
    int a = r.area();
    return a;
}
"#;
    let (_program, errors) = parse_and_typecheck_cpp(src);
    assert!(errors.is_empty(), "Type errors: {:?}", errors);
}

#[test]
fn test_this_type() {
    let src = r#"
class Counter {
public:
    int val;
    void inc() { this->val = this->val + 1; }
};
int main() { return 0; }
"#;
    let (_program, errors) = parse_and_typecheck_cpp(src);
    assert!(errors.is_empty(), "Type errors: {:?}", errors);
}

#[test]
fn test_this_outside_class() {
    let src = r#"
int main() {
    return this->x;
}
"#;
    let (_program, errors) = parse_and_typecheck_cpp(src);
    let has_this_error = errors.iter().any(|e| e.message.contains("this"));
    assert!(has_this_error, "Expected 'this' outside class error, got: {:?}", errors);
}

#[test]
fn test_cpp_method_ref_return_assign() {
    let src = r#"
class Box {
public:
    int v;
    Box() { v = 0; }
    int& get() { return v; }
    Box& set(int x) { v = x; return *this; }
};
int main() {
    Box b;
    b.get() = 42;
    b.set(1).set(2);
    return b.v;
}
"#;
    let (_program, errors) = parse_and_typecheck_cpp(src);
    assert!(errors.is_empty(), "Type errors: {:?}", errors);
}

#[test]
fn test_cpp_ref_param_access_private_member() {
    let src = r#"
class Pair {
    int a;
public:
    Pair() { a = 0; }
    void copy_from(Pair& o) { a = o.a; }
    int get() { return a; }
};
int main() {
    Pair x;
    Pair y;
    y.copy_from(x);
    return y.get();
}
"#;
    let (_program, errors) = parse_and_typecheck_cpp(src);
    assert!(errors.is_empty(), "Type errors: {:?}", errors);
}

// ============================================================================
// Auto Type Deduction Tests
// ============================================================================

#[test]
fn test_auto_deduce_int() {
    let src = r#"
int main() {
    auto x = 5;
    return x;
}
"#;
    let (_program, errors) = parse_and_typecheck_cpp(src);
    assert!(errors.is_empty(), "Type errors: {:?}", errors);
}

#[test]
fn test_auto_deduce_float() {
    let src = r#"
int main() {
    auto f = 3.14;
    return 0;
}
"#;
    let (_program, errors) = parse_and_typecheck_cpp(src);
    assert!(errors.is_empty(), "Type errors: {:?}", errors);
}

#[test]
fn test_auto_no_init_error() {
    let src = r#"
int main() {
    auto x;
    return 0;
}
"#;
    let (_program, errors) = parse_and_typecheck_cpp(src);
    let has_auto_error = errors.iter().any(|e| e.message.contains("auto"));
    assert!(has_auto_error, "Expected auto requires init error, got: {:?}", errors);
}

// ============================================================================
// Reference Type Tests
// ============================================================================
// NOTE: Reference declarations (`int& r = x`) require Parser support for `&`
// in declarators, which is not yet implemented (Phase 31 only added AST/Lexer
// nodes). These tests are deferred until Parser handles reference syntax.

// ============================================================================
// Template Monomorphization Tests
// ============================================================================

#[test]
fn test_class_template_type_mismatch() {
    let src = r#"
template <class T> class Box { public: T value; };
int main() {
    Box<int> b;
    b.value = "hello";
    return 0;
}
"#;
    let (_, errors) = parse_and_typecheck_cpp(src);
    assert!(
        errors.iter().any(|e| e.message.contains("类型不匹配")),
        "Expected type mismatch error, got: {:?}",
        errors
    );
}

#[test]
fn test_template_func_monomorph() {
    let src = r#"
template <typename T> T max(T a, T b) { if (a > b) return a; return b; }
int main() {
    int m = max(3, 5);
    return m;
}
"#;
    let (program, errors) = parse_and_typecheck_cpp(src);
    assert!(errors.is_empty(), "Type errors: {:?}", errors);
    // Check that the instantiated function was added
    let has_instantiated = program.funcs.iter().any(|f| f.name == "max__int");
    assert!(has_instantiated, "Expected instantiated function 'max__int' in program.funcs");
}

// ============================================================================
// New / Delete Type Check Tests
// ============================================================================

#[test]
fn test_new_delete_typecheck() {
    let src = r#"
int main() {
    int* p = new int(42);
    delete p;
    return 0;
}
"#;
    let (_program, errors) = parse_and_typecheck_cpp(src);
    assert!(errors.is_empty(), "Type errors: {:?}", errors);
}

// ============================================================================
// Range For Tests
// ============================================================================

#[test]
fn test_range_for_array() {
    let src = r#"
int main() {
    int v[3] = {1, 2, 3};
    int sum = 0;
    for (auto x : v) { sum = sum + x; }
    return sum;
}
"#;
    let (_program, errors) = parse_and_typecheck_cpp(src);
    assert!(errors.is_empty(), "Type errors: {:?}", errors);
}

// ============================================================================
// Try / Catch Error Test
// ============================================================================
// NOTE: Parser does not support try/catch syntax yet. TypeChecker handles
// Stmt::Try by reporting E4001, but we cannot test it end-to-end without
// Parser support.

#[test]
fn test_try_stmt_typecheck() {
    use vitro_native::compiler::ast::{SourceLoc, Stmt};
    let try_stmt = Stmt::Try {
        body: Box::new(Stmt::Block {
            stmts: vec![],
            loc: SourceLoc { line: 1, column: 1, file_id: 0 },
        }),
        catches: vec![],
        loc: SourceLoc { line: 1, column: 1, file_id: 0 },
    };
    let checker = TypeChecker::default();
    let mut program = ProgramNode::default();
    program.funcs.push(FuncDecl {
        loc: SourceLoc { line: 1, column: 1, file_id: 0 },
        return_type: Type::int(),
        name: "main".to_string(),
        params: vec![],
        body: Some(try_stmt.clone()),
        is_static: false,
        is_extern: false,
        source_file: String::new(),
        is_variadic: false,
    });
    let (errors, _warnings, _hints) = checker.check(&mut program);
    let has_try_error = errors.iter().any(|e| e.message.contains("异常"));
    assert!(has_try_error, "Expected exception not supported error, got: {:?}", errors);
}

// ============================================================================
// Reference Tests (Stage 4)
// ============================================================================

#[test]
fn test_cpp_reference_decl() {
    let src = r#"
int main() {
    int x = 10;
    int& r = x;
    const int& cr = x;
    return r;
}
"#;
    let (_program, errors) = parse_and_typecheck_cpp(src);
    assert!(errors.is_empty(), "Type errors: {:?}", errors);
}

#[test]
fn test_cpp_reference_bind_lvalue() {
    let src = r#"
int main() {
    int x = 10;
    int& r = x;
    r = 20;
    return x;
}
"#;
    let (_program, errors) = parse_and_typecheck_cpp(src);
    assert!(errors.is_empty(), "Type errors: {:?}", errors);
}

#[test]
fn test_cpp_reference_bind_rvalue_error() {
    let src = r#"
int main() {
    int& r = 5;
    return 0;
}
"#;
    let (_program, errors) = parse_and_typecheck_cpp(src);
    let has_ref_error = errors.iter().any(|e| e.message.contains("左值") || e.code == 4029);
    assert!(has_ref_error, "Expected lvalue required error, got: {:?}", errors);
}

#[test]
fn test_cpp_reference_param() {
    let src = r#"
void inc(int& x) {
    x = x + 1;
}
int main() {
    int a = 5;
    inc(a);
    return a;
}
"#;
    let (_program, errors) = parse_and_typecheck_cpp(src);
    assert!(errors.is_empty(), "Type errors: {:?}", errors);
}

#[test]
fn test_cpp_auto_ref_deduction() {
    let src = r#"
int main() {
    int x = 42;
    auto& r = x;
    const auto& cr = x;
    return 0;
}
"#;
    let (_program, errors) = parse_and_typecheck_cpp(src);
    assert!(errors.is_empty(), "Type errors: {:?}", errors);
}

#[test]
fn test_cpp_auto_ref_range_for() {
    let src = r#"
int main() {
    int arr[] = {1, 2, 3};
    for (auto& x : arr) {
        x = x * 2;
    }
    return 0;
}
"#;
    let (_program, errors) = parse_and_typecheck_cpp(src);
    assert!(errors.is_empty(), "Type errors: {:?}", errors);
}

#[test]
fn test_cpp_const_ref_bind_rvalue() {
    let src = r#"
int main() {
    const int& r = 5;
    return 0;
}
"#;
    let (_program, errors) = parse_and_typecheck_cpp(src);
    assert!(errors.is_empty(), "Type errors: {:?}", errors);
}

#[test]
fn test_cpp_auto_new_struct() {
    let src = r#"
struct Node { int x; };
int main() {
    auto p = new struct Node;
    p->x = 42;
    return 0;
}
"#;
    let (_program, errors) = parse_and_typecheck_cpp(src);
    assert!(errors.is_empty(), "Type errors: {:?}", errors);
}

#[test]
fn test_cpp_std_move() {
    let src = r#"
int main() {
    int x = 42;
    int&& r = std::move(x);
    return 0;
}
"#;
    let (_program, errors) = parse_and_typecheck_cpp(src);
    assert!(errors.is_empty(), "Type errors: {:?}", errors);
}

#[test]
fn test_cpp_const_method_read_member() {
    let src = r#"
class Box {
public:
    int x;
    int get() const { return x; }
};
int main() {
    Box b;
    b.x = 10;
    return b.get();
}
"#;
    let (_program, errors) = parse_and_typecheck_cpp(src);
    assert!(errors.is_empty(), "Type errors: {:?}", errors);
}

#[test]
fn test_cpp_const_method_cannot_modify_member() {
    let src = r#"
class Box {
public:
    int x;
    void set(int v) const { x = v; }
};
int main() { return 0; }
"#;
    let (_program, errors) = parse_and_typecheck_cpp(src);
    assert!(!errors.is_empty(), "Should report const violation");
    assert!(
        errors.iter().any(|e| e.code == 3065),
        "Expected E3065 ConstViolation, got: {:?}",
        errors
    );
}

#[test]
fn test_cpp_const_method_this_is_const() {
    let src = r#"
class Box {
public:
    int x;
    void modify() const { this->x = 42; }
};
int main() { return 0; }
"#;
    let (_program, errors) = parse_and_typecheck_cpp(src);
    assert!(
        !errors.is_empty(),
        "Should report const violation for this->x assignment in const method"
    );
    assert!(
        errors.iter().any(|e| e.code == 3065),
        "Expected E3065 ConstViolation, got: {:?}",
        errors
    );
}

#[test]
fn test_cpp_const_object_cannot_call_nonconst_method() {
    let src = r#"
class Box {
public:
    int x;
    int get() const { return x; }
    void set(int v) { x = v; }
};
int main() {
    const Box b;
    b.set(10);
    return 0;
}
"#;
    let (_program, errors) = parse_and_typecheck_cpp(src);
    assert!(
        !errors.is_empty(),
        "Should report error calling non-const method on const object"
    );
    assert!(
        errors.iter().any(|e| e.code == 3065),
        "Expected E3065 ConstViolation, got: {:?}",
        errors
    );
}

#[test]
fn test_cpp_const_object_can_call_const_method() {
    let src = r#"
class Box {
public:
    int x;
    int get() const { return x; }
    void set(int v) { x = v; }
};
int main() {
    const Box b;
    return b.get();
}
"#;
    let (_program, errors) = parse_and_typecheck_cpp(src);
    assert!(errors.is_empty(), "Type errors: {:?}", errors);
}

#[test]
fn test_cpp_ctor_overload_same_count_different_type_rejected() {
    let src = r#"
class Box {
public:
    int x;
    Box(int v) { x = v; }
    Box(float v) { x = (int)v + 100; }
};
int main() {
    Box b(5);
    return b.x;
}
"#;
    let (_program, errors) = parse_and_typecheck_cpp(src);
    assert!(
        errors.iter().any(|e| e.code == 4031),
        "Expected E4031 ConstructorOverloadAmbiguous, got: {:?}",
        errors
    );
}

#[test]
fn test_cpp_multi_var_ctor_init_inserts_this_for_all() {
    // B40: 多个类类型变量在同一声明中初始化时，每个构造函数调用都应插入 this 指针。
    let src = r#"
class Box {
public:
    int x;
    Box(int v) { x = v; }
};
int main() {
    Box a(1), b(2);
    return a.x + b.x;
}
"#;
    let (_program, errors) = parse_and_typecheck_cpp(src);
    assert!(
        errors.is_empty(),
        "Multi-var ctor init should not produce type errors: {:?}",
        errors
    );
}
