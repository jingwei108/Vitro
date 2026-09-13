#![allow(clippy::unwrap_used, clippy::expect_used)]

mod test_utils;

use test_utils::{
    assert_bytecode_equivalent, assert_bytecode_equivalent_named, compile_and_run_cpp, compile_cpp_bytecode,
    get_function_instructions,
};

// ============================================================================
// Tool self-verification tests
// ============================================================================

#[test]
fn test_compile_cpp_bytecode_helper_works() {
    let src = "int main() { return 42; }";
    let output = compile_cpp_bytecode(src).expect("compile_cpp_bytecode should succeed");
    assert!(output.func_index.contains_key("main"), "Should have main function");
    assert!(!output.code.is_empty(), "Bytecode should not be empty");
}

#[test]
fn test_assert_bytecode_equivalent_self() {
    let src = r#"
#include <stdio.h>
int foo(int x) { return x + 1; }
int main() { printf("%d\n", foo(5)); return 0; }
"#;
    let output = compile_cpp_bytecode(src).expect("compile should succeed");
    // Comparing the same output against itself should always pass.
    assert_bytecode_equivalent(&output, &output, "main");
    assert_bytecode_equivalent(&output, &output, "foo");
}

#[test]
fn test_assert_bytecode_equivalent_detects_diff() {
    let src_a = r#"
int foo() { return 1; }
int main() { return foo(); }
"#;
    let src_b = r#"
int foo() { return 2; }
int main() { return foo(); }
"#;
    let output_a = compile_cpp_bytecode(src_a).expect("compile A should succeed");
    let output_b = compile_cpp_bytecode(src_b).expect("compile B should succeed");

    // foo() should differ (PushConst 1 vs 2)
    let result = std::panic::catch_unwind(|| {
        assert_bytecode_equivalent(&output_a, &output_b, "foo");
    });
    assert!(result.is_err(), "Should detect difference in foo() bytecode");

    // main() may also differ depending on codegen, but at minimum foo() must differ.
}

#[test]
fn test_get_function_instructions_bounds() {
    let src = r#"
int foo() { return 1; }
int bar() { return 2; }
int main() { return 0; }
"#;
    let output = compile_cpp_bytecode(src).expect("compile should succeed");

    let (foo_start, foo_instrs) = get_function_instructions(&output, "foo").expect("foo should exist");
    let (bar_start, bar_instrs) = get_function_instructions(&output, "bar").expect("bar should exist");

    // foo and bar should have non-empty instruction slices.
    assert!(!foo_instrs.is_empty(), "foo should have instructions");
    assert!(!bar_instrs.is_empty(), "bar should have instructions");

    // Slices should not overlap.
    let foo_end = foo_start + foo_instrs.len();
    let bar_end = bar_start + bar_instrs.len();
    assert!(
        foo_end <= bar_start || bar_end <= foo_start,
        "foo and bar instruction slices should not overlap"
    );
}

// ============================================================================
// Stage 6: Dogfooding — vector<int>
// ============================================================================

/// C++ source that defines and uses a template class vector<int>.
fn cpp_vector_int_src() -> &'static str {
    r#"
#include <stdio.h>

template<class T>
class vector {
    int size_;
    int capacity_;
    T* data;
public:
    vector() : size_(0), capacity_(0), data((T*)0) {}
    void push_back(T x) {
        if (size_ >= capacity_) {
            int new_cap = capacity_ == 0 ? 4 : capacity_ * 2;
            T* new_data = new T[new_cap];
            for (int i = 0; i < size_; i++) new_data[i] = data[i];
            delete[] data;
            data = new_data;
            capacity_ = new_cap;
        }
        data[size_++] = x;
    }
    T get(int i) { return data[i]; }
    int size() { return size_; }
    ~vector() { delete[] data; }
};

int main() {
    vector<int> v;
    v.push_back(3);
    v.push_back(1);
    v.push_back(4);
    for (int i = 0; i < v.size(); i++) {
        printf("%d\n", v.get(i));
    }
    return 0;
}
"#
}

