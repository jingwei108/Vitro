#![allow(clippy::unwrap_used, clippy::expect_used)]

use vitro_native::compiler::lexer::{Lexer, TokenType};

fn tokenize(src: &str) -> Vec<(TokenType, String)> {
    let (tokens, errors) = Lexer::new(src).tokenize();
    assert!(errors.is_empty(), "Lexer errors: {:?}", errors);
    tokens.into_iter().map(|t| (t.ty, t.text)).collect()
}

#[test]
fn test_lexer_basic_tokens() {
    let tokens = tokenize("int main() { return 0; }");
    assert_eq!(tokens[0], (TokenType::Int, "int".to_string()));
    assert_eq!(tokens[1], (TokenType::Identifier, "main".to_string()));
    assert_eq!(tokens[2], (TokenType::LParen, "(".to_string()));
    assert_eq!(tokens[3], (TokenType::RParen, ")".to_string()));
    assert_eq!(tokens[4], (TokenType::LBrace, "{".to_string()));
    assert_eq!(tokens[5], (TokenType::Return, "return".to_string()));
    assert_eq!(tokens[6], (TokenType::Number, "0".to_string()));
    assert_eq!(tokens[7], (TokenType::Semicolon, ";".to_string()));
    assert_eq!(tokens[8], (TokenType::RBrace, "}".to_string()));
    assert_eq!(tokens[9], (TokenType::Eof, "".to_string()));
}

#[test]
fn test_lexer_hex_literal() {
    let tokens = tokenize("0xFF 0x80000000 0x0");
    assert_eq!(tokens[0], (TokenType::Number, "255".to_string()));
    assert_eq!(tokens[1], (TokenType::UnsignedLiteral, "2147483648".to_string()));
    assert_eq!(tokens[2], (TokenType::Number, "0".to_string()));
}

#[test]
fn test_lexer_string_literal() {
    let tokens = tokenize("\"hello\" \"world\\n\"");
    assert_eq!(tokens[0], (TokenType::String, "hello".to_string()));
    assert_eq!(tokens[1], (TokenType::String, "world\n".to_string()));
}

#[test]
fn test_lexer_char_literal() {
    let tokens = tokenize("'a' '\\n' '0'");
    assert_eq!(tokens[0], (TokenType::CharLiteral, "97".to_string()));
    assert_eq!(tokens[1], (TokenType::CharLiteral, "10".to_string()));
    assert_eq!(tokens[2], (TokenType::CharLiteral, "48".to_string()));
}

#[test]
fn test_lexer_operators() {
    let tokens = tokenize("+ - * / % == != <= >= && || << >> & | ^ ~");
    let ops = vec![
        "+", "-", "*", "/", "%", "==", "!=", "<=", ">=", "&&", "||", "<<", ">>", "&", "|", "^", "~",
    ];
    for (i, op) in ops.iter().enumerate() {
        assert_eq!(tokens[i].1, op.to_string(), "operator mismatch at index {}", i);
    }
}

#[test]
fn test_lexer_comments() {
    let tokens = tokenize("// line comment\nint x;\n/* block\ncomment */ float y;");
    // comments are skipped, only real tokens remain
    assert_eq!(tokens[0], (TokenType::Int, "int".to_string()));
    assert_eq!(tokens[1], (TokenType::Identifier, "x".to_string()));
    assert_eq!(tokens[2], (TokenType::Semicolon, ";".to_string()));
    assert_eq!(tokens[3], (TokenType::Float, "float".to_string()));
    assert_eq!(tokens[4], (TokenType::Identifier, "y".to_string()));
    assert_eq!(tokens[5], (TokenType::Semicolon, ";".to_string()));
}

#[test]
fn test_lexer_define_macro() {
    let src = "#define MAX 100\nint x = MAX;";
    let tokens = tokenize(src);
    // After macro expansion, MAX should be tokenized as Number 100
    let nums: Vec<_> = tokens.iter().filter(|(t, _)| *t == TokenType::Number).collect();
    assert_eq!(nums.len(), 1);
    assert_eq!(nums[0].1, "100");
}

#[test]
fn test_lexer_keywords() {
    let src = "if else while for do return break continue switch case default struct typedef enum sizeof const void float int char unsigned signed long short";
    let tokens = tokenize(src);
    let expected = vec![
        TokenType::If,
        TokenType::Else,
        TokenType::While,
        TokenType::For,
        TokenType::Do,
        TokenType::Return,
        TokenType::Break,
        TokenType::Continue,
        TokenType::Switch,
        TokenType::Case,
        TokenType::Default,
        TokenType::Struct,
        TokenType::Typedef,
        TokenType::Enum,
        TokenType::Sizeof,
        TokenType::Const,
        TokenType::Void,
        TokenType::Float,
        TokenType::Int,
        TokenType::Char,
        TokenType::Unsigned,
        TokenType::Signed,
        TokenType::Long,
        TokenType::Short,
    ];
    for (i, exp) in expected.iter().enumerate() {
        assert_eq!(&tokens[i].0, exp, "keyword mismatch at index {}", i);
    }
}

