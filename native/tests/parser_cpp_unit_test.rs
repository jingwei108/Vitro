#![allow(clippy::unwrap_used, clippy::expect_used)]

use vitro_native::compiler::ast::{AccessSpec, ClassMember, Stmt, TemplateArg, TemplateParam, Type, TypeKind};
use vitro_native::compiler::lexer::Lexer;
use vitro_native::compiler::parser::Parser;

fn parse_cpp(
    src: &str,
) -> (
    Option<vitro_native::compiler::ast::ProgramNode>,
    Vec<vitro_native::compiler::parser::ParseError>,
) {
    let (tokens, _) = Lexer::with_mode(src, true).tokenize();
    Parser::with_mode(tokens, true).parse()
}

#[test]
fn test_parser_cpp_class_basic() {
    let src = "class Foo { public: int x; }; int main() { return 0; }";
    let (program, errors) = parse_cpp(src);
    assert!(errors.is_empty(), "Parse errors: {:?}", errors);
    let program = program.unwrap();
    assert_eq!(program.classes.len(), 1);
    assert_eq!(program.classes[0].name, "Foo");
    assert!(program.classes[0].base.is_none());
    assert_eq!(program.classes[0].members.len(), 1);
    match &program.classes[0].members[0] {
        ClassMember::Field { name, access, .. } => {
            assert_eq!(name, "x");
            assert_eq!(*access, AccessSpec::Public);
        }
        _ => panic!("Expected Field"),
    }
}

#[test]
fn test_parser_cpp_class_with_private() {
    let src = r#"
class Point {
public:
    int x;
private:
    int y;
};
int main() { return 0; }
"#;
    let (program, errors) = parse_cpp(src);
    assert!(errors.is_empty(), "Parse errors: {:?}", errors);
    let program = program.unwrap();
    assert_eq!(program.classes.len(), 1);
    let class = &program.classes[0];
    assert_eq!(class.members.len(), 2);
    match &class.members[0] {
        ClassMember::Field { name, access, .. } => {
            assert_eq!(name, "x");
            assert_eq!(*access, AccessSpec::Public);
        }
        _ => panic!("Expected Field"),
    }
    match &class.members[1] {
        ClassMember::Field { name, access, .. } => {
            assert_eq!(name, "y");
            assert_eq!(*access, AccessSpec::Private);
        }
        _ => panic!("Expected Field"),
    }
}

#[test]
fn test_parser_cpp_class_field_comma_decl() {
    let src = r#"
class Point {
public:
    int x, y;
private:
    int *px, *py;
};
int main() { return 0; }
"#;
    let (program, errors) = parse_cpp(src);
    assert!(errors.is_empty(), "Parse errors: {:?}", errors);
    let program = program.unwrap();
    assert_eq!(program.classes.len(), 1);
    let class = &program.classes[0];
    assert_eq!(class.members.len(), 4);
    let expected = [
        ("x", AccessSpec::Public),
        ("y", AccessSpec::Public),
        ("px", AccessSpec::Private),
        ("py", AccessSpec::Private),
    ];
    for (i, (name, access)) in expected.iter().enumerate() {
        match &class.members[i] {
            ClassMember::Field { name: n, access: a, .. } => {
                assert_eq!(n, *name);
                assert_eq!(a, access);
            }
            _ => panic!("Expected Field at index {}", i),
        }
    }
}

#[test]
fn test_parser_cpp_class_method() {
    let src = r#"
class Rect {
public:
    int width;
    int area() { return width * 2; }
};
int main() { return 0; }
"#;
    let (program, errors) = parse_cpp(src);
    assert!(errors.is_empty(), "Parse errors: {:?}", errors);
    let program = program.unwrap();
    let class = &program.classes[0];
    assert_eq!(class.members.len(), 2);
    match &class.members[1] {
        ClassMember::Method { name, access, body, .. } => {
            assert_eq!(name, "area");
            assert_eq!(*access, AccessSpec::Public);
            assert!(body.is_some());
        }
        _ => panic!("Expected Method, got {:?}", class.members[1]),
    }
}

