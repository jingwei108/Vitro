#![allow(clippy::unwrap_used, clippy::expect_used)]

use vitro_native::compiler::codegen::BytecodeGen;
use vitro_native::compiler::lexer::Lexer;
use vitro_native::compiler::parser::Parser;
use vitro_native::compiler::typeck::TypeChecker;

fn compile_source(source: &str) -> Result<vitro_native::compiler::codegen::CompileOutput, String> {
    let (tokens, lex_errors) = Lexer::new(source).tokenize();
    if !lex_errors.is_empty() {
        return Err(format!("Lexer errors: {:?}", lex_errors));
    }

    let (maybe_program, parse_errors) = Parser::new(tokens).parse();
    if !parse_errors.is_empty() {
        return Err(format!("Parse errors: {:?}", parse_errors));
    }

    let mut program = maybe_program.ok_or("Parser returned None")?;

    let (type_errors, _warnings, _hints) = TypeChecker::default().check(&mut program);
    if !type_errors.is_empty() {
        return Err(format!("Type errors: {:?}", type_errors));
    }

    let gen = BytecodeGen::new();
    gen.generate(&mut program).map_err(|e| format!("BytecodeGen errors: {:?}", e))
}

#[test]
fn test_compile_empty_main() {
    let src = r#"
int main() {
    return 0;
}
"#;
    let result = compile_source(src);
    assert!(result.is_ok(), "{:?}", result.err());
    let output = result.unwrap();
    assert!(!output.code.is_empty());
    assert!(output.func_index.contains_key("main"));
}

#[test]
fn test_compile_hello_world() {
    let src = r#"
#include <stdio.h>
int main() {
    printf("Hello, World!\n");
    return 0;
}
"#;
    let result = compile_source(src);
    assert!(result.is_ok(), "{:?}", result.err());
}

#[test]
fn test_compile_conditional_ifdef() {
    let src = r#"
#define DEBUG
#ifdef DEBUG
int foo() { return 42; }
#else
int foo() { return 0; }
#endif
int main() {
    return foo();
}
"#;
    let result = compile_source(src);
    assert!(result.is_ok(), "{:?}", result.err());
}

#[test]
fn test_compile_conditional_ifndef() {
    let src = r#"
#ifndef RELEASE
int debug_mode = 1;
#endif
int main() {
    return debug_mode;
}
"#;
    let result = compile_source(src);
    assert!(result.is_ok(), "{:?}", result.err());
}

#[test]
fn test_compile_conditional_header_guard() {
    let src = r#"
#ifndef MYHEADER_H
#define MYHEADER_H
int helper() { return 123; }
#endif
int main() {
    return helper();
}
"#;
    let result = compile_source(src);
    assert!(result.is_ok(), "{:?}", result.err());
}

#[test]
fn test_compile_conditional_nested() {
    let src = r#"
#define OUTER
#ifdef OUTER
  #define INNER
  #ifdef INNER
    int val = 1;
  #else
    int val = 2;
  #endif
#else
  int val = 3;
#endif
int main() {
    return val;
}
"#;
    let result = compile_source(src);
    assert!(result.is_ok(), "{:?}", result.err());
}

#[test]
fn test_compile_conditional_skip_block() {
    let src = r#"
#ifdef UNDEFINED
int skipped = 0;
#else
int skipped = 1;
#endif
int main() {
    return skipped;
}
"#;
    let result = compile_source(src);
    assert!(result.is_ok(), "{:?}", result.err());
}

#[test]
fn test_compile_vla_sizeof_type() {
    let src = r#"
#include <stdio.h>
int main() {
    int n = 5;
    printf("%d\n", sizeof(int[n]));
    printf("%d\n", sizeof(int[n][3]));
    int m = 2;
    printf("%d\n", sizeof(int[n][m]));
    return 0;
}
"#;
    let result = compile_source(src);
    assert!(result.is_ok(), "{:?}", result.err());
}

#[test]
fn test_compile_with_vars_and_arithmetic() {
    let src = r#"
int main() {
    int a = 10;
    int b = 20;
    int c = a + b * 2;
    return c;
}
"#;
    let result = compile_source(src);
    assert!(result.is_ok(), "{:?}", result.err());
}

#[test]
fn test_compile_multidim_array() {
    let src = r#"
int main() {
    int arr[3][3];
    arr[1][2] = 5;
    return arr[1][2];
}
"#;
    let result = compile_source(src);
    assert!(result.is_ok(), "{:?}", result.err());
}

#[test]
fn test_compile_struct() {
    let src = r#"
struct Point {
    int x;
    int y;
};
int main() {
    struct Point p;
    p.x = 3;
    p.y = 4;
    return p.x + p.y;
}
"#;
    let result = compile_source(src);
    assert!(result.is_ok(), "{:?}", result.err());
}