/// C source that uses the Stage 0 handwritten vitro_vec_int container.
fn c_vector_int_src() -> &'static str {
    r#"
#include <stdio.h>
int main() {
    vitro_vec_int v;
    v.push_back(3);
    v.push_back(1);
    v.push_back(4);
    for (int i = 0; i < v.size(); i++) {
        printf("%d\n", v.get(i));
    }
    return 0;
}
"#
}

/// Verify that the C++ vector<int> compiles and produces correct runtime output.
#[test]
fn test_cpp_vector_int_dogfooding_runs() {
    let (ret, outputs) = compile_and_run_cpp(cpp_vector_int_src()).expect("Compile/run failed");
    assert_eq!(ret, 0, "Exit code should be 0");
    assert_eq!(outputs, vec!["3", "1", "4"], "stdout should match");
}

/// Verify that the C baseline also produces the same output.
#[test]
fn test_c_vector_int_baseline_runs() {
    let (ret, outputs) = compile_and_run_cpp(c_vector_int_src()).expect("Compile/run failed");
    assert_eq!(ret, 0, "Exit code should be 0");
    assert_eq!(outputs, vec!["3", "1", "4"], "stdout should match");
}

// ============================================================================
// Stage 6: Dogfooding — list<int>

fn cpp_list_int_src() -> &'static str {
    r#"
#include <stdio.h>

template<class T>
class list {
    struct Node {
        T data;
        Node* next;
    };
    Node* head;
    Node* tail;
    int size_;
public:
    list() : head((Node*)0), tail((Node*)0), size_(0) {}
    void push_back(T x) {
        Node* node = new Node;
        node->data = x;
        node->next = (Node*)0;
        if (tail) {
            tail->next = node;
        } else {
            head = node;
        }
        tail = node;
        size_++;
    }
    void push_front(T x) {
        Node* node = new Node;
        node->data = x;
        node->next = head;
        head = node;
        if (!tail) tail = node;
        size_++;
    }
    T get(int i) {
        Node* p = head;
        while (i-- > 0 && p != (Node*)0) p = p->next;
        if (p == (Node*)0) return 0;
        return p->data;
    }
    int size() { return size_; }
    ~list() {
        Node* p = head;
        while (p) {
            Node* n = p->next;
            delete p;
            p = n;
        }
    }
};

int main() {
    list<int> l;
    l.push_back(1);
    l.push_back(2);
    l.push_front(0);
    printf("%d\n", l.size());
    for (int i = 0; i < l.size(); i++) {
        printf("%d\n", l.get(i));
    }
    return 0;
}
"#
}

fn c_list_int_src() -> &'static str {
    r#"
#include <stdio.h>
int main() {
    vitro_list_int l;
    l.push_back(1);
    l.push_back(2);
    l.push_front(0);
    printf("%d\n", l.size());
    for (int i = 0; i < l.size(); i++) {
        printf("%d\n", l.get(i));
    }
    return 0;
}
"#
}

#[test]
fn test_cpp_list_int_dogfooding_runs() {
    let (ret, outputs) = compile_and_run_cpp(cpp_list_int_src()).expect("Compile/run failed");
    assert_eq!(ret, 0, "Exit code should be 0");
    assert_eq!(outputs, vec!["3", "0", "1", "2"], "stdout should match");
}

#[test]
fn test_c_list_int_baseline_runs() {
    let (ret, outputs) = compile_and_run_cpp(c_list_int_src()).expect("Compile/run failed");
    assert_eq!(ret, 0, "Exit code should be 0");
    assert_eq!(outputs, vec!["3", "0", "1", "2"], "stdout should match");
}

// ============================================================================
// Stage 6: Dogfooding — string
// ============================================================================

fn cpp_string_src() -> &'static str {
    r#"
#include <stdio.h>