#[test]
fn test_lexer_error_unknown_char() {
    let (_, errors) = Lexer::new("int @ x;").tokenize();
    assert!(!errors.is_empty(), "Expected lexer error for unknown char @");
    assert!(
        errors[0].message.contains("无法识别") || errors[0].message.contains("未知"),
        "Expected Chinese error message, got: {}",
        errors[0].message
    );
}

#[test]
fn test_lexer_multiline() {
    let src = "int a = 1;\nint b = 2;";
    let (tokens, _) = Lexer::new(src).tokenize();
    // check line numbers of some tokens
    assert_eq!(tokens[0].line, 1); // int
    assert_eq!(tokens[5].line, 2); // int on second line
}

// ============================================================================
// 条件编译（#ifdef / #ifndef / #else / #endif）单元测试
// ============================================================================

#[test]
fn test_lexer_ifdef_defined() {
    let src = "#define DEBUG 1\n#ifdef DEBUG\nint x = 1;\n#endif";
    let tokens = tokenize(src);
    // x 应该被编译
    let ids: Vec<_> = tokens.iter().filter(|(t, _)| *t == TokenType::Identifier).collect();
    assert!(
        ids.iter().any(|(_, text)| text == "x"),
        "x should be tokenized when DEBUG is defined"
    );
}

#[test]
fn test_lexer_ifdef_undefined() {
    let src = "#ifdef UNDEFINED\nint y = 2;\n#endif\nint z = 3;";
    let tokens = tokenize(src);
    // y 应该被跳过，z 应该被编译
    let ids: Vec<_> = tokens.iter().filter(|(t, _)| *t == TokenType::Identifier).collect();
    assert!(
        !ids.iter().any(|(_, text)| text == "y"),
        "y should be skipped when UNDEFINED is not defined"
    );
    assert!(ids.iter().any(|(_, text)| text == "z"), "z should be tokenized after #endif");
}

#[test]
fn test_lexer_ifndef() {
    let src = "#ifndef FLAG\nint a = 1;\n#endif";
    let tokens = tokenize(src);
    // FLAG 未定义，所以 a 应该被编译
    let ids: Vec<_> = tokens.iter().filter(|(t, _)| *t == TokenType::Identifier).collect();
    assert!(
        ids.iter().any(|(_, text)| text == "a"),
        "a should be tokenized when FLAG is not defined"
    );
}

#[test]
fn test_lexer_ifndef_defined() {
    let src = "#define FLAG 1\n#ifndef FLAG\nint b = 2;\n#endif\nint c = 3;";
    let tokens = tokenize(src);
    // FLAG 已定义，所以 b 应该被跳过，c 应该被编译
    let ids: Vec<_> = tokens.iter().filter(|(t, _)| *t == TokenType::Identifier).collect();
    assert!(
        !ids.iter().any(|(_, text)| text == "b"),
        "b should be skipped when FLAG is defined"
    );
    assert!(ids.iter().any(|(_, text)| text == "c"), "c should be tokenized after #endif");
}

#[test]
fn test_lexer_conditional_else() {
    let src = "#define MODE 1\n#ifdef MODE\nint a = 1;\n#else\nint a = 2;\n#endif";
    let tokens = tokenize(src);
    // MODE 定义了，所以 a=1 应该被编译，a=2 应该被跳过
    let nums: Vec<_> = tokens.iter().filter(|(t, _)| *t == TokenType::Number).collect();
    assert!(nums.iter().any(|(_, text)| text == "1"), "1 should appear when MODE is defined");
    assert!(!nums.iter().any(|(_, text)| text == "2"), "2 should be skipped in #else block");
}

#[test]
fn test_lexer_conditional_else_undefined() {
    let src = "#ifdef MODE\nint a = 1;\n#else\nint a = 2;\n#endif";
    let tokens = tokenize(src);
    // MODE 未定义，所以 a=1 应该被跳过，a=2 应该被编译
    let nums: Vec<_> = tokens.iter().filter(|(t, _)| *t == TokenType::Number).collect();
    assert!(
        !nums.iter().any(|(_, text)| text == "1"),
        "1 should be skipped when MODE is not defined"
    );
    assert!(nums.iter().any(|(_, text)| text == "2"), "2 should appear in #else block");
}

