use vitro_ast::Type;
use serde::Deserialize;
use std::collections::HashMap;
use std::sync::LazyLock;

// =============================================================================
// JSON schema (must match scripts/extract_cpp_builtin_layout.py output)
// =============================================================================

#[derive(Debug, Clone, Deserialize)]
struct FieldJson {
    name: String,
    #[serde(rename = "type")]
    ty: String,
}

#[derive(Debug, Clone, Deserialize)]
struct MethodSigJson {
    name: String,
    params: Vec<String>,
    ret: String,
    is_virtual: bool,
}

#[derive(Debug, Clone, Deserialize)]
struct ClassLayoutJson {
    cpp_name: String,
    size: i32,
    fields: Vec<FieldJson>,
    methods: Vec<MethodSigJson>,
}

#[derive(Debug, Clone, Deserialize)]
struct LayoutData {
    classes: HashMap<String, ClassLayoutJson>,
    #[serde(rename = "method_map")]
    method_map: HashMap<String, HashMap<String, String>>,
}

// =============================================================================
// Public API (kept backward-compatible)
// =============================================================================

#[derive(Debug, Clone)]
pub struct ClassLayout {
    pub size: i32,
    pub fields: Vec<(String, Type)>,
    pub methods: Vec<MethodSig>,
}

#[derive(Debug, Clone)]
pub struct MethodSig {
    pub name: String,
    pub params: Vec<Type>,
    pub ret: Type,
    pub is_virtual: bool,
}

/// Parse a simple type string into Vitro's `Type` enum.
/// Supports: void, int, float, char, double, and pointer variants (e.g. "int*").
fn parse_type_str(s: &str) -> Type {
    let s = s.trim();
    if let Some(inner) = s.strip_suffix('*') {
        return Type::pointer_to(parse_type_str(inner.trim()));
    }
    match s {
        "void" => Type::void(),
        "int" => Type::int(),
        "float" => Type::float(),
        "char" => Type::char(),
        "double" => Type::double(),
        _ => {
            // Fallback for unknown types; in practice all builtin types are covered above.
            Type::int()
        }
    }
}

fn convert_layout(json: &ClassLayoutJson) -> ClassLayout {
    ClassLayout {
        size: json.size,
        fields: json.fields.iter().map(|f| (f.name.clone(), parse_type_str(&f.ty))).collect(),
        methods: json
            .methods
            .iter()
            .map(|m| MethodSig {
                name: m.name.clone(),
                params: m.params.iter().map(|p| parse_type_str(p)).collect(),
                ret: parse_type_str(&m.ret),
                is_virtual: m.is_virtual,
            })
            .collect(),
    }
}

static LAYOUT_DATA: LazyLock<LayoutData> = LazyLock::new(|| {
    let json = include_str!("builtin_layout_data.json");
    match serde_json::from_str(json) {
        Ok(data) => data,
        Err(e) => panic!("builtin_layout_data.json is invalid: {}", e),
    }
});

static BUILTIN_LAYOUTS: LazyLock<HashMap<String, ClassLayout>> = LazyLock::new(|| {
    LAYOUT_DATA
        .classes
        .iter()
        .map(|(k, v)| (k.clone(), convert_layout(v)))
        .collect()
});

/// Look up the layout of a builtin container by its Vitro internal name.
pub fn builtin_class_layout(name: &str) -> Option<ClassLayout> {
    BUILTIN_LAYOUTS.get(name).cloned()
}

/// Look up a specific method signature on a builtin container.
pub fn builtin_method_sig(class_name: &str, method_name: &str) -> Option<MethodSig> {
    BUILTIN_LAYOUTS
        .get(class_name)?
        .methods
        .iter()
        .find(|m| m.name == method_name)
        .cloned()
}

static CPP_TO_VITRO: LazyLock<HashMap<&'static str, &'static str>> = LazyLock::new(|| {
    LAYOUT_DATA
        .classes
        .iter()
        .map(|(vitro_name, cls)| {
            let cpp: &'static str = Box::leak(cls.cpp_name.clone().into_boxed_str());
            let vitro: &'static str = Box::leak(vitro_name.clone().into_boxed_str());
            (cpp, vitro)
        })
        .collect()
});

static METHOD_MAP: LazyLock<HashMap<&'static str, HashMap<&'static str, &'static str>>> = LazyLock::new(|| {
    LAYOUT_DATA
        .method_map
        .iter()
        .map(|(class_name, methods)| {
            let class_key: &'static str = Box::leak(class_name.clone().into_boxed_str());
            let method_entries: HashMap<&'static str, &'static str> = methods
                .iter()
                .map(|(method_name, func_name)| {
                    let method_key: &'static str = Box::leak(method_name.clone().into_boxed_str());
                    let func_value: &'static str = Box::leak(func_name.clone().into_boxed_str());
                    (method_key, func_value)
                })
                .collect();
            (class_key, method_entries)
        })
        .collect()
});

/// Return all (cpp_name, vitro_name) pairs known to the builtin layout system.
/// Useful for codegen / type-checker loops that pre-register container metadata.
pub fn builtin_class_mappings() -> Vec<(&'static str, &'static str)> {
    CPP_TO_VITRO.iter().map(|(&k, &v)| (k, v)).collect()
}

/// Map a C++ standard container type name to its Vitro internal type name.
/// e.g. `vector<int>` → `vitro_vec_int`.
pub fn cpp_type_to_vitro(name: &str) -> Option<&'static str> {
    CPP_TO_VITRO.get(name).copied()
}

/// Check whether the given type name is a builtin container type.
pub fn is_builtin_container(name: &str) -> bool {
    cpp_type_to_vitro(name).is_some()
}

/// Map a builtin container method call to the underlying Vitro container library
/// function name.  The mapping is read from builtin_layout_data.json.
pub fn map_container_method(class_name: &str, method: &str) -> Option<&'static str> {
    METHOD_MAP.get(class_name)?.get(method).copied()
}