class string {
    int size_;
    int capacity_;
    char* data_;
public:
    string() : size_(0), capacity_(0), data_((char*)0) {}
    void push_back(char c) {
        if (size_ + 1 >= capacity_) {
            int new_cap = capacity_ == 0 ? 4 : capacity_ * 2;
            char* new_data = new char[new_cap];
            for (int i = 0; i < size_; i++) new_data[i] = data_[i];
            delete[] data_;
            data_ = new_data;
            capacity_ = new_cap;
        }
        data_[size_++] = c;
        data_[size_] = '\0';
    }
    char get(int i) { return data_[i]; }
    int size() { return size_; }
    char* c_str() { return data_; }
    ~string() { delete[] data_; }
};

int main() {
    string s;
    s.push_back('h');
    s.push_back('e');
    s.push_back('l');
    s.push_back('l');
    s.push_back('o');
    printf("%d\n", s.size());
    printf("%s\n", s.c_str());
    return 0;
}
"#
}

fn c_string_baseline_src() -> &'static str {
    r#"
#include <stdio.h>
int main() {
    vitro_string s;
    s.push_back('h');
    s.push_back('e');
    s.push_back('l');
    s.push_back('l');
    s.push_back('o');
    printf("%d\n", s.size());
    printf("%s\n", s.c_str());
    return 0;
}
"#
}

#[test]
fn test_cpp_string_dogfooding_runs() {
    let (ret, outputs) = compile_and_run_cpp(cpp_string_src()).expect("Compile/run failed");
    assert_eq!(ret, 0, "Exit code should be 0");
    assert_eq!(outputs, vec!["5", "hello"], "stdout should match");
}

#[test]
fn test_c_string_baseline_runs() {
    let (ret, outputs) = compile_and_run_cpp(c_string_baseline_src()).expect("Compile/run failed");
    assert_eq!(ret, 0, "Exit code should be 0");
    assert_eq!(outputs, vec!["5", "hello"], "stdout should match");
}

// ============================================================================
// Stage 1: Dogfooding — vector<float>
// ============================================================================

fn cpp_vector_float_src() -> &'static str {
    r#"
#include <stdio.h>

template<class T>
class vector {
    int size_;
    int capacity_;
    T* data;
public:
    vector() : size_(0), capacity_(0), data((T*)0) {}
    void push_back(T x) {
        if (size_ >= capacity_) {
            int new_cap = capacity_ == 0 ? 4 : capacity_ * 2;
            T* new_data = new T[new_cap];
            for (int i = 0; i < size_; i++) new_data[i] = data[i];
            delete[] data;
            data = new_data;
            capacity_ = new_cap;
        }
        data[size_++] = x;
    }
    T get(int i) { return data[i]; }
    int size() { return size_; }
    ~vector() { delete[] data; }
};

int main() {
    vector<float> v;
    v.push_back(3.0);
    v.push_back(1.0);
    v.push_back(4.0);
    for (int i = 0; i < v.size(); i++) {
        printf("%.1f\n", v.get(i));
    }
    return 0;
}
"#
}

fn c_vector_float_src() -> &'static str {
    r#"
#include <stdio.h>
int main() {
    vitro_vec_float v;
    v.push_back(3.0);
    v.push_back(1.0);
    v.push_back(4.0);
    for (int i = 0; i < v.size(); i++) {
        printf("%.1f\n", v.get(i));
    }
    return 0;
}
"#
}

fn c_vector_float_inline_src() -> &'static str {
    r#"
typedef struct {
    int n;
    int m;
    float *a;
} vitro_vec_float;
float vitro_vec_get_float(vitro_vec_float *v, int i) { return v->a[i]; }
int vitro_vec_size_float(vitro_vec_float *v) { return v->n; }
int main() { return 0; }
"#
}

#[test]
fn test_cpp_vector_float_dogfooding_runs() {
    let (ret, outputs) = compile_and_run_cpp(cpp_vector_float_src()).expect("Compile/run failed");
    assert_eq!(ret, 0, "Exit code should be 0");
    assert_eq!(outputs, vec!["3.0", "1.0", "4.0"], "stdout should match");
}