#[test]
fn test_lexer_nested_conditional() {
    let src = r#"
#define OUTER
#ifdef OUTER
  #define INNER
  #ifdef INNER
    int x = 1;
  #else
    int x = 2;
  #endif
#else
  int x = 3;
#endif
"#;
    let tokens = tokenize(src);
    // OUTER 和 INNER 都定义了，所以 x=1 应该被编译，x=2 和 x=3 应该被跳过
    let nums: Vec<_> = tokens.iter().filter(|(t, _)| *t == TokenType::Number).collect();
    assert!(nums.iter().any(|(_, text)| text == "1"), "1 should appear");
    assert!(!nums.iter().any(|(_, text)| text == "2"), "2 should be skipped");
    assert!(!nums.iter().any(|(_, text)| text == "3"), "3 should be skipped");
}

#[test]
fn test_lexer_nested_skip_inner() {
    let src = r#"
#ifdef OUTER
  #ifdef INNER
    int x = 1;
  #endif
#endif
int y = 2;
"#;
    let tokens = tokenize(src);
    // OUTER 未定义，所以所有内部代码都被跳过，只有 y=2 被编译
    let ids: Vec<_> = tokens.iter().filter(|(t, _)| *t == TokenType::Identifier).collect();
    assert!(!ids.iter().any(|(_, text)| text == "x"), "x should be skipped");
    assert!(ids.iter().any(|(_, text)| text == "y"), "y should be tokenized");
}

#[test]
fn test_lexer_header_guard_pattern() {
    let src = r#"
#ifndef MYHEADER_H
#define MYHEADER_H
int global = 42;
#endif
"#;
    let tokens = tokenize(src);
    // 第一次遇到 MYHEADER_H 未定义，所以内容被编译；然后 MYHEADER_H 被定义
    let ids: Vec<_> = tokens.iter().filter(|(t, _)| *t == TokenType::Identifier).collect();
    assert!(ids.iter().any(|(_, text)| text == "global"), "global should be tokenized");
}

#[test]
fn test_lexer_conditional_with_comments() {
    let src = r#"
#ifdef FLAG
/* block comment */
int a = 1;
#else
// line comment
int a = 2;
#endif
"#;
    let tokens = tokenize(src);
    // FLAG 未定义，所以 #else 块生效，a=2 被编译
    let nums: Vec<_> = tokens.iter().filter(|(t, _)| *t == TokenType::Number).collect();
    assert!(!nums.iter().any(|(_, text)| text == "1"), "1 should be skipped");
    assert!(nums.iter().any(|(_, text)| text == "2"), "2 should appear");
}

#[test]
fn test_lexer_conditional_error_unclosed() {
    let src = "#ifdef FLAG\nint x = 1;";
    let (_, errors) = Lexer::new(src).tokenize();
    assert!(!errors.is_empty(), "Expected error for unclosed #ifdef");
    assert!(errors.iter().any(|e| e.code == 1013), "Expected E1013_UnclosedConditional");
}

#[test]
fn test_lexer_conditional_error_unmatched_endif() {
    let src = "int x = 1;\n#endif";
    let (_, errors) = Lexer::new(src).tokenize();
    assert!(!errors.is_empty(), "Expected error for unmatched #endif");
    assert!(errors.iter().any(|e| e.code == 1011), "Expected E1011_UnmatchedConditional");
}

#[test]
fn test_lexer_conditional_error_duplicate_else() {
    let src = "#ifdef FLAG\nint a = 1;\n#else\nint a = 2;\n#else\nint a = 3;\n#endif";
    let (_, errors) = Lexer::new(src).tokenize();
    assert!(!errors.is_empty(), "Expected error for duplicate #else");
    assert!(errors.iter().any(|e| e.code == 1012), "Expected E1012_DuplicateElse");
}

#[test]
fn test_lexer_define_inside_skipped_block() {
    let src = r#"
#ifdef UNDEFINED
#define SECRET 999
#endif
int x = SECRET;
"#;
    let (_, errors) = Lexer::new(src).tokenize();
    // SECRET 在跳过块内定义，不应生效；之后使用 SECRET 应该报错（未定义标识符）
    // 注意：lexer 层只会保留 SECRET 作为 Identifier，不会展开宏
    // 这里主要验证 SECRET 没有被注册为宏
    // 由于 SECRET 未定义，编译时会在 parser/typeck 报错
    // 本测试只验证 lexer 没有产生错误（条件编译本身正确处理）
    assert!(
        errors.is_empty() || !errors.iter().any(|e| e.code == 1011 || e.code == 1012 || e.code == 1013),
        "Should not produce conditional compilation errors"
    );
}

