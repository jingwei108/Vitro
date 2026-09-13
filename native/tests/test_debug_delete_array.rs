#![allow(clippy::unwrap_used, clippy::expect_used)]

use vitro_native::compiler::codegen::BytecodeGen;
use vitro_native::compiler::lexer::Lexer;
use vitro_native::compiler::parser::Parser;
use vitro_native::compiler::typeck::TypeChecker;

#[test]
fn test_debug_delete_array() {
    let src = r#"
class A {
public:
    int id;
    A() { id = 0; }
    void init(int i) { id = i; }
    ~A() { id = 0; }
};
int main() {
    A* arr = new A[3];
    arr[0].init(1);
    delete[] arr;
    return 0;
}
"#;
    let (tokens, _) = Lexer::with_mode(src, true).tokenize();
    let (program, parse_errors) = Parser::with_mode(tokens, true).parse();
    assert!(parse_errors.is_empty(), "parse errors: {:?}", parse_errors);
    let mut program = program.unwrap();
    let checker = TypeChecker::default();
    let (errors, _warnings, _hints) = checker.check(&mut program);
    println!("Type errors: {:?}", errors);
    for f in &program.funcs {
        println!("FUNC: {}", f.name);
    }
    let gen = BytecodeGen::new();
    let result = gen.generate(&mut program);
    match result {
        Ok(output) => {
            for (i, instr) in output.code.iter().enumerate() {
                println!("{}: {:?} {}", i, instr.op, instr.operand);
            }
        }
        Err(e) => println!("Errors: {:?}", e),
    }
}