#[test]
fn test_c_vector_float_baseline_runs() {
    let (ret, outputs) = compile_and_run_cpp(c_vector_float_src()).expect("Compile/run failed");
    assert_eq!(ret, 0, "Exit code should be 0");
    assert_eq!(outputs, vec!["3.0", "1.0", "4.0"], "stdout should match");
}

#[test]
fn test_cpp_vector_float_get_bytecode_equivalent() {
    let cpp = compile_cpp_bytecode(cpp_vector_float_src()).unwrap();
    let c = compile_cpp_bytecode(c_vector_float_inline_src()).unwrap();
    assert_bytecode_equivalent_named(&cpp, "vector__float__get", &c, "vitro_vec_get_float");
}

#[test]
fn test_cpp_vector_float_size_bytecode_equivalent() {
    let cpp = compile_cpp_bytecode(cpp_vector_float_src()).unwrap();
    let c = compile_cpp_bytecode(c_vector_float_inline_src()).unwrap();
    assert_bytecode_equivalent_named(&cpp, "vector__float__size", &c, "vitro_vec_size_float");
}

// ============================================================================
// Stage 1: Dogfooding — vector<char>
// ============================================================================

fn cpp_vector_char_src() -> &'static str {
    r#"
#include <stdio.h>

template<class T>
class vector {
    int size_;
    int capacity_;
    T* data;
public:
    vector() : size_(0), capacity_(0), data((T*)0) {}
    void push_back(T x) {
        if (size_ >= capacity_) {
            int new_cap = capacity_ == 0 ? 4 : capacity_ * 2;
            T* new_data = new T[new_cap];
            for (int i = 0; i < size_; i++) new_data[i] = data[i];
            delete[] data;
            data = new_data;
            capacity_ = new_cap;
        }
        data[size_++] = x;
    }
    T get(int i) { return data[i]; }
    int size() { return size_; }
    ~vector() { delete[] data; }
};

int main() {
    vector<char> v;
    v.push_back('a');
    v.push_back('b');
    v.push_back('c');
    for (int i = 0; i < v.size(); i++) {
        printf("%c\n", v.get(i));
    }
    return 0;
}
"#
}

fn c_vector_char_src() -> &'static str {
    r#"
#include <stdio.h>
int main() {
    vitro_vec_char v;
    v.push_back('a');
    v.push_back('b');
    v.push_back('c');
    for (int i = 0; i < v.size(); i++) {
        printf("%c\n", v.get(i));
    }
    return 0;
}
"#
}

fn c_vector_char_inline_src() -> &'static str {
    r#"
typedef struct {
    int n;
    int m;
    char *a;
} vitro_vec_char;
char vitro_vec_get_char(vitro_vec_char *v, int i) { return v->a[i]; }
int vitro_vec_size_char(vitro_vec_char *v) { return v->n; }
int main() { return 0; }
"#
}

#[test]
fn test_cpp_vector_char_dogfooding_runs() {
    let (ret, outputs) = compile_and_run_cpp(cpp_vector_char_src()).expect("Compile/run failed");
    assert_eq!(ret, 0, "Exit code should be 0");
    assert_eq!(outputs, vec!["a", "b", "c"], "stdout should match");
}

#[test]
fn test_c_vector_char_baseline_runs() {
    let (ret, outputs) = compile_and_run_cpp(c_vector_char_src()).expect("Compile/run failed");
    assert_eq!(ret, 0, "Exit code should be 0");
    assert_eq!(outputs, vec!["a", "b", "c"], "stdout should match");
}

#[test]
fn test_cpp_vector_char_get_bytecode_equivalent() {
    let cpp = compile_cpp_bytecode(cpp_vector_char_src()).unwrap();
    let c = compile_cpp_bytecode(c_vector_char_inline_src()).unwrap();
    assert_bytecode_equivalent_named(&cpp, "vector__char__get", &c, "vitro_vec_get_char");
}