#[test]
fn test_lexer_cpp_keywords() {
    let src = "class public private protected this using namespace virtual override friend template typename static_cast const_cast reinterpret_cast new delete nullptr";
    let (tokens, errors) = Lexer::with_mode(src, true).tokenize();
    assert!(errors.is_empty(), "Lexer errors: {:?}", errors);
    let tokens: Vec<_> = tokens.into_iter().map(|t| (t.ty, t.text)).collect();
    let expected = vec![
        TokenType::Class,
        TokenType::Public,
        TokenType::Private,
        TokenType::Protected,
        TokenType::This,
        TokenType::Using,
        TokenType::Namespace,
        TokenType::Virtual,
        TokenType::Override,
        TokenType::Friend,
        TokenType::Template,
        TokenType::Typename,
        TokenType::StaticCast,
        TokenType::ConstCast,
        TokenType::ReinterpretCast,
        TokenType::New,
        TokenType::Delete,
        TokenType::Null,
    ];
    for (i, exp) in expected.iter().enumerate() {
        assert_eq!(&tokens[i].0, exp, "C++ keyword mismatch at index {}: got {:?}", i, tokens[i].1);
    }
}

#[test]
fn test_lexer_cpp_operators() {
    let tokens = tokenize(":: ->* .*");
    assert_eq!(tokens[0], (TokenType::ColonColon, "::".to_string()));
    assert_eq!(tokens[1], (TokenType::ArrowStar, "->*".to_string()));
    assert_eq!(tokens[2], (TokenType::DotStar, ".*".to_string()));
}

#[test]
fn test_lexer_cpp_line_comment() {
    // C++ // comment should be skipped like C-style block comments
    let tokens = tokenize("// C++ line comment\nint x;");
    assert_eq!(tokens[0], (TokenType::Int, "int".to_string()));
    assert_eq!(tokens[1], (TokenType::Identifier, "x".to_string()));
    assert_eq!(tokens[2], (TokenType::Semicolon, ";".to_string()));
}

// ── E1（C23/C89）：数字字面量与 u8 字符串 ──

fn lex_numbers(src: &str) -> Vec<(String, String)> {
    let (tokens, errors) = Lexer::new(src).tokenize();
    assert!(errors.is_empty(), "lexer errors: {:?}", errors);
    tokens
        .into_iter()
        .filter(|t| matches!(t.ty, TokenType::Number | TokenType::LongLiteral | TokenType::UnsignedLiteral | TokenType::FloatLiteral))
        .map(|t| (format!("{:?}", t.ty), t.text))
        .collect()
}

#[test]
fn test_lexer_c23_binary_literal() {
    let toks = lex_numbers("int a = 0b1010; int b = 0B11111111;");
    assert!(toks.contains(&("Number".to_string(), "10".to_string())));
    assert!(toks.contains(&("Number".to_string(), "255".to_string())));
}

#[test]
fn test_lexer_c23_digit_separators() {
    let toks = lex_numbers("int m = 1'000'000; int h = 0x1'0000; int b = 0b1010'0000;");
    assert!(toks.contains(&("Number".to_string(), "1000000".to_string())));
    assert!(toks.contains(&("Number".to_string(), "65536".to_string())));
    assert!(toks.contains(&("Number".to_string(), "160".to_string())));
}

#[test]
fn test_lexer_c89_scientific_notation() {
    let toks = lex_numbers("double a = 2.2e-16; double b = 1e5; double c = 1.5E+3; double d = 3.f;");
    assert!(toks.iter().any(|(_, t)| t == "2.2e-16"), "got {:?}", toks);
    assert!(toks.iter().any(|(_, t)| t == "1e5"), "got {:?}", toks);
    assert!(toks.iter().any(|(_, t)| t == "1.5E+3"), "got {:?}", toks);
}

#[test]
fn test_lexer_u8_string_prefix() {
    let src = "u8".to_string() + "\"" + "hi" + "\"";
    let (tokens, errors) = Lexer::new(&src).tokenize();
    assert!(errors.is_empty(), "errors: {:?}", errors);
    let strings: Vec<_> = tokens.into_iter().filter(|t| t.ty == TokenType::String).collect();
    assert_eq!(strings.len(), 1);
    assert_eq!(strings[0].text, "hi");
}

// ── E2：模块化预处理器 ──

fn preprocess(src: &str) -> (Vec<vitro_lexer::Token>, Vec<vitro_lexer::LexerError>, Vec<vitro_lexer::LexerWarning>, Vec<String>) {
    let mut lexer = vitro_lexer::Lexer::new(src);
    let (tokens, errors) = lexer.tokenize();
    let warnings = lexer.into_warnings();
    let trace = lexer.into_expansion_trace();
    (tokens, errors, warnings, trace)
}

fn token_texts(tokens: &[vitro_lexer::Token]) -> Vec<String> {
    tokens.iter().map(|t| t.text.clone()).collect()
}

#[test]
fn test_preprocessor_if_expr_arithmetic() {
    let (_, errs, _, _) = preprocess("#if 1 + 2 * 3 == 7\nint alive;\n#else\nint dead;\n#endif\n");
    assert!(errs.is_empty(), "{:?}", errs);
    let (_, errs2, _, _) = preprocess("#if 0\nint dead;\n#endif\n");
    assert!(errs2.is_empty());
}

