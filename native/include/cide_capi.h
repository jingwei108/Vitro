#pragma once

#ifdef __cplusplus
extern "C" {
#endif

#ifdef _WIN32
    #ifdef CIDE_EXPORTS
        #define CIDE_API __declspec(dllexport)
    #else
        #define CIDE_API __declspec(dllimport)
    #endif
#else
    #define CIDE_API __attribute__((visibility("default")))
#endif

// Opaque session handle
typedef struct CideSession CideSession;

// ========== 字符串所有权契约（ABI 1.3.0，2026-09-13 W0-3 补齐声明） ==========
//
// 本头文件的字符串出参分三种所有权，函数注释逐一标注：
//
//   [rust-alloc]   返回 rust 分配器分配的 NUL 结尾 UTF-8 缓冲，调用方必须用
//                  cide_free_string 释放。两次调用返回不同指针；内容超过 ~2GB
//                  或分配失败返回 NULL。全部 *_json 出口、cide_abi_version、
//                  cide_engine_version、cide_last_error、cide_get_output_delta、
//                  cide_get_program_output_delta 属于此类。
//
//   [session-loan] 返回会话内部缓冲的借用指针，勿 free、勿缓存——下一次同一
//                  会话上的编译/运行/步进调用可能使其失效。cide_get_compile_errors、
//                  cide_get_runtime_error 属于此类。
//
//   [caller-buffer] 调用方提供缓冲区 + 长度，引擎拷贝写入（len+buf 出参模式），
//                  无所有权转移。cide_get_output* 族属于此类。
//
// 完整契约文档：docs/current/06-出口与协议/CAPI评审回复与实现状态.md

// ========== 版本与能力 ==========

/// ABI 版本串（语义化版本，如 "1.3.0"）。[rust-alloc]
CIDE_API char* cide_abi_version(void);

/// 引擎版本串：crate 版本 + 构建期 git hash（如 "0.1.0 (5955cb9)"）。
/// 消费方据此做产物新鲜度自检。[rust-alloc]
CIDE_API char* cide_engine_version(void);

/// 引擎能力清单 JSON（语言锚点/预处理器能力/内存模型/schema 版本与 v0.2
/// 台账/行为契约，机器可读）。[rust-alloc]
/// ABI 1.3.0 起为 rust-alloc 所有权（此前为静态指针、勿释放——按旧注释
/// 缓存指针的下游需改为每次取用即取即放）。
CIDE_API char* cide_get_capabilities_json(void);

/// 释放任何 [rust-alloc] 出参缓冲。null 安全。
CIDE_API void cide_free_string(char* p);

/// 最近一次错误 JSON：{"kind":"compile|runtime|none","message":"..."}。[rust-alloc]
CIDE_API char* cide_last_error(CideSession* s);

// ========== 会话管理 ==========

CIDE_API CideSession* cide_session_create();
CIDE_API void cide_session_destroy(CideSession* s);

// ========== 错误码 ==========
/// C-compatible error code enumeration. Keep in sync with native/src/diagnostics/error_codes.rs.

typedef enum {
    CIDE_E1001_UnknownChar        = 1001,
    CIDE_E1002_UnterminatedString = 1002,
    CIDE_E1003_StringCrossLine    = 1003,
    CIDE_E1004_UnsupportedOp      = 1004,
    CIDE_E1005_InvalidDefine      = 1005,
    CIDE_E1006_UnsupportedFeature = 1006,
    CIDE_E1007_ComplexDeclarator = 1007,
    CIDE_E1010_UnterminatedComment = 1010,

    CIDE_E2001_ExpectedType       = 2001,
    CIDE_E2002_ExpectedArraySize  = 2002,
    CIDE_E2003_ExpectedExpr       = 2003,
    CIDE_E2004_ExpectedCaseOrDefault = 2004,
    CIDE_E2005_ExpectedSemicolon  = 2005,
    CIDE_E2006_ExpectedClosingBrace = 2006,
    CIDE_E2007_ExpectedClosingParen = 2007,
    CIDE_E2008_ExpectedClosingBracket = 2008,

    CIDE_E3001_VarRedeclared      = 3001,
    CIDE_E3002_StructRedeclared   = 3002,
    CIDE_E3003_FuncRedeclared     = 3003,
    CIDE_E3004_TypeMismatch       = 3004,
    CIDE_E3005_ArrayInitTooMany   = 3005,
    CIDE_E3006_ArrayInitTypeMismatch = 3006,
    CIDE_E3007_StringInitNonCharArray = 3007,
    CIDE_E3008_StringTooLong      = 3008,
    CIDE_E3009_InvalidArrayInit   = 3009,
    CIDE_E3010_BreakOutsideLoop   = 3010,
    CIDE_E3011_ContinueOutsideLoop = 3011,
    CIDE_E3012_VoidFuncReturnValue = 3012,
    CIDE_E3013_MissingReturnValue = 3013,
    CIDE_E3014_ReturnTypeMismatch = 3014,
    CIDE_E3015_InvalidCondition   = 3015,
    CIDE_E3016_ArithmeticTypeError = 3016,
    CIDE_E3017_ComparisonTypeError = 3017,
    CIDE_E3018_RelationTypeError  = 3018,
    CIDE_E3019_LogicTypeError     = 3019,
    CIDE_E3020_UnaryTypeError     = 3020,
    CIDE_E3021_DerefNonPointer    = 3021,
    CIDE_E3022_IncDecTypeError    = 3022,
    CIDE_E3023_UndeclaredVar      = 3023,
    CIDE_E3024_MallocArgCount     = 3024,
    CIDE_E3025_MallocArgType      = 3025,
    CIDE_E3026_FreeArgCount       = 3026,
    CIDE_E3027_FreeArgType        = 3027,
    CIDE_E3028_BuiltInArgCount    = 3028,
    CIDE_E3029_BuiltInArgType     = 3029,
    CIDE_E3030_PrintfArgCount     = 3030,
    CIDE_E3031_PrintfFirstArg     = 3031,
    CIDE_E3032_PrintfArgType      = 3032,
    CIDE_E3033_ScanfArgCount      = 3033,
    CIDE_E3034_ScanfFirstArg      = 3034,
    CIDE_E3035_ScanfArgType       = 3035,
    CIDE_E3036_UndefinedFunc      = 3036,
    CIDE_E3037_FuncArgCount       = 3037,
    CIDE_E3038_FuncArgType        = 3038,
    CIDE_E3039_ArrayIndexType     = 3039,
    CIDE_E3040_IndexNonArray      = 3040,
    CIDE_E3041_MemberNonStruct    = 3041,
    CIDE_E3042_UnknownMember      = 3042,
    CIDE_E3043_AssignToRValue     = 3043,
    CIDE_E3044_AssignTypeMismatch = 3044,
    CIDE_E3045_CompoundAssignType = 3045,
    CIDE_E3046_SwitchCondType     = 3046,
    CIDE_E3047_CaseNotConstant    = 3047,
    CIDE_E3048_BitOpTypeError     = 3048,
    CIDE_E3049_AssignToConst      = 3049,

    CIDE_W3050_AssignInCondition  = 3050,
    CIDE_W3051_ArrayBoundOffByOne = 3051,
    CIDE_W3052_ArrayToPointerDecay = 3052,
    CIDE_W3053_ImplicitScalarConversion = 3053,
    CIDE_W3054_IntToPointerCast   = 3054,
    CIDE_W3055_VoidPointerCast    = 3055,
    CIDE_W3056_UnsignedToInt      = 3056,
    CIDE_H3057_ImplicitConversionHint = 3057,
} CideErrorCode;

// ========== 错误码表导出（下游需求清单 B1）==========

/// Export the full error catalog as a rust-alloc JSON string:
///   {"catalog":[{code,code_str,lang,category,emoji,title,explanation,common_causes[]}]}
/// Entries are sorted ascending by `code` (stable across builds). Static metadata
/// only; the per-source-line `fix_suggestion` is delivered by the compile diagnostics.
/// The caller owns the returned buffer and MUST release it with cide_free_string.
/// Returns NULL on allocation failure.
CIDE_API char* cide_get_error_catalog_json(void);

// ========== 编译 ==========

/// Compile C source code. Returns 0 on success, -1 on error.
/// This clears any previously added compile units.
/// Note: The `source` string pointer is only valid for the duration of this call.
CIDE_API int cide_compile(CideSession* s, const char* source);

/// Add a compile unit (multi-file support). Does not compile yet.
CIDE_API int cide_compile_unit(CideSession* s, const char* filename, const char* source);

/// Compile all added units. Returns 0 on success, -1 on error.
CIDE_API int cide_compile_all(CideSession* s);

/// Get compilation errors as a UTF-8 string. Returns nullptr if no errors.
/// Note: The returned pointer may become invalid after the next compile call.
CIDE_API const char* cide_get_compile_errors(CideSession* s);

/// Byte length (excluding NUL) of the compile-errors JSON that
/// cide_get_compile_errors would return; 0 when no errors. Companion of
/// cide_get_compile_errors for exact-length buffer reads (ABI 1.2.0).
CIDE_API int cide_get_compile_errors_length(CideSession* s);

/// Compile diagnostics as a single JSON string (same payload as
/// cide_get_compile_errors). [rust-alloc]
CIDE_API char* cide_compile_json(CideSession* s);

// ========== 命令行参数 ==========

/// Set command-line arguments for `main(int argc, char *argv[])`.
CIDE_API void cide_set_argv(CideSession* s, int argc, const char** argv);

// ========== 执行 ==========

/// Run the compiled program. Returns 0 on success, -1 on runtime error.
CIDE_API int cide_run(CideSession* s);

/// Run the compiled program; result + runtime error + output length summary
/// as one JSON string. [rust-alloc]
CIDE_API char* cide_run_json(CideSession* s);

/// Get runtime error message. Returns nullptr if no error.
/// Note: The returned pointer may become invalid after the next run/step call.
CIDE_API const char* cide_get_runtime_error(CideSession* s);

// ========== 步进调试（统一模式） ==========

/// Initialize the unified (time-travel) engine for stepping. Returns 0 on
/// success, non-zero on error. Required before step_next_json / seek /
/// payloads / breakpoints.
CIDE_API int cide_step_begin(CideSession* s);

/// Execute one step and return the step payload as a JSON string.
/// [rust-alloc] Returns {"status":"..."} frames including finished/trap.
CIDE_API char* cide_step_next_json(CideSession* s);

/// Collect step payloads for a step range as a JSON string (visible window
/// only). Out-of-window / negative ranges yield an empty list, never panic.
/// [rust-alloc]
CIDE_API char* cide_get_step_payloads_json(CideSession* s, int start, int end);

/// Replace the breakpoint line set. `lines_json` is a JSON array of line
/// numbers, e.g. "[3,7]". Returns 0 on success.
CIDE_API int cide_set_breakpoints(CideSession* s, const char* lines_json);

// ========== 执行配置（会话级） ==========

/// Cap total executed steps (teaching fuse against infinite loops).
/// Returns the applied value (negative input is rejected with the old value).
CIDE_API int cide_set_max_steps(CideSession* s, int max_steps);

/// Cap call depth (V-P1-10; teaching fuse against runaway recursion).
CIDE_API int cide_set_call_depth_limit(CideSession* s, int depth);

/// Deterministic mode switch (rand sequence reset per run). 1 = on.
CIDE_API int cide_set_deterministic(CideSession* s, int on);

/// Query deterministic mode (1 = on, 0 = off).
CIDE_API int cide_get_deterministic(CideSession* s);

/// Heap quarantine budget in bytes (UAF detection window; see
/// 堆有界隔离决议.md). Returns the applied value.
CIDE_API int cide_set_quarantine_budget(CideSession* s, int budget_bytes);

/// Query the heap quarantine budget in bytes.
CIDE_API int cide_get_quarantine_budget(CideSession* s);

// ========== JIT 统计 ==========

/// Report JIT statistics: traces compiled and steps accelerated.
/// Pure out-parameters; writes 0/0 when unavailable.
CIDE_API void cide_get_jit_stats(CideSession* s, int* traces_compiled, int* steps_accelerated);

// ========== 输入 ==========

/// Set input lines for scanf (newline-separated lines).
CIDE_API void cide_set_input(CideSession* s, const char* input);

/// Set input mode: 0 = interactive (default), non-zero = batch.
/// In batch mode getchar returns EOF immediately when input is exhausted.
CIDE_API void cide_set_input_mode(CideSession* s, int is_batch);

/// Returns 1 if the program is waiting for input, 0 otherwise.
CIDE_API int cide_is_waiting_input(CideSession* s);

/// Provide a single input line and resume execution.
CIDE_API int cide_provide_input_line(CideSession* s, const char* line);

// ========== 输出 ==========

/// Get the length of the console output (display view: program stdout/stderr +
/// engine notes, in write order). For the program's own stdout only, use
/// cide_get_program_output_length.
CIDE_API int cide_get_output_length(CideSession* s);

/// Copy console output into the provided buffer (max_len includes null terminator).
CIDE_API void cide_get_output(CideSession* s, char* buf, int max_len);

/// Get the length of the program's own stdout (excludes engine notes and stderr).
/// E-P1-5: this is the only legitimate source for comparing against a Clang
/// golden or for grading.
CIDE_API int cide_get_program_output_length(CideSession* s);

/// Copy the program's own stdout into the provided buffer.
CIDE_API void cide_get_program_output(CideSession* s, char* buf, int max_len);

/// Get the length of engine notes (completion message, leak report, teaching hints).
CIDE_API int cide_get_engine_notes_length(CideSession* s);

/// Copy engine notes into the provided buffer.
CIDE_API void cide_get_engine_notes(CideSession* s, char* buf, int max_len);

/// Incremental output since a byte cursor, as a JSON string (rust-alloc, free with
/// cide_free_string). Returns {"delta":..,"cursor":..,"total":..,"stream":"stdout"}.
CIDE_API char* cide_get_program_output_delta(CideSession* s, int cursor);

/// Incremental display-view output since a byte cursor, as a JSON string.
/// [rust-alloc] Same cursor protocol as cide_get_program_output_delta.
CIDE_API char* cide_get_output_delta(CideSession* s, int cursor);

#ifdef __cplusplus
}
#endif