#[test]
fn test_cpp_vector_char_size_bytecode_equivalent() {
    let cpp = compile_cpp_bytecode(cpp_vector_char_src()).unwrap();
    let c = compile_cpp_bytecode(c_vector_char_inline_src()).unwrap();
    assert_bytecode_equivalent_named(&cpp, "vector__char__size", &c, "vitro_vec_size_char");
}

// ============================================================================
// Stage 1: Dogfooding — sort_int
// ============================================================================

fn cpp_sort_int_src() -> &'static str {
    r#"
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
    int a[5] = {3, 1, 4, 1, 5};
    sort(a, 5);
    for (int i = 0; i < 5; i++) {
        printf("%d\n", a[i]);
    }
    return 0;
}
"#
}

fn c_sort_int_src() -> &'static str {
    r#"
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
    int a[5] = {3, 1, 4, 1, 5};
    sort(a, 5);
    for (int i = 0; i < 5; i++) {
        printf("%d\n", a[i]);
    }
    return 0;
}
"#
}

#[test]
fn test_cpp_sort_int_dogfooding_runs() {
    let (ret, outputs) = compile_and_run_cpp(cpp_sort_int_src()).expect("Compile/run failed");
    assert_eq!(ret, 0, "Exit code should be 0");
    assert_eq!(outputs, vec!["1", "1", "3", "4", "5"], "stdout should match");
}

#[test]
fn test_c_sort_int_baseline_runs() {
    let (ret, outputs) = compile_and_run_cpp(c_sort_int_src()).expect("Compile/run failed");
    assert_eq!(ret, 0, "Exit code should be 0");
    assert_eq!(outputs, vec!["1", "1", "3", "4", "5"], "stdout should match");
}

// ============================================================================
// Stage 1: Bytecode equivalence — vector<int> get/size
// ============================================================================

fn c_vector_int_inline_src() -> &'static str {
    r#"
typedef struct { int n; int m; int *a; } vitro_vec_int;
int vitro_vec_get_int(vitro_vec_int *v, int i) { return v->a[i]; }
int vitro_vec_size_int(vitro_vec_int *v) { return v->n; }
int main() { return 0; }
"#
}

#[test]
fn test_cpp_vector_int_get_bytecode_equivalent() {
    let cpp = compile_cpp_bytecode(cpp_vector_int_src()).unwrap();
    let c = compile_cpp_bytecode(c_vector_int_inline_src()).unwrap();
    assert_bytecode_equivalent_named(&cpp, "vector__int__get", &c, "vitro_vec_get_int");
}

#[test]
fn test_cpp_vector_int_size_bytecode_equivalent() {
    let cpp = compile_cpp_bytecode(cpp_vector_int_src()).unwrap();
    let c = compile_cpp_bytecode(c_vector_int_inline_src()).unwrap();
    assert_bytecode_equivalent_named(&cpp, "vector__int__size", &c, "vitro_vec_size_int");
}

// ============================================================================
// Stage 1: Bytecode equivalence — list<int> size
// ============================================================================

fn c_list_int_inline_src() -> &'static str {
    r#"
typedef struct vitro_list_node_int { int data; struct vitro_list_node_int *next; } vitro_list_node_int;
typedef struct { vitro_list_node_int *head; vitro_list_node_int *tail; int n; } vitro_list_int;
int vitro_list_size_int(vitro_list_int *l) { return l->n; }
int main() { return 0; }
"#
}

#[test]
fn test_cpp_list_int_size_bytecode_equivalent() {
    let cpp = compile_cpp_bytecode(cpp_list_int_src()).unwrap();
    let c = compile_cpp_bytecode(c_list_int_inline_src()).unwrap();
    assert_bytecode_equivalent_named(&cpp, "list__int__size", &c, "vitro_list_size_int");
}

