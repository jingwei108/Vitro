#![allow(clippy::unwrap_used, clippy::expect_used)]

//! CompletionEngine 单元测试

use vitro_native::compiler::lexer::Lexer;
use vitro_native::compiler::parser::Parser;
use vitro_native::engine::completion::{
    build_snapshot, build_snapshot_from_source, get_completion_candidates, CompletionKind,
};
use vitro_native::session::Session;

fn parse(source: &str) -> Option<vitro_native::compiler::ast::ProgramNode> {
    let (tokens, _) = Lexer::new(source).tokenize();
    let (program, _) = Parser::new(tokens).parse();
    program
}

#[test]
fn test_build_snapshot_extracts_functions_and_globals() {
    let source = r#"
        int global_x;
        float global_y = 3.14;

        int add(int a, int b) {
            return a + b;
        }

        static void helper() {
            return;
        }
    "#;
    let program = parse(source).unwrap();
    let snapshot = build_snapshot(&program);

    assert_eq!(snapshot.globals.len(), 2);
    assert!(snapshot.globals.iter().any(|g| g.name == "global_x"));
    assert!(snapshot.globals.iter().any(|g| g.name == "global_y"));

    assert_eq!(snapshot.functions.len(), 2);
    let add = snapshot.functions.iter().find(|f| f.name == "add").unwrap();
    assert_eq!(add.return_type, "int");
    assert_eq!(add.params.len(), 2);
    assert!(!add.is_static);

    let helper = snapshot.functions.iter().find(|f| f.name == "helper").unwrap();
    assert!(helper.is_static);
}

#[test]
fn test_build_snapshot_extracts_structs() {
    let source = r#"
        struct Point {
            int x;
            int y;
        };

        union Data {
            int i;
            float f;
        };
    "#;
    let program = parse(source).unwrap();
    let snapshot = build_snapshot(&program);

    assert_eq!(snapshot.structs.len(), 1);
    let pt = &snapshot.structs[0];
    assert_eq!(pt.name, "Point");
    assert_eq!(pt.fields.len(), 2);
    assert_eq!(pt.fields[0].0, "x");

    assert_eq!(snapshot.unions.len(), 1);
    let data = &snapshot.unions[0];
    assert_eq!(data.name, "Data");
    assert_eq!(data.fields.len(), 2);
}

#[test]
fn test_completion_expression_context_locals_first() {
    let source = r#"
        int global;
        void foo() {
            int local_a = 1;
            float local_b = 2.0;
            
        }
    "#;
    let program = parse(source).unwrap();
    let mut session = Session::default();
    vitro_native::engine::completion::update_completion_snapshot(&mut session, &program);

    // r#" 后的换行导致 line 0 是空行
    // line 5 = `            ` (空行 inside foo), col 12
    let candidates = get_completion_candidates(&session, source, 5, 12, "local");

    let labels: Vec<String> = candidates.iter().map(|c| c.label.clone()).collect();
    assert!(labels.contains(&"local_a".to_string()), "missing local_a in {:?}", labels);
    assert!(labels.contains(&"local_b".to_string()), "missing local_b in {:?}", labels);
    // global/foo 不以 "local" 开头，被前缀过滤，这是预期行为
}

#[test]
fn test_completion_member_access_struct() {
    let source = r#"
        struct Point {
            int x;
            int y;
        };
        void foo() {
            struct Point p;
            p.
        }
    "#;
    let program = parse(source).unwrap();
    let mut session = Session::default();
    vitro_native::engine::completion::update_completion_snapshot(&mut session, &program);

    // line 7 = `            p.` (r#" 后的空行导致 line 0 为空)
    let candidates = get_completion_candidates(&session, source, 7, 14, "");

    let labels: Vec<String> = candidates.iter().map(|c| c.label.clone()).collect();
    assert!(
        labels.contains(&"x".to_string()),
        "should suggest struct field x, got: {:?}",
        labels
    );
    assert!(
        labels.contains(&"y".to_string()),
        "should suggest struct field y, got: {:?}",
        labels
    );
}

