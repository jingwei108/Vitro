#![allow(clippy::unwrap_used, clippy::expect_used)]

use vitro_native::compiler::codegen::BytecodeGen;
use vitro_native::compiler::lexer::Lexer;
use vitro_native::compiler::parser::Parser;
use vitro_native::compiler::typeck::TypeChecker;
use vitro_runtime::opcode::OpCode;

fn generate(src: &str) -> vitro_native::compiler::codegen::CompileOutput {
    let (tokens, _) = Lexer::new(src).tokenize();
    let (maybe_program, parse_errors) = Parser::new(tokens).parse();
    assert!(parse_errors.is_empty(), "Parse errors: {:?}", parse_errors);
    let mut program = maybe_program.unwrap();
    let (type_errors, _, _) = TypeChecker::default().check(&mut program);
    assert!(type_errors.is_empty(), "Type errors: {:?}", type_errors);
    let gen = BytecodeGen::new();
    gen.generate(&mut program).unwrap()
}

#[test]
fn test_bytecode_gen_has_main() {
    let output = generate("int main() { return 42; }");
    assert!(output.func_index.contains_key("main"), "Should have main function");
}

#[test]
fn test_bytecode_gen_return_const() {
    let output = generate("int main() { return 42; }");
    let _main_idx = output.func_index["main"];
    let main_meta = &output.func_table["main"];
    // Find PushConst 42 and Return in the bytecode around main function
    let start_ip = main_meta.ip;
    let mut found_push = false;
    let mut found_return = false;
    for i in start_ip..output.code.len() {
        let instr = &output.code[i];
        match instr.op {
            OpCode::PushConst if instr.operand == 42 => found_push = true,
            OpCode::Ret => {
                found_return = true;
                break;
            }
            _ => {}
        }
    }
    assert!(found_push, "Should push 42");
    assert!(found_return, "Should have Return");
}

#[test]
fn test_bytecode_gen_local_var() {
    let output = generate("int main() { int x = 10; return x; }");
    let _main_idx = output.func_index["main"];
    let main_meta = &output.func_table["main"];
    assert!(main_meta.local_count >= 1, "Should have at least 1 local");
}

#[test]
fn test_bytecode_gen_binary_op() {
    let output = generate("int main() { return 1 + 2; }");
    let main_meta = &output.func_table["main"];
    let start_ip = main_meta.ip;
    let mut found_add = false;
    for i in start_ip..output.code.len() {
        if output.code[i].op == OpCode::Add {
            found_add = true;
            break;
        }
    }
    assert!(found_add, "Should have Add instruction");
}

#[test]
fn test_bytecode_gen_if_statement() {
    let output = generate("int main() { if (1) { return 1; } return 0; }");
    let main_meta = &output.func_table["main"];
    let start_ip = main_meta.ip;
    let mut found_jump_if_zero = false;
    let mut found_jump = false;
    for i in start_ip..output.code.len() {
        match output.code[i].op {
            OpCode::JumpIfZero => found_jump_if_zero = true,
            OpCode::Jump => found_jump = true,
            _ => {}
        }
    }
    assert!(found_jump_if_zero, "Should have JumpIfZero for if condition");
    assert!(found_jump, "Should have Jump for then-branch skip");
}

#[test]
fn test_bytecode_gen_while_loop() {
    let output = generate("int main() { while (0) { } return 0; }");
    let main_meta = &output.func_table["main"];
    let start_ip = main_meta.ip;
    let mut found_jump_if_zero = false;
    for i in start_ip..output.code.len() {
        if output.code[i].op == OpCode::JumpIfZero {
            found_jump_if_zero = true;
            break;
        }
    }
    assert!(found_jump_if_zero, "Should have JumpIfZero for while condition");
}

#[test]
fn test_bytecode_gen_function_call() {
    let output = generate("int add(int a, int b) { return a + b; } int main() { return add(1, 2); }");
    assert!(output.func_index.contains_key("add"));
    assert!(output.func_index.contains_key("main"));
    let main_meta = &output.func_table["main"];
    let start_ip = main_meta.ip;
    let mut found_call = false;
    for i in start_ip..output.code.len() {
        if output.code[i].op == OpCode::Call {
            found_call = true;
            break;
        }
    }
    assert!(found_call, "Should have Call instruction");
}

#[test]
fn test_bytecode_gen_global_var() {
    let output = generate("int g = 10; int main() { return g; }");
    assert!(!output.globals_init_32.is_empty(), "Should have global init");
}

#[test]
fn test_bytecode_gen_string_data() {
    let output = generate("int main() { printf(\"hello\"); return 0; }");
    assert!(!output.string_data.is_empty(), "Should have string data");
    assert_eq!(output.string_data[0].1, "hello");
}

#[test]
fn test_bytecode_gen_source_map() {
    let output = generate("int main() { return 0; }");
    assert!(!output.source_map.is_empty(), "Should have source map entries");
}

/// U3#2 红锚（2026-09-14）：变参调用的 double/long long 实参必须经**8 字节
/// 专用槽**中转（StoreLocalD/Q 写 8 字节）。修复前用 `get_temp_slot(0)`（4 字节
/// 槽）+ 占位 slot1 止血——slot0 与 slot1 分配顺序由首次使用决定，不保证相邻，
/// 跨槽写可踩相邻局部变量/其他槽（3 起槽位 bug 同病灶）。
/// 结构判据：同函数内 4 字节槽用户（struct 按值传参的地址临时 StoreLocal）
/// 与变参 8 字节中转（StoreLocalD）**不得共享 offset**——修复前两者都指向
/// slot0，必然相等（红）；修复后变参走 8 字节专用槽，必然不同（绿）。
#[test]
fn test_u3_variadic_64bit_args_use_dedicated_slot() {
    let src = r#"
#include <stdarg.h>
struct S { int a, b; };
int take(struct S s) { return s.a; }
double dv(int n, ...) {
    va_list ap;
    va_start(ap, n);
    double s = 0;
    for (int i = 0; i < n; i++) { s = s + va_arg(ap, double); }
    va_end(ap);
    return s;
}
int take2(struct S s) { return s.b; }
int main() {
    struct S v = {1, 2};
    struct S w;
    w = v;                  // 4 字节槽用户：struct 赋值 src 地址临时 StoreLocal [slot0]
    int a = take2(w);
    double r = dv(1, 1.5);  // 变参 8 字节中转 StoreLocalD（修复前也用 slot0）
    return a + (int)r;
}
"#;
    let output = generate(src);
    let main_meta = &output.func_table["main"];
    let mut local4_targets: Vec<i32> = Vec::new();
    let mut d_targets: Vec<i32> = Vec::new();
    for i in main_meta.ip..output.code.len() {
        let instr = &output.code[i];
        match instr.op {
            OpCode::StoreLocal => local4_targets.push(instr.operand),
            OpCode::StoreLocalD => d_targets.push(instr.operand),
            OpCode::Ret => break,
            _ => {}
        }
    }
    assert!(!d_targets.is_empty(), "变参 double 实参应有 StoreLocalD——测试前提");
    assert!(
        !local4_targets.is_empty(),
        "struct 按值传参应有地址临时 StoreLocal——测试前提"
    );
    for &t in &d_targets {
        assert!(
            !local4_targets.contains(&t),
            "StoreLocalD 目标 {t} 与 4 字节槽用户共享 offset（修复前同指 slot0，8 字节跨槽写病灶）"
        );
    }
}
