use super::*;

fn loc() -> SourceLoc {
    SourceLoc::default()
}

fn lit(v: i32) -> Expr {
    Expr::Literal {
        value: v,
        loc: loc(),
        ty: Type::int(),
    }
}

fn init(value: Expr) -> InitElement {
    InitElement { designators: vec![], value }
}

#[test]
fn test_flatten_init_list_empty() {
    let mut errors = vec![];
    let result = flatten_init_list(&[], &mut errors);
    assert!(result.is_empty());
    assert!(errors.is_empty());
}

#[test]
fn test_flatten_init_list_simple_ints() {
    let mut errors = vec![];
    let elems = vec![init(lit(1)), init(lit(2)), init(lit(3))];
    let result = flatten_init_list(&elems, &mut errors);
    assert_eq!(result, vec![1, 2, 3]);
    assert!(errors.is_empty());
}

#[test]
fn test_flatten_init_list_nested() {
    let mut errors = vec![];
    let nested = Expr::InitList {
        elements: vec![init(lit(4)), init(lit(5))],
        loc: loc(),
        ty: Type::array_of(Type::int(), vec![2]),
    };
    let elems = vec![init(lit(1)), init(nested), init(lit(6))];
    let result = flatten_init_list(&elems, &mut errors);
    assert_eq!(result, vec![1, 4, 5, 6]);
    assert!(errors.is_empty());
}

#[test]
fn test_flatten_init_list_float_bits() {
    let mut errors = vec![];
    let f = Expr::FloatLiteral {
        value: 1.0,
        loc: loc(),
        ty: Type::float(),
    };
    let result = flatten_init_list(&[init(f)], &mut errors);
    assert_eq!(result, vec![1.0_f32.to_bits() as i32]);
    assert!(errors.is_empty());
}

#[test]
fn test_flatten_init_list_negative_literal() {
    let mut errors = vec![];
    let neg = Expr::Unary {
        op: UnaryOp::Neg,
        operand: Box::new(lit(7)),
        loc: loc(),
        ty: Type::int(),
    };
    let result = flatten_init_list(&[init(neg)], &mut errors);
    assert_eq!(result, vec![-7]);
    assert!(errors.is_empty());
}

#[test]
fn test_flatten_init_list_designator_error() {
    let mut errors = vec![];
    let mut elem = init(lit(1));
    elem.designators.push(Designator::Index(Box::new(lit(0))));
    let result = flatten_init_list(&[elem], &mut errors);
    assert_eq!(result, vec![1]);
    assert_eq!(errors.len(), 1);
    assert!(errors[0].contains("Designated"));
}

#[test]
fn test_stmt_loc_var_decl() {
    let loc = SourceLoc {
        line: 10,
        column: 5,
        file_id: 0,
    };
    let stmt = Stmt::VarDecl {
        var_type: Type::int(),
        name: "x".to_string(),
        init: None,
        extra_vars: vec![],
        is_static: false,
        loc,
    };
    let got = stmt_loc(&stmt);
    assert_eq!(got.line, 10);
    assert_eq!(got.column, 5);
}

#[test]
fn test_stmt_loc_return() {
    let loc = SourceLoc {
        line: 20,
        column: 1,
        file_id: 0,
    };
    let stmt = Stmt::Return { value: None, loc };
    let got = stmt_loc(&stmt);
    assert_eq!(got.line, 20);
    assert_eq!(got.column, 1);
}

#[test]
fn test_compute_stride_1d_array() {
    let arr = Type::array_of(Type::int(), vec![5]);
    assert_eq!(compute_stride(&arr, 4), 4);
}

#[test]
fn test_compute_stride_2d_array() {
    let arr = Type::array_of(Type::int(), vec![3, 4]);
    assert_eq!(compute_stride(&arr, 4), 16);
}

#[test]
fn test_compute_stride_3d_array() {
    let arr = Type::array_of(Type::int(), vec![2, 3, 4]);
    assert_eq!(compute_stride(&arr, 4), 48);
}