#[test]
fn test_compile_if_while_for() {
    let src = r#"
int main() {
    int sum = 0;
    for (int i = 0; i < 5; i = i + 1) {
        sum = sum + i;
    }
    int j = 0;
    while (j < 3) {
        sum = sum + j;
        j = j + 1;
    }
    if (sum > 0) {
        return sum;
    } else {
        return 0;
    }
}
"#;
    let result = compile_source(src);
    assert!(result.is_ok(), "{:?}", result.err());
}

#[test]
fn test_compile_malloc_free() {
    let src = r#"
#include <stdlib.h>
int main() {
    int *p = malloc(sizeof(int));
    *p = 42;
    int val = *p;
    free(p);
    return val;
}
"#;
    let result = compile_source(src);
    assert!(result.is_ok(), "{:?}", result.err());
}

#[test]
fn test_compile_switch() {
    let src = r#"
int main() {
    int x = 2;
    switch (x) {
        case 1: return 10;
        case 2: return 20;
        default: return 0;
    }
}
"#;
    let result = compile_source(src);
    assert!(result.is_ok(), "{:?}", result.err());
}

#[test]
fn test_compile_scanf() {
    let src = r#"
#include <stdio.h>
int main() {
    int a;
    scanf("%d", &a);
    return a;
}
"#;
    let result = compile_source(src);
    assert!(result.is_ok(), "{:?}", result.err());
}

#[test]
fn test_compile_define_macro() {
    let src = r#"
#define N 100
int main() {
    int arr[N];
    return N;
}
"#;
    let result = compile_source(src);
    assert!(result.is_ok(), "{:?}", result.err());
}

#[test]
fn test_type_error_detected() {
    let src = r#"
int main() {
    int x = "hello";
    return 0;
}
"#;
    let (tokens, _lex_errors) = Lexer::new(src).tokenize();
    let (maybe_program, _parse_errors) = Parser::new(tokens).parse();
    let mut program = maybe_program.unwrap();
    let (type_errors, _warnings, _hints) = TypeChecker::default().check(&mut program);
    assert!(!type_errors.is_empty(), "Expected type error for string assigned to int");
}

#[test]
fn test_undeclared_var_detected() {
    let src = r#"
int main() {
    return unknown_var;
}
"#;
    let (tokens, _lex_errors) = Lexer::new(src).tokenize();
    let (maybe_program, _parse_errors) = Parser::new(tokens).parse();
    let mut program = maybe_program.unwrap();
    let (type_errors, _warnings, _hints) = TypeChecker::default().check(&mut program);
    assert!(!type_errors.is_empty(), "Expected type error for undeclared variable");
}

#[test]
fn test_typedef_struct_tree() {
    let src = r#"
typedef struct tree {
    int data;
    struct tree *left;
    struct tree *right;
} Tree;

Tree *createNode(int val) {
    Tree *newtree = (Tree *)malloc(sizeof(Tree));
    if(!newtree){
        return NULL;
    }
    newtree->data = val;
    newtree->left = NULL;
    newtree->right = NULL;
    return newtree;
}

int main() {
    Tree *root = createNode(1);
    return 0;
}
"#;
    let result = compile_source(src);
    assert!(result.is_ok(), "{:?}", result.err());
}

// ── E3：C23 语义级 ──

#[test]
fn test_e3_nullptr_assigns_pointer() {
    let out = compile_source("#include <stdio.h>\nint main(){ int *p = nullptr; return p == (int*)0 ? 0 : 1; }")
        .expect("nullptr 应可编译（与 NULL 同路径，void* 空）");
    assert!(!out.code.is_empty());
}

#[test]
fn test_e3_static_assert_pass_and_fail() {
    let ok = compile_source("#include <stdio.h>\nstatic_assert(1 + 1 == 2, \"math\");\n_Static_assert(sizeof(int) == 4);\nstatic_assert(2 > 1);\nint main(){ return 0; }");
    assert!(ok.is_ok(), "成立的断言应编译通过，实际 {:?}", ok.err());

    let bad = compile_source("static_assert(1 == 2, \"broken\");\nint main(){ return 0; }");
    assert!(bad.is_err(), "失败的断言应编译失败");
    let msg = bad.unwrap_err();
    assert!(msg.contains("static_assert 断言失败"), "{:?}", msg);
}

#[test]
fn test_e3_static_assert_single_arg_and_block_scope_block_support_note() {
    // 单参形态（C23）
    let ok = compile_source("static_assert(42 > 41);\nint main(){ return 0; }");
    assert!(ok.is_ok(), "{:?}", ok.err());
}

#[test]
fn test_e3_constexpr_object() {
    // constexpr 按 const 语义处理（教学子集边界，spec §2.12）
    let out = compile_source("constexpr int N = 42;\nint main(){ return N - 42; }")
        .expect("constexpr 对象应可编译");
    assert!(!out.code.is_empty());
}

#[test]
fn test_e3_attributes_prefix_skip() {
    let out = compile_source("#include <stdio.h>\n[[maybe_unused]] int u = 5;\nint main(){ [[maybe_unused]] int v = 6; printf(\"%d\", u + v); return 0; }")
        .expect("属性前缀应被解析并忽略");
    assert!(!out.code.is_empty());
}