#[test]
fn test_completion_member_access_pointer() {
    let source = r#"
        struct Node {
            int val;
            struct Node* next;
        };
        void foo() {
            struct Node* n;
            n->
        }
    "#;
    let program = parse(source).unwrap();
    let mut session = Session::default();
    vitro_native::engine::completion::update_completion_snapshot(&mut session, &program);

    // line 7 = `            n->`
    let candidates = get_completion_candidates(&session, source, 7, 16, "");
    let labels: Vec<String> = candidates.iter().map(|c| c.label.clone()).collect();
    assert!(labels.contains(&"val".to_string()), "missing val in {:?}", labels);
    assert!(labels.contains(&"next".to_string()), "missing next in {:?}", labels);
}

#[test]
fn test_completion_type_context() {
    let source = r#"
        struct Point { int x; };
        union Data { int i; };
        void foo() {
            int 
        }
    "#;
    let program = parse(source).unwrap();
    let mut session = Session::default();
    vitro_native::engine::completion::update_completion_snapshot(&mut session, &program);

    // line 4 = `            int ` -> TypePosition 上下文
    let candidates = get_completion_candidates(&session, source, 4, 16, "");
    let labels: Vec<String> = candidates.iter().map(|c| c.label.clone()).collect();
    assert!(labels.contains(&"struct".to_string()), "missing struct in {:?}", labels);
    assert!(labels.contains(&"Point".to_string()), "missing Point in {:?}", labels);
    assert!(labels.contains(&"union".to_string()), "missing union in {:?}", labels);
}

