import ctypes, sys, io
sys.stdout = io.TextIOWrapper(sys.stdout.buffer, encoding='utf-8', errors='replace')
DLL = ctypes.CDLL("../../target/release/vitro_native.dll")
DLL.vitro_session_create.restype = ctypes.c_void_p
DLL.vitro_session_destroy.argtypes = [ctypes.c_void_p]
DLL.vitro_compile.argtypes = [ctypes.c_void_p, ctypes.c_char_p]
DLL.vitro_compile.restype = ctypes.c_int
DLL.vitro_run.argtypes = [ctypes.c_void_p]
DLL.vitro_run.restype = ctypes.c_int
DLL.vitro_get_program_output_length.argtypes = [ctypes.c_void_p]
DLL.vitro_get_program_output_length.restype = ctypes.c_int
DLL.vitro_get_program_output.argtypes = [ctypes.c_void_p, ctypes.c_char_p, ctypes.c_int]

results = []

def test(name, src):
    s = ctypes.c_void_p(DLL.vitro_session_create())
    ret = DLL.vitro_compile(s, src.encode())
    if ret != 0:
        results.append((name, "FAIL", ""))
        DLL.vitro_session_destroy(s)
        return
    DLL.vitro_run(s)
    # E-P1-5：纯程序 stdout 通道，不再做文本清洗。
    n = DLL.vitro_get_program_output_length(s)
    buf = ctypes.create_string_buffer(n+1)
    DLL.vitro_get_program_output(s, buf, n+1)
    out = buf.value.decode("utf-8", "replace").strip()
    results.append((name, "OK", out))
    DLL.vitro_session_destroy(s)