#[test]
fn test_parser_cpp_class_method_ref_return() {
    let src = r#"
class Counter {
public:
    int x;
    Counter() { x = 0; }
    int& get_x() { return x; }
    Counter& inc() { x++; return *this; }
};
int main() { return 0; }
"#;
    let (program, errors) = parse_cpp(src);
    assert!(errors.is_empty(), "Parse errors: {:?}", errors);
    let program = program.unwrap();
    let class = &program.classes[0];
    assert_eq!(class.members.len(), 4);
    match &class.members[2] {
        ClassMember::Method { name, ret, .. } => {
            assert_eq!(name, "get_x");
            assert!(ret.is_reference(), "Expected reference return, got {:?}", ret);
            assert_eq!(ret.reference_base().unwrap().kind(), Type::int().kind());
        }
        _ => panic!("Expected Method"),
    }
    match &class.members[3] {
        ClassMember::Method { name, ret, .. } => {
            assert_eq!(name, "inc");
            assert!(ret.is_reference(), "Expected reference return, got {:?}", ret);
        }
        _ => panic!("Expected Method"),
    }
}

#[test]
fn test_parser_cpp_class_inheritance() {
    let src = r#"
class Base { public: int x; };
class Derived : public Base { public: int y; };
int main() { return 0; }
"#;
    let (program, errors) = parse_cpp(src);
    assert!(errors.is_empty(), "Parse errors: {:?}", errors);
    let program = program.unwrap();
    assert_eq!(program.classes.len(), 2);
    assert_eq!(program.classes[1].name, "Derived");
    assert_eq!(program.classes[1].base.as_ref().unwrap(), "Base");
}

#[test]
fn test_parser_cpp_class_constructor() {
    let src = r#"
class Point {
public:
    int x;
    Point() { x = 0; }
};
int main() { return 0; }
"#;
    let (program, errors) = parse_cpp(src);
    assert!(errors.is_empty(), "Parse errors: {:?}", errors);
    let program = program.unwrap();
    let class = &program.classes[0];
    assert_eq!(class.members.len(), 2);
    match &class.members[1] {
        ClassMember::Constructor { access, body, .. } => {
            assert_eq!(*access, AccessSpec::Public);
            assert!(body.is_some());
        }
        _ => panic!("Expected Constructor, got {:?}", class.members[1]),
    }
}

#[test]
fn test_parser_cpp_template_function() {
    let src = r#"
template <typename T> T max(T a, T b) { if (a > b) return a; return b; }
int main() { return 0; }
"#;
    let (program, errors) = parse_cpp(src);
    assert!(errors.is_empty(), "Parse errors: {:?}", errors);
    let program = program.unwrap();
    assert_eq!(program.templates.len(), 1);
    assert_eq!(program.templates[0].params.len(), 1);
    match &program.templates[0].params[0] {
        TemplateParam::Type { name, .. } | TemplateParam::NonType { name, .. } => {
            assert_eq!(name, "T");
        }
    }
    match &program.templates[0].decl {
        vitro_native::compiler::ast::Templateable::Func(ref f) => {
            assert_eq!(f.name, "max");
            assert_eq!(f.params.len(), 2);
        }
        _ => panic!("Expected Func template"),
    }
}

#[test]
fn test_parser_cpp_template_class() {
    let src = r#"
template <typename T> class Box { public: T value; };
int main() { return 0; }
"#;
    let (program, errors) = parse_cpp(src);
    assert!(errors.is_empty(), "Parse errors: {:?}", errors);
    let program = program.unwrap();
    assert_eq!(program.templates.len(), 1);
    match &program.templates[0].decl {
        vitro_native::compiler::ast::Templateable::Class(ref c) => {
            assert_eq!(c.name, "Box");
            assert_eq!(c.members.len(), 1);
        }
        _ => panic!("Expected Class template"),
    }
}

#[test]
fn test_parser_cpp_template_class_self_ref_param() {
    let src = r#"
template<class T>
class unique_ptr {
    T* p;
public:
    unique_ptr(T* x) : p(x) {}
    void move_from(unique_ptr<T>& o) { p = o.p; o.p = (T*)0; }
};
int main() { return 0; }
"#;
    let (program, errors) = parse_cpp(src);
    assert!(errors.is_empty(), "Parse errors: {:?}", errors);
    let program = program.unwrap();
    assert_eq!(program.templates.len(), 1);
    match &program.templates[0].decl {
        vitro_native::compiler::ast::Templateable::Class(ref c) => {
            assert_eq!(c.name, "unique_ptr");
            let method = match &c.members[2] {
                ClassMember::Method { name, params, .. } => {
                    assert_eq!(name, "move_from");
                    assert_eq!(params.len(), 1);
                    params[0].ty.clone()
                }
                _ => panic!("Expected Method"),
            };
            assert!(method.is_reference(), "Expected reference param, got {:?}", method);
            let base = method.reference_base().unwrap();
            assert!(
                matches!(base.kind(), TypeKind::TemplateId),
                "Expected TemplateId base, got {:?}",
                base
            );
        }
        _ => panic!("Expected Class template"),
    }
}