#[test]
fn test_preprocessor_if_skips_inactive_branch() {
    let (tokens, errs, _, _) = preprocess("#define M 2\n#if M == 1\nint dead_var;\n#elif M == 2\nint alive_var;\n#else\nint else_var;\n#endif\n");
    assert!(errs.is_empty(), "{:?}", errs);
    let texts = token_texts(&tokens);
    assert!(texts.contains(&"alive_var".to_string()), "{:?}", texts);
    assert!(!texts.contains(&"dead_var".to_string()));
    assert!(!texts.contains(&"else_var".to_string()));
}

#[test]
fn test_preprocessor_shortcircuit_avoids_div_zero() {
    // C 短路语义：左假时右不求值（除零不触发）
    let (_, errs, _, _) = preprocess("#define A 0\n#if A != 0 && 10 / A > 1\nint dead;\n#endif\nint ok_v;\n");
    assert!(errs.is_empty(), "{:?}", errs);
}

#[test]
fn test_preprocessor_stringize_keeps_raw_spelling() {
    // # 操作数不展开；间接一层才展开（C99 6.10.3.1 特例）
    let (tokens, errs, _, _) = preprocess("#define STR(x) #x\n#define XSTR(x) STR(x)\n#define V 9\nSTR(V) XSTR(V)\n");
    assert!(errs.is_empty(), "{:?}", errs);
    let strings: Vec<_> = tokens.iter().filter(|t| t.ty == vitro_lexer::TokenType::String).map(|t| t.text.clone()).collect();
    assert_eq!(strings, vec!["V".to_string(), "9".to_string()], "{:?}", strings);
}

#[test]
fn test_preprocessor_paste_single_token() {
    let (tokens, errs, _, _) = preprocess("#define GLUE(a, b) a##b\nGLUE(my, var)\n");
    assert!(errs.is_empty(), "{:?}", errs);
    assert!(token_texts(&tokens).contains(&"myvar".to_string()));
}

#[test]
fn test_preprocessor_paste_invalid_result_errors() {
    let (_, errs, _, _) = preprocess("#define BAD(a, b) a##b\nBAD(1, +)\n");
    assert!(
        errs.iter().any(|e| e.code == vitro_shared::ErrorCode::E1016_TokenPasteInvalid as i32),
        "拼接出非法结果应报 E1016，实际 {:?}",
        errs
    );
}

#[test]
fn test_preprocessor_depth_fuse() {
    // 实参驱动的指数展开：嵌套 80 层 REP(x) x x → 保险丝在 64 层触发
    let depth = 80;
    let mut src = String::from("#define REP(x) x x\nint v = ");
    for _ in 0..depth {
        src.push_str("REP(");
    }
    src.push('1');
    for _ in 0..depth {
        src.push(')');
    }
    src.push_str(";\n");
    let (_, errs, _, _) = preprocess(&src);
    assert!(
        errs.iter().any(|e| e.code == vitro_shared::ErrorCode::E1017_ExpandDepthExceeded as i32),
        "深嵌套应触发展开保险丝 E1017，实际 {:?}",
        errs
    );
}

/// U1#6 红锚①：不同名对象宏链不触发自引用查重（每个名字只出现一次），
/// token 数（5000）与字节数都远低于规模预算——**只有深度保险丝能拦**。
/// 修复前深度检查只在 depth=0 入口（死代码），实测该形状 5000 层栈溢出
/// 崩溃；既有 test_preprocessor_depth_fuse 旧代码下被 token 预算的同码
/// E1017 掩盖（先撞 26 万 token），并未真正覆盖深度路径。
#[test]
fn test_preprocessor_depth_fuse_object_macro_chain() {
    let n = 5000;
    let mut src = String::new();
    for i in 0..n {
        src.push_str(&format!("#define M{} M{}\n", i, i + 1));
    }
    src.push_str("#define M5 123\nint v = M0;\n");
    let (_, errs, _, _) = preprocess(&src);
    assert!(
        errs.iter().any(|e| e.code == vitro_shared::ErrorCode::E1017_ExpandDepthExceeded as i32),
        "不同名对象宏链 5000 层应触发深度保险丝 E1017（而非栈溢出崩溃），实际 {:?}",
        errs
    );
}

/// U1#6 红锚②：4KB 字面量 × S(x) x x 嵌套 14 层 = 16384 个 token（远低于
/// 旧 token 预算 26 万）但实际产出 67MB——**只有字节口径预算能拦**。
/// 修复前该形状零诊断，67MB token 流全程进入 parser/typeck（实测诊断里
/// 暴露 char[67108865]）。
#[test]
fn test_preprocessor_byte_budget_large_literal_amplification() {
    let lit = "A".repeat(4096);
    let mut body = format!("\"{}\"", lit);
    for _ in 0..14 {
        body = format!("S({})", body);
    }
    let src = format!("#define S(x) x x\nconst char* big = {};\n", body);
    let (_, errs, _, _) = preprocess(&src);
    assert!(
        errs.iter().any(|e| e.code == vitro_shared::ErrorCode::E1017_ExpandDepthExceeded as i32),
        "4KB 字面量 × 2^14 放大（67MB）应触发字节预算熔断 E1017（而非零诊断），实际 {:?}",
        errs
    );
}

