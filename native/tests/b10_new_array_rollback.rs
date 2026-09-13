#![allow(clippy::unwrap_used, clippy::expect_used)]

mod test_utils;

use vitro_native::session::Session;
use vitro_native::vm::core::VitroVM;

#[test]
fn test_new_array_rollback_on_ctor_trap() {
    let src = r#"
#include <stdio.h>
class Foo {
public:
    Foo() {
        static int count = 0;
        printf("ctor %d\n", count);
        if (count == 2) {
            int *p = 0;
            *p = 1;
        }
        count++;
    }
};
int main() {
    Foo *arr = new Foo[3];
    return 0;
}
"#;
    let output = test_utils::compile_cpp_bytecode(src).expect("compile should succeed");

    let mut vm = VitroVM::new();
    vm.load_program(output.code.clone());
    for (name, meta) in &output.func_table {
        if let Some(&idx) = output.func_index.get(name) {
            vm.register_function(
                idx as u32,
                vitro_native::vm::core::FuncMeta {
                    ip: meta.ip,
                    arg_count: meta.arg_count,
                    param_count: meta.param_count,
                    local_count: meta.local_count,
                    param_sizes: meta.param_sizes.clone(),
                    return_type: meta.return_type.clone(),
                    is_variadic: meta.is_variadic,
                    local_buffers: Vec::new(),
                },
            );
            vm.register_function_name(idx as u32, name.clone());
        }
    }
    vm.set_globals_32(&output.globals_init_32);
    vm.set_globals_64(&output.globals_init_64);
    vm.set_symbols(
        output
            .symbols
            .iter()
            .map(|s| vitro_native::vm::core::VMSymbol {
                name: s.name.clone(),
                addr: s.addr,
                is_local: s.is_local,
                ty: s.ty.clone(),
                scope_depth: s.scope_depth,
                func_name: s.func_name.clone(),
                decl_line: s.decl_line,
            })
            .collect(),
    );

    let mut session = Session::default();
    vm.run(&mut session.as_vm_context());

    assert!(!vm.get_error().is_empty(), "expected trap but got none");
    assert!(
        !vm.get_freed_logs().is_empty(),
        "expected memory to be freed on constructor trap, but freed_logs is empty"
    );
}