#[test]
fn test_parser_cpp_mode_class_as_identifier_in_c() {
    // In C mode, 'class' should be treated as a normal identifier
    let src = "int class = 10; int main() { return class; }";
    let (tokens, _) = Lexer::new(src).tokenize();
    let (program, errors) = Parser::new(tokens).parse();
    assert!(errors.is_empty(), "Parse errors: {:?}", errors);
    let program = program.unwrap();
    assert!(program.classes.is_empty());
    assert_eq!(program.globals.len(), 1);
    assert_eq!(program.globals[0].name, "class");
}

// ============================================================================
// C++ Expression / Statement Tests (Step 4)
// ============================================================================

use vitro_native::compiler::ast::Expr;

#[test]
fn test_parser_cpp_this_expr() {
    let src = r#"
class Foo {
public:
    int x;
    void set(int v) { this->x = v; }
};
int main() { return 0; }
"#;
    let (program, errors) = parse_cpp(src);
    assert!(errors.is_empty(), "Parse errors: {:?}", errors);
    let program = program.unwrap();
    let class = &program.classes[0];
    match &class.members[1] {
        ClassMember::Method { body, .. } => {
            if let Some(Stmt::Block { stmts, .. }) = body {
                if let Stmt::Expr { expr, .. } = &stmts[0] {
                    if let Expr::Assign { left, .. } = expr {
                        if let Expr::Member { object, .. } = left.as_ref() {
                            assert!(matches!(object.as_ref(), Expr::This { .. }), "Expected this->x");
                        } else {
                            panic!("Expected Member assign");
                        }
                    } else {
                        panic!("Expected Assign expr");
                    }
                } else {
                    panic!("Expected Expr stmt");
                }
            } else {
                panic!("Expected Block body");
            }
        }
        _ => panic!("Expected Method"),
    }
}

#[test]
fn test_parser_cpp_member_call() {
    let src = r#"
class Calc {
public:
    int add(int a, int b) { return a + b; }
};
int main() { return 0; }
"#;
    let (program, errors) = parse_cpp(src);
    assert!(errors.is_empty(), "Parse errors: {:?}", errors);
    let program = program.unwrap();
    let class = &program.classes[0];
    match &class.members[0] {
        ClassMember::Method { name, .. } => {
            assert_eq!(name, "add");
        }
        _ => panic!("Expected Method"),
    }
}

#[test]
fn test_parser_cpp_new_delete() {
    let src = r#"
int main() {
    int* p = new int(42);
    delete p;
    int* arr = new int[10];
    delete[] arr;
    return 0;
}
"#;
    let (program, errors) = parse_cpp(src);
    assert!(errors.is_empty(), "Parse errors: {:?}", errors);
    let program = program.unwrap();
    let body = program.funcs[0].body.as_ref().unwrap();
    if let Stmt::Block { stmts, .. } = body {
        // var decl: int* p = new int(42);
        if let Stmt::VarDecl { init: Some(expr), .. } = &stmts[0] {
            assert!(matches!(expr, Expr::New { .. }), "Expected new expr");
        } else {
            panic!("Expected VarDecl with new init");
        }
        // delete p;
        if let Stmt::Expr { expr, .. } = &stmts[1] {
            assert!(matches!(expr, Expr::Delete { is_array: false, .. }), "Expected delete expr");
        } else {
            panic!("Expected delete stmt");
        }
        // delete[] arr;
        if let Stmt::Expr { expr, .. } = &stmts[3] {
            assert!(matches!(expr, Expr::Delete { is_array: true, .. }), "Expected delete[] expr");
        } else {
            panic!("Expected delete[] stmt");
        }
    } else {
        panic!("Expected Block");
    }
}