#[test]
fn test_completion_format_string() {
    let source = r#"
        void foo() {
            printf("
        }
    "#;
    let program = parse(source).unwrap();
    let mut session = Session::default();
    vitro_native::engine::completion::update_completion_snapshot(&mut session, &program);

    let candidates = get_completion_candidates(&session, source, 2, 20, "%");
    let labels: Vec<String> = candidates.iter().map(|c| c.label.clone()).collect();
    assert!(labels.contains(&"%d".to_string()));
    assert!(labels.contains(&"%f".to_string()));
    assert!(labels.contains(&"%s".to_string()));
}

#[test]
fn test_completion_preprocessor() {
    let source = "#include <\n";
    let program = parse(source).unwrap_or_default();
    let mut session = Session::default();
    vitro_native::engine::completion::update_completion_snapshot(&mut session, &program);

    let candidates = get_completion_candidates(&session, source, 0, 10, "");
    let labels: Vec<String> = candidates.iter().map(|c| c.label.clone()).collect();
    assert!(labels.contains(&"stdio.h".to_string()), "missing stdio.h in {:?}", labels);
}

#[test]
fn test_completion_function_insert_text() {
    let source = r#"
        void bar(int x) {}
        void foo() {
            
        }
    "#;
    let program = parse(source).unwrap();
    let mut session = Session::default();
    vitro_native::engine::completion::update_completion_snapshot(&mut session, &program);

    // line 3 = `            ` (空行 inside foo)
    let candidates = get_completion_candidates(&session, source, 3, 12, "bar");
    let bar = candidates
        .iter()
        .find(|c| c.label == "bar" && c.kind == CompletionKind::Function)
        .unwrap();
    assert_eq!(bar.insert_text, "bar()");
    assert!(bar.detail.contains("void"));
}

// ============================================================================
// 实时解析补全测试（利用自研 Parser 错误恢复能力）
// ============================================================================

#[test]
fn test_build_snapshot_from_source_with_incomplete_code() {
    // 代码不完整：函数体未闭合，但 struct/union/typedef/函数声明已可解析
    let source = r#"
        struct Point {
            int x;
            float y;
        };

        union Data {
            int i;
            float f;
        };

        typedef int Integer;

        int global_a;

        int add(int a, int b) {
            return a + b;
        
        void foo() {
            // 未闭合
    "#;

    let snapshot = build_snapshot_from_source(source);

    // 即使代码不完整，Parser 的错误恢复仍能提取已成功解析的符号
    assert!(snapshot.structs.iter().any(|s| s.name == "Point"), "应提取 struct Point");
    let pt = snapshot.structs.iter().find(|s| s.name == "Point").unwrap();
    assert_eq!(pt.fields.len(), 2);
    assert_eq!(pt.fields[0].0, "x");

    assert!(snapshot.unions.iter().any(|u| u.name == "Data"), "应提取 union Data");

    assert!(snapshot.globals.iter().any(|g| g.name == "global_a"), "应提取全局变量 global_a");

    assert!(snapshot.functions.iter().any(|f| f.name == "add"), "应提取函数 add");
    let add = snapshot.functions.iter().find(|f| f.name == "add").unwrap();
    assert_eq!(add.params.len(), 2);
}

#[test]
fn test_completion_member_access_without_compiled_snapshot() {
    // Session 没有任何编译快照（模拟刚打开编辑器/从未编译）
    let session = Session::default();

    // 源码含 struct 定义 + 成员访问，但代码不完整（函数未闭合）
    let source = r#"
        struct Node {
            int val;
            struct Node* next;
        };

        void foo() {
            struct Node n;
            n.
    "#;

    // line 8 = `            n.`
    let candidates = get_completion_candidates(&session, source, 8, 14, "");

    let labels: Vec<String> = candidates.iter().map(|c| c.label.clone()).collect();
    assert!(
        labels.contains(&"val".to_string()),
        "未编译时应通过实时解析提取 struct 字段 val，got: {:?}",
        labels
    );
    assert!(
        labels.contains(&"next".to_string()),
        "未编译时应通过实时解析提取 struct 字段 next，got: {:?}",
        labels
    );
}

#[test]
fn test_completion_expression_with_incomplete_code() {
    let session = Session::default();

    // 代码不完整，但全局函数和变量已定义
    let source = r#"
        int global_count;

        int compute(int x) {
            return x * 2;
        
        void foo() {
            
    "#;

    // line 7 = `            `（foo 函数体内，在 compute 定义之后）
    let candidates = get_completion_candidates(&session, source, 7, 12, "comp");

    let labels: Vec<String> = candidates.iter().map(|c| c.label.clone()).collect();
    assert!(
        labels.contains(&"compute".to_string()),
        "应通过实时解析提取函数 compute，got: {:?}",
        labels
    );
    // global_count 不以 comp 开头，不应出现
    assert!(!labels.contains(&"global_count".to_string()));
}

#[test]
fn test_completion_type_context_with_incomplete_code() {
    let session = Session::default();

    let source = r#"
        struct Point { int x; int y; };
        typedef int Integer;

        void foo() {
            struct 
    "#;

    // line 5 = `            struct ` -> TypePosition 上下文
    let candidates = get_completion_candidates(&session, source, 5, 19, "");

    let labels: Vec<String> = candidates.iter().map(|c| c.label.clone()).collect();
    assert!(
        labels.contains(&"Point".to_string()),
        "类型上下文应通过实时解析提取 struct Point，got: {:?}",
        labels
    );
}

#[test]
fn test_find_variable_type_parses_unsigned_int_param() {
    // B48: 验证带空格的类型名（如 unsigned int）能被正确解析。
    use vitro_native::engine::completion::candidates::find_variable_type;
    use vitro_native::engine::completion::{CompletionSnapshot, SnapshotFunc};

    let snapshot = CompletionSnapshot {
        functions: vec![SnapshotFunc {
            name: "foo".to_string(),
            return_type: "void".to_string(),
            params: vec!["unsigned int x".to_string(), "int* y".to_string()],
            is_static: false,
            filename: String::new(),
        }],
        globals: vec![],
        structs: vec![],
        unions: vec![],
        typedefs: vec![],
        enums: vec![],
        macros: vec![],
    };

    assert_eq!(
        find_variable_type(&snapshot, "", 0, 0, "x"),
        Some("unsigned int".to_string()),
        "unsigned int x 应解析为类型 unsigned int"
    );
    assert_eq!(
        find_variable_type(&snapshot, "", 0, 0, "y"),
        Some("int*".to_string()),
        "int* y 应解析为类型 int*"
    );
}