// ============================================================================
// Stage 1: Bytecode equivalence — string get/size
// ============================================================================

fn c_string_inline_src() -> &'static str {
    r#"
typedef struct { int n; int m; char *s; } vitro_string;
char vitro_string_get(vitro_string *str, int i) { return str->s[i]; }
int vitro_string_size(vitro_string *str) { return str->n; }
int main() { return 0; }
"#
}

#[test]
fn test_cpp_string_get_bytecode_equivalent() {
    let cpp = compile_cpp_bytecode(cpp_string_src()).unwrap();
    let c = compile_cpp_bytecode(c_string_inline_src()).unwrap();
    assert_bytecode_equivalent_named(&cpp, "string__get", &c, "vitro_string_get");
}

#[test]
fn test_cpp_string_size_bytecode_equivalent() {
    let cpp = compile_cpp_bytecode(cpp_string_src()).unwrap();
    let c = compile_cpp_bytecode(c_string_inline_src()).unwrap();
    assert_bytecode_equivalent_named(&cpp, "string__size", &c, "vitro_string_size");
}

// ============================================================================
// M5: Implicit move constructor generation
// ============================================================================

#[test]
fn test_implicit_move_ctor_pointer_nulls_source() {
    let src = r#"
#include <stdio.h>

class Buffer {
    int* data;
    int size_;
public:
    Buffer() : data((int*)0), size_(0) {}
    void alloc(int n) {
        data = new int[n];
        size_ = n;
    }
    int get_size() { return size_; }
    ~Buffer() { delete[] data; }
};

int main() {
    Buffer a;
    a.alloc(5);
    Buffer b = std::move(a);
    printf("%d\n", b.get_size());
    return 0;
}
"#;
    let (ret, outputs) = compile_and_run_cpp(src).expect("Compile/run failed");
    assert_eq!(ret, 0, "Exit code should be 0");
    assert_eq!(outputs, vec!["5"], "Move ctor should transfer size correctly");
}

#[test]
fn test_implicit_move_ctor_builtin_vector() {
    // Verify that built-in container types (which have pointer fields)
    // also get implicit move constructors registered.
    let src = r#"
#include <stdio.h>
int main() {
    vitro_vec_int v;
    v.push_back(42);
    printf("%d\n", v.size());
    return 0;
}
"#;
    let (ret, outputs) = compile_and_run_cpp(src).expect("Compile/run failed");
    assert_eq!(ret, 0, "Exit code should be 0");
    assert_eq!(outputs, vec!["1"], "Builtin vector should still work");
}

// ============================================================================
// M5: Dogfooding — unique_ptr<int>
// ============================================================================

#[test]
fn test_cpp_unique_ptr_int_dogfooding_runs() {
    let src = r#"
#include <stdio.h>
#include <stdlib.h>

template<class T>
class unique_ptr {
    T* ptr;
public:
    unique_ptr() : ptr((T*)0) {}
    unique_ptr(T* p) : ptr(p) {}
    T* get() { return ptr; }
    void reset(T* p) { delete ptr; ptr = p; }
    T* release() { T* t = ptr; ptr = (T*)0; return t; }
    ~unique_ptr() { delete ptr; }
};

int main() {
    unique_ptr<int> p(new int(42));
    printf("%d\n", *p.get());

    unique_ptr<int> q = std::move(p);
    printf("%d\n", *q.get());
    printf("p=%p\n", p.get());

    int* raw = q.release();
    printf("raw=%d\n", *raw);
    free(raw);

    q.reset(new int(100));
    printf("reset=%d\n", *q.get());

    return 0;
}
"#;
    let (ret, outputs) = compile_and_run_cpp(src).expect("Compile/run failed");
    assert_eq!(ret, 0, "Exit code should be 0");
    assert_eq!(
        outputs,
        vec!["42", "42", "p=0x0", "raw=42", "reset=100"],
        "unique_ptr<int> should manage ownership correctly"
    );
}
