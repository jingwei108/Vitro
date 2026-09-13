#![allow(clippy::unwrap_used, clippy::expect_used)]
#![allow(dead_code)]

use std::collections::HashMap;
use std::ffi::{c_char, CString};

use vitro_native::compiler::codegen::{BytecodeGen, CompileOutput};
use vitro_native::compiler::lexer::Lexer;
use vitro_native::compiler::parser::Parser;
use vitro_native::compiler::typeck::TypeChecker;
use vitro_runtime::instruction::Instruction;
use vitro_runtime::opcode::OpCode;

/// Compile C++ source code directly through the compiler pipeline and return the CompileOutput.
/// This is a white-box helper for dogfooding tests.
pub fn compile_cpp_bytecode(source: &str) -> Result<CompileOutput, String> {
    let (tokens, lex_errors) = Lexer::with_mode(source, true).tokenize();
    if !lex_errors.is_empty() {
        return Err(format!("Lexer errors: {:?}", lex_errors));
    }

    let (maybe_program, parse_errors) = Parser::with_mode(tokens, true).parse();
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

/// Extract the instruction slice for a named function from a CompileOutput.
/// Returns `(start_ip, instructions)` where `instructions` is a cloned Vec of the function body.
pub fn get_function_instructions(output: &CompileOutput, func_name: &str) -> Option<(usize, Vec<Instruction>)> {
    let meta = output.func_table.get(func_name)?;
    let start_ip = meta.ip;

    // Compute end IP by finding the next function's start IP.
    let mut ips: Vec<usize> = output.func_table.values().map(|m| m.ip).collect();
    ips.sort();
    let mut end_ip = ips.iter().find(|&&ip| ip > start_ip).copied().unwrap_or(output.code.len());

    // If this is the last function, exclude trailing startup code (Call(main), Ret)
    // that the VM appends after all user functions.
    if end_ip == output.code.len() && output.code.len() >= 2 {
        if let Some(&main_idx) = output.func_index.get("main") {
            let second_last = &output.code[output.code.len() - 2];
            let last = &output.code[output.code.len() - 1];
            if second_last.op == OpCode::Call && second_last.operand == main_idx && last.op == OpCode::Ret {
                end_ip -= 2;
            }
        }
    }

    Some((start_ip, output.code[start_ip..end_ip].to_vec()))
}

/// Normalized operand for semantic bytecode comparison.
#[derive(Debug, Clone, PartialEq)]
pub enum NormalizedOperand {
    Int(i32),
    FuncName(String),
}

/// Normalize a function's instruction slice for comparison.
///
/// - `Jump` / `JumpIfZero` / `JumpIfNotZero`: absolute IP operands are converted to
///   relative offsets from the current instruction index (relative to function start).
/// - `Call`: function index operand is converted to the function name via `func_index`.
/// - `StepEvent`: source line numbers are normalized to 0 (debug events should not affect
///   semantic equivalence).
/// - All other operands are kept as-is.
fn normalize_instructions(
    instrs: &[Instruction],
    start_ip: usize,
    func_index: &HashMap<String, i32>,
) -> Vec<(OpCode, NormalizedOperand)> {
    let rev_index: HashMap<i32, String> = func_index.iter().map(|(k, v)| (*v, k.clone())).collect();

    instrs
        .iter()
        .enumerate()
        .map(|(idx, instr)| {
            let current_abs_ip = start_ip + idx;
            match instr.op {
                OpCode::StepEvent => (instr.op, NormalizedOperand::Int(0)),
                OpCode::Jump | OpCode::JumpIfZero | OpCode::JumpIfNotZero => {
                    let abs_target = instr.operand as usize;
                    let rel = abs_target as i32 - current_abs_ip as i32;
                    (instr.op, NormalizedOperand::Int(rel))
                }
                OpCode::Call => {
                    if let Some(name) = rev_index.get(&instr.operand) {
                        (instr.op, NormalizedOperand::FuncName(name.clone()))
                    } else {
                        (instr.op, NormalizedOperand::Int(instr.operand))
                    }
                }
                _ => (instr.op, NormalizedOperand::Int(instr.operand)),
            }
        })
        .collect()
}

/// Format a normalized instruction slice as human-readable assembly text.
pub fn display_normalized_slice(slice: &[(OpCode, NormalizedOperand)]) -> String {
    slice
        .iter()
        .enumerate()
        .map(|(idx, (op, operand))| {
            let op_str = format!("{:?}", op);
            let operand_str = match operand {
                NormalizedOperand::Int(v) => v.to_string(),
                NormalizedOperand::FuncName(name) => format!("fn:{}", name),
            };
            format!("{:03}: {:20} {}", idx, op_str, operand_str)
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Assert that two CompileOutputs contain semantically equivalent bytecode for the given function.
///
/// Panics with a detailed diff on mismatch.
pub fn assert_bytecode_equivalent(actual_output: &CompileOutput, expected_output: &CompileOutput, func_name: &str) {
    assert_bytecode_equivalent_named(actual_output, func_name, expected_output, func_name);
}

/// Assert that two CompileOutputs contain semantically equivalent bytecode for the given functions,
/// allowing the actual and expected functions to have different names.
///
/// Panics with a detailed diff on mismatch.
pub fn assert_bytecode_equivalent_named(
    actual_output: &CompileOutput,
    actual_name: &str,
    expected_output: &CompileOutput,
    expected_name: &str,
) {
    let (actual_start, actual_instrs) = get_function_instructions(actual_output, actual_name)
        .unwrap_or_else(|| panic!("Function '{}' not found in actual output", actual_name));
    let (expected_start, expected_instrs) = get_function_instructions(expected_output, expected_name)
        .unwrap_or_else(|| panic!("Function '{}' not found in expected output", expected_name));

    let actual_norm = normalize_instructions(&actual_instrs, actual_start, &actual_output.func_index);
    let expected_norm = normalize_instructions(&expected_instrs, expected_start, &expected_output.func_index);

    if actual_norm != expected_norm {
        let diff = format_diff(&actual_norm, &expected_norm);
        panic!(
            "Bytecode mismatch (actual '{}' vs expected '{}'):\n--- actual\n{}\n--- expected\n{}\n--- diff\n{}",
            actual_name,
            expected_name,
            display_normalized_slice(&actual_norm),
            display_normalized_slice(&expected_norm),
            diff
        );
    }
}

fn format_diff(actual: &[(OpCode, NormalizedOperand)], expected: &[(OpCode, NormalizedOperand)]) -> String {
    let max_len = actual.len().max(expected.len());
    let mut lines = Vec::new();
    for i in 0..max_len {
        let a = actual.get(i);
        let e = expected.get(i);
        match (a, e) {
            (Some(a_instr), Some(e_instr)) if a_instr == e_instr => {
                lines.push(format!("{:03}:  {:?}", i, a_instr));
            }
            (Some(a_instr), Some(e_instr)) => {
                lines.push(format!("{:03}: - {:?}", i, a_instr));
                lines.push(format!("{:03}: + {:?}", i, e_instr));
            }
            (Some(a_instr), None) => {
                lines.push(format!("{:03}: - {:?}", i, a_instr));
            }
            (None, Some(e_instr)) => {
                lines.push(format!("{:03}: + {:?}", i, e_instr));
            }
            (None, None) => break,
        }
    }
    lines.join("\n")
}

/// Compile and run C++ source code through the full C API pipeline (includes VM, libc, containers).
/// Returns `(exit_code, stdout_lines)`.
pub fn compile_and_run_cpp(source: &str) -> Result<(i32, Vec<String>), String> {
    unsafe {
        let session = vitro_native::capi::vitro_session_create();
        if session.is_null() {
            return Err("Failed to create session".to_string());
        }

        let src = CString::new(source).map_err(|e| e.to_string())?;
        let fname = CString::new("main.cpp").map_err(|e| e.to_string())?;
        vitro_native::capi::vitro_compile_unit(session, fname.as_ptr() as *const c_char, src.as_ptr() as *const c_char);

        let compile_ret = vitro_native::capi::vitro_compile_all(session);
        if compile_ret != 0 {
            let err_ptr = vitro_native::capi::vitro_get_compile_errors(session);
            let err_msg = if err_ptr.is_null() {
                "Unknown compile error".to_string()
            } else {
                std::ffi::CStr::from_ptr(err_ptr).to_string_lossy().to_string()
            };
            vitro_native::capi::vitro_session_destroy(session);
            return Err(err_msg);
        }

        let run_ret = vitro_native::capi::vitro_run(session);

        // E-P1-5：直接读纯程序 stdout 通道（引擎附注走 note 通道），不再做文本清洗。
        let mut outputs = Vec::new();
        let out_len = vitro_native::capi::vitro_get_program_output_length(session);
        if out_len > 0 {
            let mut buf = vec![0u8; out_len as usize + 1];
            vitro_native::capi::vitro_get_program_output(session, buf.as_mut_ptr() as *mut c_char, buf.len() as i32);
            let out_str = String::from_utf8_lossy(&buf);
            for line in out_str.lines() {
                let trimmed = line.trim_matches('\0');
                if !trimmed.is_empty() {
                    outputs.push(trimmed.to_string());
                }
            }
        }

        let err_ptr = vitro_native::capi::vitro_get_runtime_error(session);
        let runtime_err = if err_ptr.is_null() {
            None
        } else {
            Some(std::ffi::CStr::from_ptr(err_ptr).to_string_lossy().to_string())
        };

        vitro_native::capi::vitro_session_destroy(session);

        if let Some(e) = runtime_err {
            if !e.is_empty() {
                return Err(format!("Runtime error: {}", e));
            }
        }

        Ok((run_ret, outputs))
    }
}