#[test]
fn test_parser_cpp_range_for() {
    let src = r#"
int main() {
    int v[3] = {1, 2, 3};
    int sum = 0;
    for (auto x : v) { sum = sum + x; }
    return sum;
}
"#;
    let (program, errors) = parse_cpp(src);
    assert!(errors.is_empty(), "Parse errors: {:?}", errors);
    let program = program.unwrap();
    let body = program.funcs[0].body.as_ref().unwrap();
    if let Stmt::Block { stmts, .. } = body {
        if let Stmt::RangeFor { var, var_type, .. } = &stmts[2] {
            assert_eq!(var, "x");
            assert!(matches!(var_type, vitro_native::compiler::ast::Type::Auto));
        } else {
            panic!("Expected RangeFor, got {:?}", stmts[2]);
        }
    } else {
        panic!("Expected Block");
    }
}

#[test]
fn test_parser_template_id_type() {
    let src = r#"
template <typename T> class vector { public: T* data; };
int main() {
    vector<int> v;
    return 0;
}
"#;
    let (program, errors) = parse_cpp(src);
    assert!(errors.is_empty(), "Parse errors: {:?}", errors);
    let program = program.unwrap();
    let body = program.funcs[0].body.as_ref().unwrap();
    if let Stmt::Block { stmts, .. } = body {
        if let Stmt::VarDecl { var_type, .. } = &stmts[0] {
            if let vitro_native::compiler::ast::Type::TemplateId { base, args, .. } = var_type {
                assert_eq!(base, "vector");
                assert_eq!(args.len(), 1);
                assert!(matches!(args[0], TemplateArg::Type(Type::Int { .. })));
            } else {
                panic!("Expected TemplateId, got {:?}", var_type);
            }
        } else {
            panic!("Expected VarDecl");
        }
    } else {
        panic!("Expected Block");
    }
}

#[test]
fn test_parser_template_id_nested_pointer() {
    let src = r#"
template <typename T> class vector { public: T* data; };
int main() {
    vector<int>* p;
    return 0;
}
"#;
    let (program, errors) = parse_cpp(src);
    assert!(errors.is_empty(), "Parse errors: {:?}", errors);
    let program = program.unwrap();
    let body = program.funcs[0].body.as_ref().unwrap();
    if let Stmt::Block { stmts, .. } = body {
        if let Stmt::VarDecl { var_type, .. } = &stmts[0] {
            if let vitro_native::compiler::ast::Type::Pointer { pointee, .. } = var_type {
                if let vitro_native::compiler::ast::Type::TemplateId { base, args, .. } = pointee.as_ref() {
                    assert_eq!(base, "vector");
                    assert_eq!(args.len(), 1);
                    assert!(matches!(args[0], TemplateArg::Type(Type::Int { .. })));
                } else {
                    panic!("Expected Pointer<TemplateId>, got {:?}", pointee);
                }
            } else {
                panic!("Expected Pointer, got {:?}", var_type);
            }
        } else {
            panic!("Expected VarDecl");
        }
    } else {
        panic!("Expected Block");
    }
}

#[test]
fn test_parser_cpp_lambda() {
    let src = r#"
int main() {
    auto f = [](int x) { return x * 2; };
    return f(5);
}
"#;
    let (program, errors) = parse_cpp(src);
    assert!(errors.is_empty(), "Parse errors: {:?}", errors);
    let program = program.unwrap();
    let body = program.funcs[0].body.as_ref().unwrap();
    if let Stmt::Block { stmts, .. } = body {
        if let Stmt::VarDecl { init: Some(expr), .. } = &stmts[0] {
            assert!(matches!(expr, Expr::Lambda { .. }), "Expected Lambda expr");
        } else {
            panic!("Expected VarDecl with lambda init");
        }
    } else {
        panic!("Expected Block");
    }
}