/// U1#6 反向锚：正常嵌套宏（4 层、产出 16 个 token）不受双保险丝误伤。
#[test]
fn test_preprocessor_normal_nesting_not_affected_by_fuses() {
    let (toks, errs, _, _) = preprocess(
        "#define MAX(a, b) ((a) > (b) ? (a) : (b))\n\
         #define SQ(x) ((x) * (x))\n\
         #define FOUR(x) ((x) + (x) + (x) + (x))\n\
         #define WRAP(x) FOUR(FOUR(x))\n\
         int v = MAX(MAX(1, 5), 3) + SQ(MAX(2, 3)) + WRAP(1);\n",
    );
    assert!(errs.is_empty(), "正常嵌套宏不应报错，实际 {:?}", errs);
    assert!(
        !token_texts(&toks).contains(&"WRAP".to_string()),
        "WRAP 应被展开"
    );
}

#[test]
fn test_preprocessor_shadowing_warning() {
    let (_, errs, warnings, _) = preprocess("#define W 1\n#define W 2\nint x = W;\n");
    assert!(errs.is_empty());
    assert!(
        warnings.iter().any(|w| w.code == vitro_shared::ErrorCode::W1018_MacroShadowing as i32),
        "不同体重定义应报 W1018，实际 {:?}",
        warnings
    );
    // 相同体重定义静默（C 标准允许）
    let (_, errs2, warnings2, _) = preprocess("#define U 1\n#define U 1\nint y = U;\n");
    assert!(errs2.is_empty() && warnings2.is_empty());
}

#[test]
fn test_preprocessor_side_effect_warning() {
    let (_, errs, warnings, _) = preprocess("#define SQ(x) ((x) * (x))\nint i = 3;\nint z = SQ(i++);\n");
    assert!(errs.is_empty());
    assert!(
        warnings.iter().any(|w| w.code == vitro_shared::ErrorCode::W1019_MacroArgSideEffect as i32),
        "SQ(i++) 应报 W1019，实际 {:?}",
        warnings
    );
}

#[test]
fn test_preprocessor_expansion_trace() {
    let (_, errs, _, trace) = preprocess("#define DOUBLE(x) ((x) * 2)\nint v = DOUBLE(5);\n");
    assert!(errs.is_empty());
    assert!(trace.iter().any(|t| t.contains("DOUBLE(5)")), "展开链应被记录，实际 {:?}", trace);
}

#[test]
fn test_preprocessor_branch_reason() {
    let (_, errs, _, trace) = preprocess("#define A 5\n#if A > 3\nint ok_v;\n#endif\n");
    assert!(errs.is_empty());
    assert!(trace.iter().any(|t| t.starts_with("#if") && t.contains("真")), "{:?}", trace);
}

#[test]
fn test_preprocessor_has_include() {
    let (tokens, errs, _, _) = preprocess("#if __has_include(<stdio.h>)\nint has_v;\n#else\nint no_v;\n#endif\n");
    assert!(errs.is_empty(), "{:?}", errs);
    let texts = token_texts(&tokens);
    assert!(texts.contains(&"has_v".to_string()));
    assert!(!texts.contains(&"no_v".to_string()));
}

#[test]
fn test_preprocessor_include_once() {
    // 无守卫双 include：Vitro include-once 静默跳过（与 Clang 的差异已入 spec）。
    // U1#11 改写：旧版用不存在的头文件断言 errs.is_empty()——那正是 H-1 缺陷
    // （include 找不到静默跳过）被固化成了断言；include-once 语义改用真实头
    // 验证，"找不到头文件"的语义锚移至 test_u11_include_quote_not_found。
    let dir = TempIncludeDir::new("once", &[("once.h", "int once_var;\n")]);
    let (tokens, errs) = dir.lex("#include \"once.h\"\n#include \"once.h\"\nint z;\n");
    assert!(errs.is_empty(), "{:?}", errs);
    let texts = token_texts(&tokens);
    assert_eq!(
        texts.iter().filter(|t| *t == "once_var").count(),
        1,
        "include-once：同头二次 include 只拼接一次"
    );
}

#[test]
fn test_preprocessor_undef() {
    let (tokens, errs, _, _) = preprocess("#define T 1\n#undef T\n#define T 7\nint q = T;\n");
    assert!(errs.is_empty());
    assert!(token_texts(&tokens).contains(&"7".to_string()));
}

