#pragma once

#ifdef __cplusplus
extern "C" {
#endif

#ifdef _WIN32
    #ifdef VITRO_EXPORTS
        #define VITRO_API __declspec(dllexport)
    #else
        #define VITRO_API __declspec(dllimport)
    #endif
#else
    #define VITRO_API __attribute__((visibility("default")))
#endif

// Opaque session handle
typedef struct VitroSession VitroSession;

// ========== 字符串所有权契约（ABI 1.3.0，2026-09-13 W0-3 补齐声明） ==========
//
// 本头文件的字符串出参分三种所有权，函数注释逐一标注：
//
//   [rust-alloc]   返回 rust 分配器分配的 NUL 结尾 UTF-8 缓冲，调用方必须用
//                  vitro_free_string 释放。两次调用返回不同指针；内容超过 ~2GB
//                  或分配失败返回 NULL。全部 *_json 出口、vitro_abi_version、
//                  vitro_engine_version、vitro_last_error、vitro_get_output_delta、
//                  vitro_get_program_output_delta 属于此类。
//
//   [session-loan] 返回会话内部缓冲的借用指针，勿 free、勿缓存——下一次同一
//                  会话上的编译/运行/步进调用可能使其失效。vitro_get_compile_errors、
//                  vitro_get_runtime_error 属于此类。
//
//   [caller-buffer] 调用方提供缓冲区 + 长度，引擎拷贝写入（len+buf 出参模式），
//                  无所有权转移。vitro_get_output* 族属于此类。
//
// 完整契约文档：docs/current/06-出口与协议/CAPI评审回复与实现状态.md

// ========== 版本与能力 ==========

/// ABI 版本串（语义化版本，如 "1.3.0"）。[rust-alloc]
VITRO_API char* vitro_abi_version(void);

/// 引擎版本串：crate 版本 + 构建期 git hash（如 "0.1.0 (5955cb9)"）。
/// 消费方据此做产物新鲜度自检。[rust-alloc]
VITRO_API char* vitro_engine_version(void);

/// 引擎能力清单 JSON（语言锚点/预处理器能力/内存模型/schema 版本与 v0.2
/// 台账/行为契约，机器可读）。[rust-alloc]
/// ABI 1.3.0 起为 rust-alloc 所有权（此前为静态指针、勿释放——按旧注释
/// 缓存指针的下游需改为每次取用即取即放）。
VITRO_API char* vitro_get_capabilities_json(void);

/// 释放任何 [rust-alloc] 出参缓冲。null 安全。
VITRO_API void vitro_free_string(char* p);

/// 最近一次错误 JSON：{"kind":"compile|runtime|none","message":"..."}。[rust-alloc]
VITRO_API char* vitro_last_error(VitroSession* s);

// ========== 会话管理 ==========

VITRO_API VitroSession* vitro_session_create();
VITRO_API void vitro_session_destroy(VitroSession* s);

// ========== 错误码 ==========
/// C-compatible error code enumeration. Keep in sync with native/src/diagnostics/error_codes.rs.

typedef enum {
    VITRO_E1001_UnknownChar        = 1001,
    VITRO_E1002_UnterminatedString = 1002,
    VITRO_E1003_StringCrossLine    = 1003,
    VITRO_E1004_UnsupportedOp      = 1004,
    VITRO_E1005_InvalidDefine      = 1005,
    VITRO_E1006_UnsupportedFeature = 1006,
    VITRO_E1007_ComplexDeclarator = 1007,
    VITRO_E1010_UnterminatedComment = 1010,

    VITRO_E2001_ExpectedType       = 2001,
    VITRO_E2002_ExpectedArraySize  = 2002,
    VITRO_E2003_ExpectedExpr       = 2003,
    VITRO_E2004_ExpectedCaseOrDefault = 2004,
    VITRO_E2005_ExpectedSemicolon  = 2005,
    VITRO_E2006_ExpectedClosingBrace = 2006,
    VITRO_E2007_ExpectedClosingParen = 2007,
    VITRO_E2008_ExpectedClosingBracket = 2008,

    VITRO_E3001_VarRedeclared      = 3001,
    VITRO_E3002_StructRedeclared   = 3002,
    VITRO_E3003_FuncRedeclared     = 3003,
    VITRO_E3004_TypeMismatch       = 3004,
    VITRO_E3005_ArrayInitTooMany   = 3005,
    VITRO_E3006_ArrayInitTypeMismatch = 3006,
    VITRO_E3007_StringInitNonCharArray = 3007,
    VITRO_E3008_StringTooLong      = 3008,
    VITRO_E3009_InvalidArrayInit   = 3009,
    VITRO_E3010_BreakOutsideLoop   = 3010,
    VITRO_E3011_ContinueOutsideLoop = 3011,
    VITRO_E3012_VoidFuncReturnValue = 3012,
    VITRO_E3013_MissingReturnValue = 3013,
    VITRO_E3014_ReturnTypeMismatch = 3014,
    VITRO_E3015_InvalidCondition   = 3015,
    VITRO_E3016_ArithmeticTypeError = 3016,
    VITRO_E3017_ComparisonTypeError = 3017,
    VITRO_E3018_RelationTypeError  = 3018,
    VITRO_E3019_LogicTypeError     = 3019,
    VITRO_E3020_UnaryTypeError     = 3020,
    VITRO_E3021_DerefNonPointer    = 3021,
    VITRO_E3022_IncDecTypeError    = 3022,
    VITRO_E3023_UndeclaredVar      = 3023,
    VITRO_E3024_MallocArgCount     = 3024,
    VITRO_E3025_MallocArgType      = 3025,
    VITRO_E3026_FreeArgCount       = 3026,
    VITRO_E3027_FreeArgType        = 3027,
    VITRO_E3028_BuiltInArgCount    = 3028,
    VITRO_E3029_BuiltInArgType     = 3029,
    VITRO_E3030_PrintfArgCount     = 3030,
    VITRO_E3031_PrintfFirstArg     = 3031,
    VITRO_E3032_PrintfArgType      = 3032,
    VITRO_E3033_ScanfArgCount      = 3033,
    VITRO_E3034_ScanfFirstArg      = 3034,
    VITRO_E3035_ScanfArgType       = 3035,
    VITRO_E3036_UndefinedFunc      = 3036,
    VITRO_E3037_FuncArgCount       = 3037,
    VITRO_E3038_FuncArgType        = 3038,
    VITRO_E3039_ArrayIndexType     = 3039,
    VITRO_E3040_IndexNonArray      = 3040,
    VITRO_E3041_MemberNonStruct    = 3041,
    VITRO_E3042_UnknownMember      = 3042,
    VITRO_E3043_AssignToRValue     = 3043,
    VITRO_E3044_AssignTypeMismatch = 3044,
    VITRO_E3045_CompoundAssignType = 3045,
    VITRO_E3046_SwitchCondType     = 3046,
    VITRO_E3047_CaseNotConstant    = 3047,
    VITRO_E3048_BitOpTypeError     = 3048,
    VITRO_E3049_AssignToConst      = 3049,

    VITRO_W3050_AssignInCondition  = 3050,
    VITRO_W3051_ArrayBoundOffByOne = 3051,
    VITRO_W3052_ArrayToPointerDecay = 3052,
    VITRO_W3053_ImplicitScalarConversion = 3053,
    VITRO_W3054_IntToPointerCast   = 3054,
    VITRO_W3055_VoidPointerCast    = 3055,
    VITRO_W3056_UnsignedToInt      = 3056,
    VITRO_H3057_ImplicitConversionHint = 3057,
} VitroErrorCode;

// ========== 错误码表导出（下游需求清单 B1）==========

/// Export the full error catalog as a rust-alloc JSON string:
///   {"catalog":[{code,code_str,lang,category,emoji,title,explanation,common_causes[]}]}
/// Entries are sorted ascending by `code` (stable across builds). Static metadata
/// only; the per-source-line `fix_suggestion` is delivered by the compile diagnostics.
/// The caller owns the returned buffer and MUST release it with vitro_free_string.
/// Returns NULL on allocation failure.
VITRO_API char* vitro_get_error_catalog_json(void);

// ========== 编译 ==========

/// Compile C source code. Returns 0 on success, -1 on error.
/// This clears any previously added compile units.
/// Note: The `source` string pointer is only valid for the duration of this call.
VITRO_API int vitro_compile(VitroSession* s, const char* source);

/// Add a compile unit (multi-file support). Does not compile yet.
VITRO_API int vitro_compile_unit(VitroSession* s, const char* filename, const char* source);

/// Compile all added units. Returns 0 on success, -1 on error.
VITRO_API int vitro_compile_all(VitroSession* s);

/// Get compilation errors as a UTF-8 string. Returns nullptr if no errors.
/// Note: The returned pointer may become invalid after the next compile call.
VITRO_API const char* vitro_get_compile_errors(VitroSession* s);

/// Byte length (excluding NUL) of the compile-errors JSON that
/// vitro_get_compile_errors would return; 0 when no errors. Companion of
/// vitro_get_compile_errors for exact-length buffer reads (ABI 1.2.0).
VITRO_API int vitro_get_compile_errors_length(VitroSession* s);

/// Compile diagnostics as a single JSON string (same payload as
/// vitro_get_compile_errors). [rust-alloc]
VITRO_API char* vitro_compile_json(VitroSession* s);

// ========== 命令行参数 ==========

/// Set command-line arguments for `main(int argc, char *argv[])`.
VITRO_API void vitro_set_argv(VitroSession* s, int argc, const char** argv);

// ========== 执行 ==========

/// Run the compiled program. Returns 0 on success, -1 on runtime error.
VITRO_API int vitro_run(VitroSession* s);

/// Run the compiled program; result + runtime error + output length summary
/// as one JSON string. [rust-alloc]
VITRO_API char* vitro_run_json(VitroSession* s);

/// Get runtime error message. Returns nullptr if no error.
/// Note: The returned pointer may become invalid after the next run/step call.
VITRO_API const char* vitro_get_runtime_error(VitroSession* s);

// ========== 步进调试（统一模式） ==========

/// Initialize the unified (time-travel) engine for stepping. Returns 0 on
/// success, non-zero on error. Required before step_next_json / seek /
/// payloads / breakpoints.
VITRO_API int vitro_step_begin(VitroSession* s);

/// Execute one step and return the step payload as a JSON string.
/// [rust-alloc] Returns {"status":"..."} frames including finished/trap.
VITRO_API char* vitro_step_next_json(VitroSession* s);

/// Collect step payloads for a step range as a JSON string (visible window
/// only). Out-of-window / negative ranges yield an empty list, never panic.
/// [rust-alloc]
VITRO_API char* vitro_get_step_payloads_json(VitroSession* s, int start, int end);

/// Replace the breakpoint line set. `lines_json` is a JSON array of line
/// numbers, e.g. "[3,7]". Returns 0 on success.
VITRO_API int vitro_set_breakpoints(VitroSession* s, const char* lines_json);

// ========== 执行配置（会话级） ==========

/// Cap total executed steps (teaching fuse against infinite loops).
/// Returns the applied value (negative input is rejected with the old value).
VITRO_API int vitro_set_max_steps(VitroSession* s, int max_steps);

/// Cap call depth (V-P1-10; teaching fuse against runaway recursion).
VITRO_API int vitro_set_call_depth_limit(VitroSession* s, int depth);

/// Deterministic mode switch (rand sequence reset per run). 1 = on.
VITRO_API int vitro_set_deterministic(VitroSession* s, int on);

/// Query deterministic mode (1 = on, 0 = off).
VITRO_API int vitro_get_deterministic(VitroSession* s);

/// Heap quarantine budget in bytes (UAF detection window; see
/// 堆有界隔离决议.md). Returns the applied value.
VITRO_API int vitro_set_quarantine_budget(VitroSession* s, int budget_bytes);

/// Query the heap quarantine budget in bytes.
VITRO_API int vitro_get_quarantine_budget(VitroSession* s);

// ========== JIT 统计 ==========

/// Report JIT statistics: traces compiled and steps accelerated.
/// Pure out-parameters; writes 0/0 when unavailable.
VITRO_API void vitro_get_jit_stats(VitroSession* s, int* traces_compiled, int* steps_accelerated);

// ========== 输入 ==========

/// Set input lines for scanf (newline-separated lines).
VITRO_API void vitro_set_input(VitroSession* s, const char* input);

/// Set input mode: 0 = interactive (default), non-zero = batch.
/// In batch mode getchar returns EOF immediately when input is exhausted.
VITRO_API void vitro_set_input_mode(VitroSession* s, int is_batch);

/// Returns 1 if the program is waiting for input, 0 otherwise.
VITRO_API int vitro_is_waiting_input(VitroSession* s);

/// Provide a single input line and resume execution.
VITRO_API int vitro_provide_input_line(VitroSession* s, const char* line);

// ========== 输出 ==========

/// Get the length of the console output (display view: program stdout/stderr +
/// engine notes, in write order). For the program's own stdout only, use
/// vitro_get_program_output_length.
VITRO_API int vitro_get_output_length(VitroSession* s);

/// Copy console output into the provided buffer (max_len includes null terminator).
VITRO_API void vitro_get_output(VitroSession* s, char* buf, int max_len);

/// Get the length of the program's own stdout (excludes engine notes and stderr).
/// E-P1-5: this is the only legitimate source for comparing against a Clang
/// golden or for grading.
VITRO_API int vitro_get_program_output_length(VitroSession* s);

/// Copy the program's own stdout into the provided buffer.
VITRO_API void vitro_get_program_output(VitroSession* s, char* buf, int max_len);

/// Get the length of engine notes (completion message, leak report, teaching hints).
VITRO_API int vitro_get_engine_notes_length(VitroSession* s);

/// Copy engine notes into the provided buffer.
VITRO_API void vitro_get_engine_notes(VitroSession* s, char* buf, int max_len);

/// Incremental output since a byte cursor, as a JSON string (rust-alloc, free with
/// vitro_free_string). Returns {"delta":..,"cursor":..,"total":..,"stream":"stdout"}.
VITRO_API char* vitro_get_program_output_delta(VitroSession* s, int cursor);

/// Incremental display-view output since a byte cursor, as a JSON string.
/// [rust-alloc] Same cursor protocol as vitro_get_program_output_delta.
VITRO_API char* vitro_get_output_delta(VitroSession* s, int cursor);

#ifdef __cplusplus
}
#endif