#[test]
fn test_parser_cpp_ctor_init_list() {
    let src = r#"
class Point {
public:
    int x;
    int y;
    Point() : x(10), y(20) {}
};
int main() { return 0; }
"#;
    let (program, errors) = parse_cpp(src);
    assert!(errors.is_empty(), "Parse errors: {:?}", errors);
    let program = program.unwrap();
    let class = &program.classes[0];
    assert_eq!(class.members.len(), 3);
    match &class.members[2] {
        ClassMember::Constructor { body, .. } => {
            assert!(body.is_some());
            if let Stmt::Block { stmts, .. } = body.as_ref().unwrap() {
                // Init list should produce two Expr::Assign statements before the body content.
                assert!(
                    stmts.len() >= 2,
                    "Expected at least 2 statements (init list), got {}",
                    stmts.len()
                );
                // First statement should be this->x = 10
                if let Stmt::Expr { expr, .. } = &stmts[0] {
                    if let Expr::Assign { left, .. } = expr {
                        if let Expr::Member { member, .. } = left.as_ref() {
                            assert_eq!(member, "x", "First init should be x");
                        } else {
                            panic!("Expected Member access on this");
                        }
                    } else {
                        panic!("Expected Assign expr, got {:?}", expr);
                    }
                } else {
                    panic!("Expected Expr statement");
                }
                // Second statement should be this->y = 20
                if let Stmt::Expr { expr, .. } = &stmts[1] {
                    if let Expr::Assign { left, .. } = expr {
                        if let Expr::Member { member, .. } = left.as_ref() {
                            assert_eq!(member, "y", "Second init should be y");
                        } else {
                            panic!("Expected Member access on this");
                        }
                    } else {
                        panic!("Expected Assign expr, got {:?}", expr);
                    }
                } else {
                    panic!("Expected Expr statement");
                }
            } else {
                panic!("Expected Block body");
            }
        }
        _ => panic!("Expected Constructor, got {:?}", class.members[2]),
    }
}

#[test]
fn test_parser_cpp_struct_tag_as_type_alias() {
    let src = r#"
struct Node { int x; };
Node* p;
int main() { return 0; }
"#;
    let (program, errors) = parse_cpp(src);
    assert!(errors.is_empty(), "Parse errors: {:?}", errors);
    let program = program.unwrap();
    // C++ 模式下 struct 被解析为 ClassDecl
    assert_eq!(program.classes.len(), 1);
    assert_eq!(program.classes[0].name, "Node");
    assert_eq!(program.globals.len(), 1);
    assert_eq!(program.globals[0].name, "p");
    assert_eq!(program.globals[0].ty.to_string(), "class Node*");
}

#[test]
fn test_parser_cpp_template_struct() {
    let src = r#"
template<class T>
struct Pair {
    T first;
    T second;
};
int main() { return 0; }
"#;
    let (program, errors) = parse_cpp(src);
    assert!(errors.is_empty(), "Parse errors: {:?}", errors);
    let program = program.unwrap();
    assert_eq!(program.templates.len(), 1);
    assert_eq!(program.templates[0].params.len(), 1);
    match &program.templates[0].params[0] {
        TemplateParam::Type { name, .. } | TemplateParam::NonType { name, .. } => {
            assert_eq!(name, "T");
        }
    }
    match &program.templates[0].decl {
        vitro_native::compiler::ast::Templateable::Class(c) => {
            assert_eq!(c.name, "Pair");
            assert_eq!(c.members.len(), 2);
        }
        _ => panic!("Expected Class template"),
    }
}

#[test]
fn test_parser_cpp_cast_expr() {
    let src = r#"
int main() {
    int* p = (int*)0;
    return 0;
}
"#;
    let (_program, errors) = parse_cpp(src);
    assert!(errors.is_empty(), "Parse errors: {:?}", errors);
}

#[test]
fn test_parser_cpp_ctor_init_list_cast_class() {
    let src = r#"
class Foo {
public:
    int* p;
    Foo() : p((int*)0) {}
};
int main() { return 0; }
"#;
    let (_program, errors) = parse_cpp(src);
    assert!(errors.is_empty(), "Parse errors: {:?}", errors);
}

#[test]
fn test_parser_cpp_class_inner_struct() {
    let src = r#"
class List {
public:
    struct Node {
        int data;
        Node* next;
    };
    Node* head;
};
int main() { return 0; }
"#;
    let (_program, errors) = parse_cpp(src);
    assert!(errors.is_empty(), "Parse errors: {:?}", errors);
}

#[test]
fn test_parser_cpp_template_class_inner_struct() {
    let src = r#"
template<class T>
class list {
    struct Node {
        T data;
        Node* next;
    };
    Node* head;
public:
    list() : head(nullptr) {}
};
int main() { return 0; }
"#;
    let (_program, errors) = parse_cpp(src);
    assert!(errors.is_empty(), "Parse errors: {:?}", errors);
}

#[test]
fn test_parser_cpp_member_func_outside_class() {
    let src = r#"
class Bar {
public:
    int x;
    void set(int v);
};
void Bar::set(int v) { x = v; }
int main() { return 0; }
"#;
    let (program, errors) = parse_cpp(src);
    assert!(errors.is_empty(), "Parse errors: {:?}", errors);
    let program = program.unwrap();
    let bar_set = program.funcs.iter().find(|f| f.name == "Bar__set");
    assert!(bar_set.is_some(), "Expected Bar__set function");
}