// ── U1#11 预处理器 P0 批（H-1/H-2/H-3 + 条件栈污染 + 环检测）──
//
// 红→绿锚（2026-09-14）：以下用例先于修复落库，修复前全部 FAIL 留痕。

/// 在系统临时目录建唯一子目录并写入一组文件，供 `#include` 的文件系统
/// 候选链测试使用；drop 时 best-effort 清理。
struct TempIncludeDir {
    path: std::path::PathBuf,
}

impl TempIncludeDir {
    fn new(tag: &str, files: &[(&str, &str)]) -> Self {
        let dir = std::env::temp_dir()
            .join(format!("vitro_u11_{}_{}", tag, std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        for (name, content) in files {
            let p = dir.join(name);
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            std::fs::write(p, content).unwrap();
        }
        Self { path: dir }
    }

    fn lex(&self, src: &str) -> (Vec<vitro_lexer::Token>, Vec<vitro_lexer::LexerError>) {
        let mut lexer = vitro_lexer::Lexer::with_base_path(src, Some(self.path.clone()));
        lexer.tokenize()
    }
}

impl Drop for TempIncludeDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

/// H-1 / T5：quote include 找不到头文件必须报 E1021（行号 = include 行），
/// 不得静默跳过后把错误错位到使用点。
#[test]
fn test_u11_include_quote_not_found() {
    let dir = TempIncludeDir::new("q404", &[]);
    let (tokens, errs) = dir.lex("#include \"no_such_header_u11.h\"\nint after_include;\n");
    assert!(
        errs.iter().any(|e| e.code == 1021),
        "quote include 找不到应报 E1021_IncludeNotFound，实际 {:?}",
        errs
    );
    assert!(
        errs.iter().any(|e| e.code == 1021 && e.line == 1),
        "E1021 应定位在 include 行（line=1），实际 {:?}",
        errs
    );
    // 报错后扫描继续：include 之后的代码不被吞
    assert!(token_texts(&tokens).contains(&"after_include".to_string()));
}

/// T11：`<>` 引不存在的非标准库头必须报 E1021（修复前静默通过、程序照跑）。
#[test]
fn test_u11_include_angle_not_found() {
    let dir = TempIncludeDir::new("a404", &[]);
    let (_, errs) = dir.lex("#include <no_such_stdlib_u11.h>\nint after_include;\n");
    assert!(
        errs.iter().any(|e| e.code == 1021),
        "angle include 找不到应报 E1021_IncludeNotFound，实际 {:?}",
        errs
    );
}

/// H-3 / T4：`<>` 只查标准库存根，不再搜索文件系统目录——同目录存在同名
/// 自定义头时 `<local.h>` 应报 E1021 且不拼接（与 Clang 一致）。存量语料
/// 扫描（2026-09-14）：655 处 `<>` include 全部为标准头名，零存量依赖。
#[test]
fn test_u11_include_angle_custom_rejected() {
    let dir = TempIncludeDir::new("a_local", &[("local_u11.h", "int from_angle;\n")]);
    let (tokens, errs) = dir.lex("#include <local_u11.h>\nint after_include;\n");
    assert!(
        errs.iter().any(|e| e.code == 1021),
        "<> 形式不应加载文件系统中的自定义头（只查标准库存根），实际 {:?}",
        errs
    );
    assert!(!token_texts(&tokens).contains(&"from_angle".to_string()));
}

/// H-2 / T14：头文件内 `__has_include("...")` 与 `#include "..."` 候选链
/// 统一（包含者目录优先）——nest/outer.h 内探测同目录 inner.h 必须为真。
#[test]
fn test_u11_has_include_uses_includer_dir() {
    let dir = TempIncludeDir::new(
        "has_inc",
        &[
            ("nest/outer.h", "#if __has_include(\"inner.h\")\nint inner_found;\n#else\nint inner_missing;\n#endif\n"),
            ("nest/inner.h", "int inner_val;\n"),
        ],
    );
    let (tokens, errs) = dir.lex("#include \"nest/outer.h\"\n");
    assert!(errs.is_empty(), "{:?}", errs);
    let texts = token_texts(&tokens);
    assert!(texts.contains(&"inner_found".to_string()), "包含者目录命中：{:?}", texts);
    assert!(!texts.contains(&"inner_missing".to_string()), "{:?}", texts);
}

/// H-3 一致性：`__has_include(<...>)` 同样只查标准库存根，与 `<>` include
/// 行为对齐（修复前 `<local.h>` 会命中文件系统，与 include 收紧后自相矛盾）。
#[test]
fn test_u11_has_include_angle_stub_only() {
    let dir = TempIncludeDir::new("has_ang", &[("local_u11.h", "int from_angle;\n")]);
    let (tokens, errs) =
        dir.lex("#if __has_include(<local_u11.h>)\nint angle_hit;\n#else\nint angle_miss;\n#endif\n");
    assert!(errs.is_empty(), "{:?}", errs);
    let texts = token_texts(&tokens);
    assert!(!texts.contains(&"angle_hit".to_string()), "<> 探测不得命中文件系统自定义头：{:?}", texts);
    assert!(texts.contains(&"angle_miss".to_string()));
}

/// 条件栈污染（多余 #endif）：头文件内多余的 `#endif` 会弹掉包含者的条件组，
/// 报错必须定位在头文件内（而非主文件自己的 #endif 行——错位诊断正是缺陷）。
/// 主文件形状：L1 `#if 1` / L2 include / L3 `#endif` / L4 代码。
#[test]
fn test_u11_header_extra_endif_localized() {
    let dir = TempIncludeDir::new("extra_end", &[("bad_extra.h", "int helper_decl;\n#endif\n")]);
    let (tokens, errs) =
        dir.lex("#if 1\n#include \"bad_extra.h\"\n#endif\nint alive_after;\n");
    let e1011: Vec<_> = errs.iter().filter(|e| e.code == 1011).collect();
    assert_eq!(e1011.len(), 1, "恰好一条 E1011，实际 {:?}", errs);
    assert!(
        e1011[0].line < 3,
        "E1011 应定位在头文件内（行号 < 主文件 #endif 所在的 3 行），实际 line={}",
        e1011[0].line
    );
    // 主文件结构不被破坏：helper（头内）与 #if 组外的代码都保留
    let texts = token_texts(&tokens);
    assert!(texts.contains(&"helper_decl".to_string()));
    assert!(texts.contains(&"alive_after".to_string()));
}

/// 条件栈污染（#if 未闭合）：头文件内未闭合的 `#if/#ifndef` 不得把包含者的
/// 后续代码全部吞掉——include 边界哨兵处自动闭合并报 E1013。
#[test]
fn test_u11_header_unclosed_if_autoclosed() {
    let dir = TempIncludeDir::new(
        "leak_if",
        &[("leak.h", "#ifndef LEAK_GUARD\n#define LEAK_GUARD\nint guard_body;\n")],
    );
    let (tokens, errs) = dir.lex("#include \"leak.h\"\nint after_header;\n");
    let e1013: Vec<_> = errs.iter().filter(|e| e.code == 1013).collect();
    assert_eq!(e1013.len(), 1, "头文件未闭合 #if 应报 E1013，实际 {:?}", errs);
    assert!(
        token_texts(&tokens).contains(&"after_header".to_string()),
        "头文件泄漏的条件组不得吞掉包含者的后续代码"
    );
}

/// 环检测长链漏报：20 个文件组成的 include 环必须报 E1015（修复前静态 DFS
/// 深度封顶 16，环静默漏报、零诊断）。
#[test]
fn test_u11_include_cycle_20_files() {
    let mut files: Vec<(String, String)> = Vec::new();
    for i in 0..20 {
        let next = format!("h{:02}.h", (i + 1) % 20);
        files.push((format!("h{:02}.h", i), format!("#include \"{}\"\n", next)));
    }
    let refs: Vec<(&str, &str)> = files.iter().map(|(n, c)| (n.as_str(), c.as_str())).collect();
    let dir = TempIncludeDir::new("ring20", &refs);
    let (tokens, errs) = dir.lex("#include \"h00.h\"\nint after_ring;\n");
    assert!(
        errs.iter().any(|e| e.code == 1015),
        "20 文件 include 环应报 E1015_IncludeCycle，实际 {:?}",
        errs
    );
    assert!(token_texts(&tokens).contains(&"after_ring".to_string()));
}

/// include 嵌套深度保险丝（保险丝可触发性义务）：70 层无环深链在嵌套深度
/// 上限处必须报 E1015（修复前任意深度静默拼接）。
#[test]
fn test_u11_include_depth_fuse() {
    let mut files: Vec<(String, String)> = Vec::new();
    for i in 0..70 {
        let content = if i == 69 {
            "int deepest;\n".to_string()
        } else {
            format!("#include \"d{:02}.h\"\n", i + 1)
        };
        files.push((format!("d{:02}.h", i), content));
    }
    let refs: Vec<(&str, &str)> = files.iter().map(|(n, c)| (n.as_str(), c.as_str())).collect();
    let dir = TempIncludeDir::new("chain70", &refs);
    let (tokens, errs) = dir.lex("#include \"d00.h\"\nint after_chain;\n");
    assert!(
        errs.iter().any(|e| e.code == 1015),
        "超深 include 链应触发嵌套深度保险丝 E1015，实际 {:?}",
        errs
    );
    assert!(
        !token_texts(&tokens).contains(&"deepest".to_string()),
        "超过嵌套上限的头不得继续拼接"
    );
    assert!(token_texts(&tokens).contains(&"after_chain".to_string()));
}