cases = [
    ("multi_if_else", 'int main() { int x = 2; if (x == 1) printf("one"); else if (x == 2) printf("two"); else printf("other"); return 0; }'),
    ("if_nested", 'int main() { int a = 1, b = 2; if (a == 1) { if (b == 2) printf("yes"); } return 0; }'),
    ("switch_default_first", 'int main() { int x = 5; switch(x) { default: printf("def"); break; case 1: printf("one"); break; } return 0; }'),
    ("for_no_init", 'int main() { int i = 0; for (; i < 3; i++) printf("%d", i); return 0; }'),
    ("for_no_incr", 'int main() { int i = 0; for (; i < 3; ) { printf("%d", i); i++; } return 0; }'),
    ("while_nested", 'int main() { int i = 0; while (i < 2) { int j = 0; while (j < 2) { printf("%d", i+j); j++; } i++; } return 0; }'),
    ("do_while_continue", 'int main() { int i = 0; do { i++; if (i == 2) continue; printf("%d", i); } while (i < 4); return 0; }'),
    ("break_in_nested_loop", 'int main() { for (int i = 0; i < 3; i++) { for (int j = 0; j < 3; j++) { if (j == 1) break; printf("%d", i+j); } } return 0; }'),
    ("short_circuit_and", 'int f() { printf("call"); return 1; } int main() { int x = 0; if (x && f()) printf("yes"); printf("done"); return 0; }'),
    ("short_circuit_or", 'int f() { printf("call"); return 1; } int main() { int x = 1; if (x || f()) printf("yes"); printf("done"); return 0; }'),
    ("ternary_as_expr", 'int main() { int a = 5, b = 3; int m = (a > b ? a : b) + 1; printf("%d", m); return 0; }'),
    ("comma_expr", 'int main() { int a = (1, 2, 3); printf("%d", a); return 0; }'),
    ("assign_chain", 'int main() { int a, b, c; a = b = c = 5; printf("%d %d %d", a, b, c); return 0; }'),
    ("precedence", 'int main() { printf("%d", 2 + 3 * 4); return 0; }'),
    ("parentheses_priority", 'int main() { printf("%d", (2 + 3) * 4); return 0; }'),
    ("post_inc_vs_pre_inc", 'int main() { int i = 5; printf("%d", i++); printf("%d", ++i); return 0; }'),
    ("neg_modulo", 'int main() { printf("%d", -17 % 5); return 0; }'),
    ("bitwise_combo", 'int main() { printf("%d", ((5 & 3) | 6) ^ 1); return 0; }'),
    ("shift_zero", 'int main() { printf("%d", 1 << 0); return 0; }'),
    ("shift_31", 'int main() { printf("%d", 1 << 31); return 0; }'),
    ("not_zero", 'int main() { printf("%d", ~0); return 0; }'),
    ("float_neg", 'int main() { float f = -3.14f; printf("%.2f", f); return 0; }'),
    ("double_compare", 'int main() { double d = 3.14; printf("%d", d > 3.0); return 0; }'),
    ("double_array_op", 'int main() { double arr[2] = {1.5, 2.5}; printf("%.1f", arr[0] + arr[1]); return 0; }'),
    ("long_long_arith", 'int main() { long long a = 10, b = 3; printf("%lld", a / b); return 0; }'),
    ("long_long_compare", 'int main() { long long a = 5, b = 3; printf("%d", a > b); return 0; }'),
    ("long_long_array", 'int main() { long long arr[3] = {1, 2, 3}; printf("%lld", arr[1]); return 0; }'),
    ("union_float_member", 'union U { int i; float f; }; int main() { union U u; u.f = 3.14f; printf("%.2f", u.f); return 0; }'),
    ("struct_nested", 'struct Outer { struct Inner { int x; } inner; }; int main() { struct Outer o; o.inner.x = 42; printf("%d", o.inner.x); return 0; }'),
    ("struct_array_init", 'struct S { int x; }; int main() { struct S arr[2] = {{1}, {2}}; printf("%d", arr[1].x); return 0; }'),
    ("enum_non_continuous", 'enum E { A = 10, B = 20 }; int main() { printf("%d", B); return 0; }'),
    ("sizeof_typedef", 'typedef int Integer; int main() { printf("%d", sizeof(Integer)); return 0; }'),
    ("sizeof_enum", 'enum Color { R, G }; int main() { printf("%d", sizeof(enum Color)); return 0; }'),
    ("zero_init_array", 'int main() { int a[5] = {0}; printf("%d", a[4]); return 0; }'),
    ("string_array", 'int main() { char* arr[2] = {"a", "b"}; printf("%s", arr[1]); return 0; }'),
    ("pointer_array", 'int main() { int x = 1, y = 2; int* arr[2] = {&x, &y}; printf("%d", *arr[1]); return 0; }'),
    ("array_no_size", 'int main() { int a[] = {1, 2, 3}; printf("%d", a[2]); return 0; }'),
    ("multi_dim_partial_init", 'int main() { int a[2][2] = {{1}, {3, 4}}; printf("%d", a[1][1]); return 0; }'),
    ("global_var_init", 'int g = 7; int main() { printf("%d", g); return 0; }'),
    ("static_local", 'int main() { static int s = 5; s++; printf("%d", s); return 0; }'),
    ("static_global", 'static int g = 3; int main() { printf("%d", g); return 0; }'),
    ("typedef_chain", 'typedef int A; typedef A B; int main() { B x = 5; printf("%d", x); return 0; }'),
    ("const_pointer", 'int main() { const int* p; int x = 5; p = &x; printf("%d", *p); return 0; }'),
    ("void_param", 'int f(void) { return 1; } int main() { printf("%d", f()); return 0; }'),
    ("func_many_params", 'int sum(int a, int b, int c, int d, int e) { return a+b+c+d+e; } int main() { printf("%d", sum(1,2,3,4,5)); return 0; }'),
    ("func_return_ptr", 'int* f() { static int x = 42; return &x; } int main() { printf("%d", *f()); return 0; }'),
    ("array_of_struct_ptr", 'struct S { int x; }; int main() { struct S s1, s2; struct S* arr[2] = {&s1, &s2}; arr[0]->x = 5; printf("%d", arr[0]->x); return 0; }'),
    ("ptr_to_array", 'int main() { int arr[3] = {1,2,3}; int (*p)[3] = &arr; printf("%d", (*p)[1]); return 0; }'),
    ("printf_char", 'int main() { printf("%c", \'A\'); return 0; }'),
    ("printf_hex", 'int main() { printf("%x", 255); return 0; }'),
    ("printf_ptr", 'int main() { int x = 5; printf("%p", &x); return 0; }'),
    ("printf_width", 'int main() { printf("%5d", 42); return 0; }'),
    ("printf_left_align", 'int main() { printf("%-5d", 42); return 0; }'),
    ("printf_zero_pad", 'int main() { printf("%05d", 42); return 0; }'),
    ("printf_string_width", 'int main() { printf("%5s", "hi"); return 0; }'),
    ("fprintf_stderr", 'int main() { fprintf(stderr, "err"); return 0; }'),
    ("realloc_shrink", 'int main() { int* p = malloc(8); p[0] = 1; p[1] = 2; p = realloc(p, 4); printf("%d", p[0]); free(p); return 0; }'),
    ("memset_struct", 'struct S { int a; int b; }; int main() { struct S s; memset(&s, 0, sizeof(s)); printf("%d", s.a); return 0; }'),
    ("strcat_empty", 'int main() { char a[10] = "a"; strcat(a, ""); printf("%s", a); return 0; }'),
    ("atoi_negative", 'int main() { printf("%d", atoi("-42")); return 0; }'),
    ("exit_nonzero", 'int main() { exit(1); printf("no"); return 0; }'),
    ("define_const_float", '#define PI 3.14f\nint main() { printf("%.2f", PI); return 0; }'),
    ("define_in_expr", '#define TEN 10\nint main() { printf("%d", TEN + 5); return 0; }'),
    ("empty_func", 'void f() {} int main() { f(); printf("ok"); return 0; }'),
    ("label_no_goto", 'int main() { int x = 1; end: printf("%d", x); return 0; }'),
    ("switch_empty", 'int main() { int x = 1; switch(x) {} printf("ok"); return 0; }'),
    ("switch_fallthrough", 'int main() { int x = 1; switch(x) { case 1: case 2: printf("ok"); break; } return 0; }'),
    ("float_zero_compare", 'int main() { float f = 0.0f; printf("%d", f == 0.0f); return 0; }'),
    ("double_assign", 'int main() { double d = 3.14; d = 2.71; printf("%.2f", d); return 0; }'),
    ("int_div", 'int main() { printf("%d", 7 / 3); return 0; }'),
    ("int_div_neg", 'int main() { printf("%d", -7 / 3); return 0; }'),
    ("mul_overflow_safe", 'int main() { printf("%d", 1000 * 1000); return 0; }'),
    ("compare_eq", 'int main() { printf("%d", 5 == 5); return 0; }'),
    ("compare_lt_eq", 'int main() { printf("%d", 5 <= 5); return 0; }'),
    ("compare_gt_eq", 'int main() { printf("%d", 5 >= 3); return 0; }'),
    ("bool_from_compare", 'int main() { int b = 5 > 3; printf("%d", b); return 0; }'),
    ("char_signedness", 'int main() { char c = -1; printf("%d", c); return 0; }'),
    ("array_bounds_safe", 'int main() { int a[5] = {0,1,2,3,4}; printf("%d", a[4]); return 0; }'),
    ("ptr_init_null", 'int main() { int* p = NULL; printf("%d", p == NULL); return 0; }'),
    ("struct_copy", 'struct S { int a; int b; }; int main() { struct S s1 = {1, 2}; struct S s2 = s1; printf("%d %d", s2.a, s2.b); return 0; }'),
    ("union_size_access", 'union U { char c; int i; }; int main() { printf("%d", sizeof(union U)); return 0; }'),
    ("nested_ternary", 'int main() { int a = 1, b = 2, c = 3; int r = a ? b : c; printf("%d", r); return 0; }'),
    ("loop_var_shadow", 'int main() { int i = 10; for (int i = 0; i < 3; i++) printf("%d", i); printf("%d", i); return 0; }'),
    ("func_ptr_param_decay", 'int sum(int a[]) { int s = 0; for (int i = 0; i < 3; i++) s += a[i]; return s; } int main() { int arr[3] = {1,2,3}; printf("%d", sum(arr)); return 0; }'),
]
for name, src in cases:
    test(name, src)

print("\n=== PASSED ===")
passed = [r for r in results if r[1] == "OK"]
failed = [r for r in results if r[1] == "FAIL"]
for name, status, out in passed:
    print(f"{name}: {out}")
print(f"\nPassed: {len(passed)}, Failed: {len(failed)}")
if failed:
    print("\n=== FAILED ===")
    for name, status, out in failed:
        print(f"{name}")