#[test]
fn test_parser_cpp_lambda_multi_capture() {
    let src = r#"
int main() {
    int a = 1, b = 2;
    auto f = [a, &b](int x) { return a + b + x; };
    return 0;
}
"#;
    let (_program, errors) = parse_cpp(src);
    assert!(errors.is_empty(), "Parse errors: {:?}", errors);
}

#[test]
fn test_parser_cpp_range_for_ref() {
    let src = r#"
int main() {
    int arr[3] = {1, 2, 3};
    for (auto& x : arr) { x = x * 2; }
    for (const auto& x : arr) {}
    return 0;
}
"#;
    let (_program, errors) = parse_cpp(src);
    assert!(errors.is_empty(), "Parse errors: {:?}", errors);
}

#[test]
fn test_parser_cpp_ctor_overload() {
    let src = r#"
class Box {
public:
    int x;
    Box() { x = 0; }
    Box(int v) { x = v; }
};
int main() { return 0; }
"#;
    let (program, errors) = parse_cpp(src);
    assert!(errors.is_empty(), "Parse errors: {:?}", errors);
    let program = program.unwrap();
    let box_class = &program.classes[0];
    let ctors: Vec<_> = box_class
        .members
        .iter()
        .filter(|m| matches!(m, ClassMember::Constructor { .. }))
        .collect();
    assert_eq!(ctors.len(), 2);
}

#[test]
fn test_parser_cpp_template_ctor_overload() {
    let src = r#"
template<class T>
class Vec {
public:
    T* data;
    Vec() {}
    Vec(int n) {}
};
int main() { return 0; }
"#;
    let (_program, errors) = parse_cpp(src);
    assert!(errors.is_empty(), "Parse errors: {:?}", errors);
}

#[test]
fn test_parser_cpp_ctor_init_list_with_body() {
    let src = r#"
class Rect {
public:
    int w;
    int h;
    Rect(int a, int b) : w(a), h(b) {
        int area = w * h;
    }
};
int main() { return 0; }
"#;
    let (program, errors) = parse_cpp(src);
    assert!(errors.is_empty(), "Parse errors: {:?}", errors);
    let program = program.unwrap();
    let class = &program.classes[0];
    match &class.members[2] {
        ClassMember::Constructor { body, .. } => {
            if let Stmt::Block { stmts, .. } = body.as_ref().unwrap() {
                // Init list (2 assignments) + body content (1 VarDecl)
                assert_eq!(stmts.len(), 3, "Expected 3 statements: 2 init + 1 body");
            } else {
                panic!("Expected Block body");
            }
        }
        _ => panic!("Expected Constructor"),
    }
}

#[test]
fn test_parser_cpp_const_member_func() {
    let src = r#"
class Box {
public:
    int x;
    int get() const { return x; }
    void set(int v) { x = v; }
};
int main() { return 0; }
"#;
    let (program, errors) = parse_cpp(src);
    assert!(errors.is_empty(), "Parse errors: {:?}", errors);
    let program = program.unwrap();
    let class = &program.classes[0];
    assert_eq!(class.members.len(), 3);
    match &class.members[1] {
        ClassMember::Method { name, is_const, .. } => {
            assert_eq!(name, "get");
            assert!(*is_const, "get() should be const");
        }
        _ => panic!("Expected Method for get"),
    }
    match &class.members[2] {
        ClassMember::Method { name, is_const, .. } => {
            assert_eq!(name, "set");
            assert!(!is_const, "set() should not be const");
        }
        _ => panic!("Expected Method for set"),
    }
}

#[test]
fn test_parser_cpp_const_member_func_outside_class() {
    let src = r#"
class Box {
public:
    int x;
    int get() const;
};
int Box::get() const { return x; }
int main() { return 0; }
"#;
    let (program, errors) = parse_cpp(src);
    assert!(errors.is_empty(), "Parse errors: {:?}", errors);
    let program = program.unwrap();
    assert_eq!(program.classes.len(), 1);
    match &program.classes[0].members[1] {
        ClassMember::Method { name, is_const, .. } => {
            assert_eq!(name, "get");
            assert!(*is_const, "get() should be const");
        }
        _ => panic!("Expected Method for get"),
    }
    assert!(program.funcs.iter().any(|f| f.name == "Box__get"), "Box__get should exist");
}
