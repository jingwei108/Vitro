#![allow(clippy::unwrap_used, clippy::expect_used)]

use vitro_native::compiler::ast::{TemplateArg, Type};

fn ptr(ty: Type) -> Type {
    Type::Pointer {
        pointee: Box::new(ty),
        is_const: false,
    }
}

fn array(ty: Type, dims: Vec<i32>) -> Type {
    let array_size = dims.iter().product();
    Type::Array {
        element: Box::new(ty),
        array_size,
        dims,
        is_const: false,
        is_vla: false,
        vla_dims: Vec::new(),
    }
}

fn func(ret: Type, params: Vec<Type>) -> Type {
    Type::Function {
        return_type: Box::new(ret),
        param_types: params,
        is_const: false,
        is_variadic: false,
    }
}

#[test]
fn test_mangle_name_into_matches_mangle_name() {
    let cases: Vec<Type> = vec![
        Type::void(),
        Type::int(),
        Type::unsigned_int(),
        Type::char(),
        Type::float(),
        Type::double(),
        ptr(Type::int()),
        array(Type::int(), vec![3, 4]),
        func(Type::int(), vec![Type::int(), ptr(Type::char())]),
        Type::struct_type("Node".to_string()),
        Type::Class {
            name: "Vector".to_string(),
            is_const: false,
        },
        Type::Reference {
            base: Box::new(Type::int()),
            is_const: false,
        },
        Type::RValueRef { base: Box::new(Type::int()) },
        Type::TemplateId {
            base: "list".to_string(),
            args: vec![TemplateArg::Type(Type::int()), TemplateArg::Type(ptr(Type::int()))],
            is_const: false,
        },
    ];

    for ty in cases {
        let expected = ty.mangle_name();
        let mut buf = String::new();
        ty.mangle_name_into(&mut buf);
        assert_eq!(buf, expected, "mangle_name_into mismatch for {:?}", ty);
    }
}

#[test]
fn test_mangle_name_into_appends_to_buffer() {
    let mut buf = String::from("prefix_");
    ptr(array(Type::int(), vec![2, 3])).mangle_name_into(&mut buf);
    assert_eq!(buf, "prefix_p_a2_3_int");
}
