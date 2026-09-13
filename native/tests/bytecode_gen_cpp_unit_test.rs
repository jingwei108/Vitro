#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::ffi::{c_char, CString};

fn compile_and_run_cpp(source: &str) -> Result<(i32, Vec<String>), String> {
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

#[test]
fn test_cpp_class_field_and_method() {
    let src = r#"
#include <stdio.h>
class Point {
public:
    int x;
    int getX() { return this->x; }
};
int main() {
    Point p;
    p.x = 42;
    printf("%d\n", p.getX());
    return 0;
}
"#;
    let (ret, outputs) = compile_and_run_cpp(src).expect("Compile/run failed");
    assert_eq!(ret, 0);
    assert_eq!(outputs, vec!["42"]);
}

#[test]
fn test_cpp_new_delete_with_ctor() {
    let src = r#"
#include <stdio.h>
class Point {
public:
    int x;
    Point(int v) { this->x = v; }
};
int main() {
    Point* p = new Point(10);
    printf("%d\n", p->x);
    delete p;
    return 0;
}
"#;
    let (ret, outputs) = compile_and_run_cpp(src).expect("Compile/run failed");
    assert_eq!(ret, 0);
    assert_eq!(outputs, vec!["10"]);
}

#[test]
fn test_cpp_range_for_array() {
    let src = r#"
#include <stdio.h>
int main() {
    int arr[] = {1, 2, 3};
    int sum = 0;
    for (int x : arr) {
        sum = sum + x;
    }
    printf("%d\n", sum);
    return 0;
}
"#;
    let (ret, outputs) = compile_and_run_cpp(src).expect("Compile/run failed");
    assert_eq!(ret, 0);
    assert_eq!(outputs, vec!["6"]);
}

#[test]
fn test_cpp_range_for_ref_modify() {
    let src = r#"
#include <stdio.h>
int main() {
    int arr[] = {1, 2, 3};
    for (auto& x : arr) {
        x = x * 2;
    }
    printf("%d %d %d\n", arr[0], arr[1], arr[2]);
    return 0;
}
"#;
    let (ret, outputs) = compile_and_run_cpp(src).expect("Compile/run failed");
    assert_eq!(ret, 0);
    assert_eq!(outputs, vec!["2 4 6"]);
}

#[test]
fn test_cpp_rvalue_ref() {
    let src = r#"
#include <stdio.h>
int foo() { return 42; }
int main() {
    int&& r = foo();
    printf("%d\n", r);
    return 0;
}
"#;
    let (ret, outputs) = compile_and_run_cpp(src).expect("Compile/run failed");
    assert_eq!(ret, 0);
    assert_eq!(outputs, vec!["42"]);
}

#[test]
fn test_cpp_const_ref_rvalue() {
    let src = r#"
#include <stdio.h>
int main() {
    const int& r = 5;
    printf("%d\n", r);
    return 0;
}
"#;
    let (ret, outputs) = compile_and_run_cpp(src).expect("Compile/run failed");
    assert_eq!(ret, 0);
    assert_eq!(outputs, vec!["5"]);
}

#[test]
fn test_cpp_virtual_call() {
    let src = r#"
#include <stdio.h>
class Base {
public:
    virtual int foo() { return 1; }
};
class Derived : public Base {
public:
    int foo() { return 2; }
};
int main() {
    Base* b = new Derived();
    printf("%d\n", b->foo());
    delete b;
    return 0;
}
"#;
    let (ret, outputs) = compile_and_run_cpp(src).expect("Compile/run failed");
    assert_eq!(ret, 0);
    assert_eq!(outputs, vec!["2"]);
}

#[test]
fn test_cpp_lambda_capture_by_value() {
    let src = r#"
#include <stdio.h>
int main() {
    int x = 5;
    auto f = [x](int y) { return x + y; };
    printf("%d\n", f(3));
    return 0;
}
"#;
    let (ret, outputs) = compile_and_run_cpp(src).expect("Compile/run failed");
    assert_eq!(ret, 0);
    assert_eq!(outputs, vec!["8"]);
}

#[test]
fn test_cpp_lambda_capture_by_reference() {
    let src = r#"
#include <stdio.h>
int main() {
    int x = 5;
    auto f = [&x](int y) { x = x + y; };
    f(3);
    printf("%d\n", x);
    return 0;
}
"#;
    let (ret, outputs) = compile_and_run_cpp(src).expect("Compile/run failed");
    assert_eq!(ret, 0);
    assert_eq!(outputs, vec!["8"]);
}

#[test]
fn test_cpp_delete_calls_dtor() {
    let src = r#"
#include <stdio.h>
class Flag {
public:
    int* p;
    Flag(int* ptr) { p = ptr; }
    ~Flag() { *p = 1; }
};
int main() {
    int flag = 0;
    Flag* f = new Flag(&flag);
    delete f;
    printf("%d\n", flag);
    return 0;
}
"#;
    let (ret, outputs) = compile_and_run_cpp(src).expect("Compile/run failed");
    assert_eq!(ret, 0);
    assert_eq!(outputs, vec!["1"]);
}

#[test]
fn test_cpp_range_for_vector() {
    let src = r#"
#include <stdio.h>
int main() {
    vitro_vec_int v;
    v.push_back(1);
    v.push_back(2);
    v.push_back(3);
    int sum = 0;
    for (auto x : v) {
        sum = sum + x;
    }
    printf("%d\n", sum);
    return 0;
}
"#;
    let (ret, outputs) = compile_and_run_cpp(src).expect("Compile/run failed");
    assert_eq!(ret, 0);
    assert_eq!(outputs, vec!["6"]);
}

#[test]
fn test_cpp_range_for_string() {
    let src = r#"
#include <stdio.h>
int main() {
    vitro_string s;
    s.push_back('a');
    s.push_back('b');
    s.push_back('c');
    for (auto c : s) {
        putchar(c);
    }
    putchar('\n');
    return 0;
}
"#;
    let (ret, outputs) = compile_and_run_cpp(src).expect("Compile/run failed");
    assert_eq!(ret, 0);
    assert_eq!(outputs, vec!["abc"]);
}

#[test]
fn test_cpp_class_template_box_int() {
    let src = r#"
#include <stdio.h>
template <class T> class Box {
public:
    T value;
};
int main() {
    Box<int> b;
    b.value = 42;
    printf("%d\n", b.value);
    return 0;
}
"#;
    let (ret, outputs) = compile_and_run_cpp(src).expect("Compile/run failed");
    assert_eq!(ret, 0);
    assert_eq!(outputs, vec!["42"]);
}

#[test]
fn test_cpp_class_template_adder_int() {
    let src = r#"
#include <stdio.h>
template <class T> class Adder {
public:
    T add(T a, T b) { return a + b; }
};
int main() {
    Adder<int> a;
    printf("%d\n", a.add(3, 4));
    return 0;
}
"#;
    let (ret, outputs) = compile_and_run_cpp(src).expect("Compile/run failed");
    assert_eq!(ret, 0);
    assert_eq!(outputs, vec!["7"]);
}

#[test]
fn test_cpp_class_template_wrapper_int_new() {
    let src = r#"
#include <stdio.h>
template <class T> class Wrapper {
public:
    T v;
    Wrapper(T x) { v = x; }
};
int main() {
    Wrapper<int>* w = new Wrapper<int>(10);
    printf("%d\n", w->v);
    delete w;
    return 0;
}
"#;
    let (ret, outputs) = compile_and_run_cpp(src).expect("Compile/run failed");
    assert_eq!(ret, 0);
    assert_eq!(outputs, vec!["10"]);
}

#[test]
fn test_cpp_class_template_ptr_int() {
    let src = r#"
#include <stdio.h>
template <class T> class Ptr {
public:
    T* p;
};
int main() {
    Ptr<int> ptr;
    ptr.p = new int;
    *ptr.p = 5;
    printf("%d\n", *ptr.p);
    delete ptr.p;
    return 0;
}
"#;
    let (ret, outputs) = compile_and_run_cpp(src).expect("Compile/run failed");
    assert_eq!(ret, 0);
    assert_eq!(outputs, vec!["5"]);
}

#[test]
fn test_cpp_new_int_with_init() {
    let src = r#"
#include <stdio.h>
int main() {
    int* p = new int(5);
    printf("%d\n", *p);
    delete p;
    return 0;
}
"#;
    let (ret, outputs) = compile_and_run_cpp(src).expect("Compile/run failed");
    assert_eq!(ret, 0);
    assert_eq!(outputs, vec!["5"]);
}

#[test]
fn test_cpp_container_vec_int() {
    let src = r#"
#include <stdio.h>
int main() {
    vitro_vec_int v;
    v.push_back(10);
    v.push_back(20);
    printf("%d\n", v.size());
    printf("%d\n", v.get(0));
    printf("%d\n", v.get(1));
    v.pop_back();
    printf("%d\n", v.size());
    return 0;
}
"#;
    let (ret, outputs) = compile_and_run_cpp(src).expect("Compile/run failed");
    assert_eq!(ret, 0);
    assert_eq!(outputs, vec!["2", "10", "20", "1"]);
}

#[test]
fn test_cpp_container_vec_float() {
    let src = r#"
#include <stdio.h>
int main() {
    vitro_vec_float v;
    v.push_back(15);
    v.push_back(25);
    printf("%.1f\n", v.get(0));
    printf("%.1f\n", v.get(1));
    return 0;
}
"#;
    let (ret, outputs) = compile_and_run_cpp(src).expect("Compile/run failed");
    assert_eq!(ret, 0);
    assert_eq!(outputs, vec!["15.0", "25.0"]);
}

#[test]
fn test_cpp_container_string() {
    let src = r#"
#include <stdio.h>
int main() {
    vitro_string s;
    s.push_back('h');
    s.push_back('i');
    printf("%d\n", s.size());
    printf("%c\n", s.get(0));
    printf("%c\n", s.get(1));
    s.pop_back();
    printf("%d\n", s.size());
    return 0;
}
"#;
    let (ret, outputs) = compile_and_run_cpp(src).expect("Compile/run failed");
    assert_eq!(ret, 0);
    assert_eq!(outputs, vec!["2", "h", "i", "1"]);
}

#[test]
fn test_cpp_builtin_layout_from_toml() {
    let layout = vitro_native::compiler::cpp_frontend::builtin_layout::builtin_class_layout("vitro_vec_int");
    assert!(layout.is_some(), "builtin_class_layout should return Some for vitro_vec_int");
    let layout = layout.unwrap();
    assert_eq!(layout.size, 12, "vitro_vec_int size should be 12");
    assert_eq!(layout.fields.len(), 3, "vitro_vec_int should have 3 fields");
    let method_names: Vec<_> = layout.methods.iter().map(|m| m.name.as_str()).collect();
    assert!(method_names.contains(&"push_back"), "vitro_vec_int should have push_back method");
    assert!(method_names.contains(&"size"), "vitro_vec_int should have size method");
    assert!(method_names.contains(&"get"), "vitro_vec_int should have get method");
}

#[test]
fn test_cpp_container_vec_char() {
    let src = r#"
#include <stdio.h>
int main() {
    vitro_vec_char v;
    v.push_back('a');
    v.push_back('b');
    printf("%d\n", v.size());
    printf("%c\n", v.get(0));
    printf("%c\n", v.get(1));
    v.pop_back();
    printf("%d\n", v.size());
    return 0;
}
"#;
    let (ret, outputs) = compile_and_run_cpp(src).expect("Compile/run failed");
    assert_eq!(ret, 0);
    assert_eq!(outputs, vec!["2", "a", "b", "1"]);
}

#[test]
fn test_cpp_container_list_int() {
    let src = r#"
#include <stdio.h>
int main() {
    vitro_list_int l;
    l.push_back(1);
    l.push_back(2);
    l.push_front(0);
    printf("%d\n", l.size());
    printf("%d\n", l.get(0));
    printf("%d\n", l.get(1));
    printf("%d\n", l.get(2));
    l.pop_back();
    printf("%d\n", l.size());
    return 0;
}
"#;
    let (ret, outputs) = compile_and_run_cpp(src).expect("Compile/run failed");
    assert_eq!(ret, 0);
    assert_eq!(outputs, vec!["3", "0", "1", "2", "2"]);
}

#[test]
fn test_cpp_sort_int() {
    let src = r#"
#include <stdio.h>

template<class T>
void sort_swap(T *a, T *b) {
    T t = *a;
    *a = *b;
    *b = t;
}

template<class T>
void sort_rec(T *a, int left, int right) {
    if (left >= right) return;
    T pivot = a[(left + right) / 2];
    int i = left;
    int j = right;
    while (i <= j) {
        while (a[i] < pivot) i++;
        while (a[j] > pivot) j--;
        if (i <= j) {
            sort_swap(&a[i], &a[j]);
            i++;
            j--;
        }
    }
    if (left < j) sort_rec(a, left, j);
    if (i < right) sort_rec(a, i, right);
}

template<class T>
void sort(T *a, int n) {
    if (n > 1) sort_rec(a, 0, n - 1);
}

int main() {
    int arr[] = {5, 2, 8, 1, 9};
    sort(arr, 5);
    for (int i = 0; i < 5; i++) {
        printf("%d\n", arr[i]);
    }
    return 0;
}
"#;
    let (ret, outputs) = compile_and_run_cpp(src).expect("Compile/run failed");
    assert_eq!(ret, 0);
    assert_eq!(outputs, vec!["1", "2", "5", "8", "9"]);
}

#[test]
fn test_cpp_type_map_lookup() {
    assert_eq!(
        vitro_native::compiler::cpp_frontend::type_map::cpp_type_to_vitro("vector<int>"),
        Some("vitro_vec_int")
    );
    assert_eq!(
        vitro_native::compiler::cpp_frontend::type_map::cpp_type_to_vitro("vector<float>"),
        Some("vitro_vec_float")
    );
    assert_eq!(
        vitro_native::compiler::cpp_frontend::type_map::cpp_type_to_vitro("string"),
        Some("vitro_string")
    );
    assert_eq!(
        vitro_native::compiler::cpp_frontend::type_map::map_container_method("vitro_vec_int", "push_back"),
        Some("vitro_vec_int__push_back")
    );
    assert_eq!(
        vitro_native::compiler::cpp_frontend::type_map::map_container_method("vitro_string", "push_back"),
        Some("vitro_string__push_back")
    );
}

// ============================================================================
// Stage 2: C++ 栈对象 RAII（构造函数自动调用 / 作用域退出自动析构）
// ============================================================================

#[test]
fn test_cpp_stack_ctor_dtor_basic() {
    let src = r#"
#include <stdio.h>
int g_ctor = 0;
int g_dtor = 0;
class Flag {
public:
    int x;
    Flag() { g_ctor++; x = 42; }
    ~Flag() { g_dtor++; }
};
void foo() {
    Flag f;
}
int main() {
    foo();
    printf("%d\n", g_ctor);
    printf("%d\n", g_dtor);
    return 0;
}
"#;
    let (ret, outputs) = compile_and_run_cpp(src).expect("Compile/run failed");
    assert_eq!(ret, 0);
    assert_eq!(outputs, vec!["1", "1"]);
}

#[test]
fn test_cpp_nested_scope_dtors_lifo() {
    let src = r#"
#include <stdio.h>
int g_log = 0;
void set_log(int v) { g_log = g_log * 10 + v; }
class A {
public:
    int id;
    A() { id = 0; }
    void init(int i) { id = i; }
    ~A() { set_log(id); }
};
void foo() {
    {
        A a1;
        a1.init(1);
        {
            A a2;
            a2.init(2);
        }
    }
}
int main() {
    foo();
    printf("%d\n", g_log);
    return 0;
}
"#;
    let (ret, outputs) = compile_and_run_cpp(src).expect("Compile/run failed");
    assert_eq!(ret, 0);
    assert_eq!(outputs, vec!["21"]);
}

#[test]
fn test_cpp_early_return_dtors() {
    let src = r#"
#include <stdio.h>
int g_log = 0;
void set_log(int v) { g_log = g_log * 10 + v; }
class A {
public:
    int id;
    A() { id = 0; }
    void init(int i) { id = i; }
    ~A() { set_log(id); }
};
void foo(int x) {
    A a1;
    a1.init(1);
    if (x > 0) {
        A a2;
        a2.init(2);
        return;
    }
    A a3;
    a3.init(3);
}
int main() {
    foo(1);
    printf("%d\n", g_log);
    return 0;
}
"#;
    let (ret, outputs) = compile_and_run_cpp(src).expect("Compile/run failed");
    assert_eq!(ret, 0);
    assert_eq!(outputs, vec!["21"]);
}

#[test]
fn test_cpp_break_dtors() {
    let src = r#"
#include <stdio.h>
int g_log = 0;
void set_log(int v) { g_log = g_log * 10 + v; }
class A {
public:
    int id;
    A() { id = 0; }
    void init(int i) { id = i; }
    ~A() { set_log(id); }
};
void foo() {
    for (int i = 0; i < 3; i++) {
        A a;
        a.init(i + 1);
        if (i == 1) {
            break;
        }
    }
}
int main() {
    foo();
    printf("%d\n", g_log);
    return 0;
}
"#;
    let (ret, outputs) = compile_and_run_cpp(src).expect("Compile/run failed");
    assert_eq!(ret, 0);
    assert_eq!(outputs, vec!["12"]);
}

#[test]
fn test_cpp_continue_dtors() {
    let src = r#"
#include <stdio.h>
int g_log = 0;
void set_log(int v) { g_log = g_log * 10 + v; }
class A {
public:
    int id;
    A() { id = 0; }
    void init(int i) { id = i; }
    ~A() { set_log(id); }
};
void foo() {
    for (int i = 0; i < 3; i++) {
        A a;
        a.init(i + 1);
        if (i == 1) {
            continue;
        }
    }
}
int main() {
    foo();
    printf("%d\n", g_log);
    return 0;
}
"#;
    let (ret, outputs) = compile_and_run_cpp(src).expect("Compile/run failed");
    assert_eq!(ret, 0);
    assert_eq!(outputs, vec!["123"]);
}

// ============================================================================
// Stage 3：new[] / delete[] 元素构造析构
// ============================================================================

#[test]
fn test_cpp_new_array_ctor_dtor() {
    let src = r#"
#include <stdio.h>
int g_ctor = 0;
int g_dtor = 0;
class A {
public:
    int id;
    A() { g_ctor++; }
    ~A() { g_dtor++; }
};
int main() {
    A* arr = new A[3];
    printf("%d\n", g_ctor);
    delete[] arr;
    printf("%d\n", g_dtor);
    return 0;
}
"#;
    let (ret, outputs) = compile_and_run_cpp(src).expect("Compile/run failed");
    assert_eq!(ret, 0);
    assert_eq!(outputs, vec!["3", "3"]);
}

#[test]
fn test_cpp_new_array_ctor_dtor_reverse_order() {
    let src = r#"
#include <stdio.h>
int g_log = 0;
void set_log(int v) { g_log = g_log * 10 + v; }
class A {
public:
    int id;
    A() { id = 0; }
    void init(int i) { id = i; }
    ~A() { set_log(id); }
};
int main() {
    A* arr = new A[3];
    arr[0].init(1);
    arr[1].init(2);
    arr[2].init(3);
    delete[] arr;
    printf("%d\n", g_log);
    return 0;
}
"#;
    let (ret, outputs) = compile_and_run_cpp(src).expect("Compile/run failed");
    assert_eq!(ret, 0);
    assert_eq!(outputs, vec!["321"]);
}

// ============================================================================
// Stage 4: Reference Tests
// ============================================================================

#[test]
fn test_cpp_reference_auto_deref() {
    let src = r#"
#include <stdio.h>
int main() {
    int x = 10;
    int& r = x;
    printf("%d\n", r);
    r = 20;
    printf("%d\n", r);
    return 0;
}
"#;
    let (ret, outputs) = compile_and_run_cpp(src).expect("Compile/run failed");
    assert_eq!(ret, 0);
    assert_eq!(outputs, vec!["10", "20"]);
}

#[test]
fn test_cpp_reference_modify_original() {
    let src = r#"
#include <stdio.h>
int main() {
    int x = 10;
    int& r = x;
    r = 20;
    printf("%d\n", x);
    return 0;
}
"#;
    let (ret, outputs) = compile_and_run_cpp(src).expect("Compile/run failed");
    assert_eq!(ret, 0);
    assert_eq!(outputs, vec!["20"]);
}

#[test]
fn test_cpp_reference_param() {
    let src = r#"
#include <stdio.h>
void inc(int& x) {
    x = x + 1;
}
int main() {
    int a = 5;
    inc(a);
    printf("%d\n", a);
    return 0;
}
"#;
    let (ret, outputs) = compile_and_run_cpp(src).expect("Compile/run failed");
    assert_eq!(ret, 0);
    assert_eq!(outputs, vec!["6"]);
}

#[test]
fn test_cpp_reference_return() {
    let src = r#"
#include <stdio.h>
int g_val = 42;
int& get_ref() {
    return g_val;
}
int main() {
    int& r = get_ref();
    printf("%d\n", r);
    r = 100;
    printf("%d\n", g_val);
    return 0;
}
"#;
    let (ret, outputs) = compile_and_run_cpp(src).expect("Compile/run failed");
    assert_eq!(ret, 0);
    assert_eq!(outputs, vec!["42", "100"]);
}

#[test]
fn test_cpp_method_ref_return_assign_and_chain() {
    let src = r#"
#include <stdio.h>
class Counter {
public:
    int x;
    Counter() { x = 0; }
    int& get() { return x; }
    Counter& add(int v) { x += v; return *this; }
};
int main() {
    Counter c;
    c.get() = 5;
    c.add(1).add(2).add(3);
    printf("%d\n", c.x);
    return 0;
}
"#;
    let (ret, outputs) = compile_and_run_cpp(src).expect("Compile/run failed");
    assert_eq!(ret, 0);
    assert_eq!(outputs, vec!["11"]);
}

#[test]
fn test_cpp_ref_param_access_private_member() {
    let src = r#"
#include <stdio.h>
class Pair {
    int a;
public:
    Pair() { a = 7; }
    void copy_from(Pair& o) { a = o.a; }
    int get() { return a; }
};
int main() {
    Pair x;
    Pair y;
    y.copy_from(x);
    printf("%d\n", y.get());
    return 0;
}
"#;
    let (ret, outputs) = compile_and_run_cpp(src).expect("Compile/run failed");
    assert_eq!(ret, 0);
    assert_eq!(outputs, vec!["7"]);
}

// ============================================================================
// Phase D: BytecodeGen 加固 — 嵌套 struct new size / 深层 RAII / goto 边界
// ============================================================================

#[test]
fn test_cpp_new_nested_struct_size() {
    let src = r#"
#include <stdio.h>
template<class T>
class list {
    struct Node {
        T data;
        Node* next;
    };
    Node* head;
public:
    list() : head((Node*)0) {}
    void push_back(T x) {
        Node* node = new Node;
        node->data = x;
        node->next = head;
        head = node;
    }
    T get(int i) {
        Node* p = head;
        while (i-- > 0 && p != (Node*)0) p = p->next;
        if (p == (Node*)0) return 0;
        return p->data;
    }
    ~list() {
        Node* p = head;
        while (p != (Node*)0) {
            Node* n = p->next;
            delete p;
            p = n;
        }
    }
};
int main() {
    list<int> l;
    l.push_back(10);
    l.push_back(20);
    printf("%d\n", l.get(0));
    printf("%d\n", l.get(1));
    return 0;
}
"#;
    let (ret, outputs) = compile_and_run_cpp(src).expect("Compile/run failed");
    assert_eq!(ret, 0);
    assert_eq!(outputs, vec!["20", "10"]);
}

#[test]
fn test_cpp_deep_nested_scope_raii() {
    let src = r#"
#include <stdio.h>
int g_log = 0;
void set_log(int v) { g_log = g_log * 10 + v; }
class A {
public:
    int id;
    A() { id = 0; }
    void init(int i) { id = i; }
    ~A() { set_log(id); }
};
void foo() {
    {
        A a1;
        a1.init(1);
        {
            A a2;
            a2.init(2);
            {
                A a3;
                a3.init(3);
                return;
            }
        }
    }
}
int main() {
    foo();
    printf("%d\n", g_log);
    return 0;
}
"#;
    let (ret, outputs) = compile_and_run_cpp(src).expect("Compile/run failed");
    assert_eq!(ret, 0);
    assert_eq!(outputs, vec!["321"]);
}

#[test]
fn test_cpp_goto_with_dtor_scope() {
    let src = r#"
#include <stdio.h>
int g_log = 0;
void set_log(int v) { g_log = g_log * 10 + v; }
class A {
public:
    int id;
    A() { id = 0; }
    void init(int i) { id = i; }
    ~A() { set_log(id); }
};
void foo() {
    A a1;
    a1.init(1);
    goto end;
    {
        A a2;
        a2.init(2);
    }
end:
    ;
}
int main() {
    foo();
    printf("%d\n", g_log);
    return 0;
}
"#;
    // 当前行为：Vitro 允许 goto 向前跳转，a1 在函数退出时析构，a2 所在块被跳过。
    // 记录为已知行为：不强制报错，但需确保不崩溃且已构造对象正确析构。
    let (ret, outputs) = compile_and_run_cpp(src).expect("Compile/run failed");
    assert_eq!(ret, 0);
    assert_eq!(outputs, vec!["1"]);
}

// ============================================================================
// Stage 2b: 类外方法定义 + 显式模板实例化
// ============================================================================

#[test]
fn test_cpp_out_of_line_method_definition() {
    let src = r#"
#include <stdio.h>
class Counter {
public:
    int value;
    Counter();
    void inc();
    int get();
};
Counter::Counter() { value = 0; }
void Counter::inc() { value = value + 1; }
int Counter::get() { return value; }
int main() {
    Counter c;
    c.inc();
    c.inc();
    printf("%d\n", c.get());
    return 0;
}
"#;
    let (ret, outputs) = compile_and_run_cpp(src).expect("Compile/run failed");
    assert_eq!(ret, 0);
    assert_eq!(outputs, vec!["2"]);
}

#[test]
fn test_cpp_explicit_class_template_instantiation() {
    let src = r#"
#include <stdio.h>
template <class T>
class Box {
public:
    T v;
    void set(T x);
    T get();
};
template <class T> void Box<T>::set(T x) { v = x; }
template <class T> T Box<T>::get() { return v; }
template class Box<int>;
int main() {
    Box<int> b;
    b.set(42);
    printf("%d\n", b.get());
    return 0;
}
"#;
    let (ret, outputs) = compile_and_run_cpp(src).expect("Compile/run failed");
    assert_eq!(ret, 0);
    assert_eq!(outputs, vec!["42"]);
}

#[test]
fn test_cpp_nttp_class() {
    let src = r#"
#include <stdio.h>
template<typename T, int N>
class Array {
public:
    T data[N];
    int size() { return N; }
};
int main() {
    Array<int, 5> a;
    printf("%d\n", a.size());
    return 0;
}
"#;
    let (ret, outputs) = compile_and_run_cpp(src).expect("Compile/run failed");
    assert_eq!(ret, 0);
    assert_eq!(outputs, vec!["5"]);
}

#[test]
fn test_cpp_copy_ctor() {
    let src = r#"
#include <stdio.h>
class Box {
public:
    int* p;
    Box(int v) { p = new int(v); }
    Box(const Box& other) { p = new int(*other.p); }
    ~Box() { delete p; }
    int value() { return *p; }
};
int main() {
    Box a(5);
    Box b(a);
    Box c = a;
    printf("%d %d %d\n", a.value(), b.value(), c.value());
    *b.p = 10;
    printf("%d %d %d\n", a.value(), b.value(), c.value());
    return 0;
}
"#;
    let (ret, outputs) = compile_and_run_cpp(src).expect("Compile/run failed");
    assert_eq!(ret, 0);
    assert_eq!(outputs, vec!["5 5 5", "5 10 5"]);
}

#[test]
fn test_cpp_copy_ctor_return_by_value() {
    let src = r#"
#include <stdio.h>
class Box {
public:
    int* p;
    Box(int v) { p = new int(v); }
    Box(const Box& other) { p = new int(*other.p); }
    ~Box() { delete p; }
    int value() { return *p; }
};
Box make_box(int v) {
    Box t(v);
    return t;
}
int main() {
    Box b = make_box(7);
    printf("%d\n", b.value());
    return 0;
}
"#;
    let (ret, outputs) = compile_and_run_cpp(src).expect("Compile/run failed");
    assert_eq!(ret, 0);
    assert_eq!(outputs, vec!["7"]);
}

#[test]
fn test_cpp_copy_ctor_pass_by_value() {
    let src = r#"
#include <stdio.h>
class Box {
public:
    int* p;
    Box(int v) { p = new int(v); }
    Box(const Box& other) { p = new int(*other.p); }
    ~Box() { delete p; }
    int value() { return *p; }
};
void print(Box x) {
    printf("%d\n", x.value());
}
int main() {
    Box a(5);
    print(a);
    printf("%d\n", a.value());
    return 0;
}
"#;
    let (ret, outputs) = compile_and_run_cpp(src).expect("Compile/run failed");
    assert_eq!(ret, 0);
    assert_eq!(outputs, vec!["5", "5"]);
}
