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
    ("double_init_zero", 'int main() { double d = 0.0; printf("%.0f", d); return 0; }'),
    ("float_init_zero", 'int main() { float f = 0.0f; printf("%.0f", f); return 0; }'),
    ("long_long_init_zero", 'int main() { long long ll = 0; printf("%lld", ll); return 0; }'),
    ("long_long_neg", 'int main() { long long ll = -5; printf("%lld", ll); return 0; }'),
    ("sizeof_double_arr", 'int main() { double a[3]; printf("%d", sizeof(a)); return 0; }'),
    ("sizeof_float_arr", 'int main() { float a[3]; printf("%d", sizeof(a)); return 0; }'),
    ("sizeof_long_long", 'int main() { printf("%d", sizeof(long long)); return 0; }'),
    ("union_int_member", 'union U { int i; float f; }; int main() { union U u; u.i = 42; printf("%d", u.i); return 0; }'),
    ("struct_member_array", 'struct S { int arr[3]; }; int main() { struct S s; s.arr[1] = 5; printf("%d", s.arr[1]); return 0; }'),
    ("ptr_to_struct_member", 'struct S { int x; }; int main() { struct S s; struct S* p = &s; p->x = 7; printf("%d", p->x); return 0; }'),
    ("array_of_union", 'union U { int i; }; int main() { union U arr[2]; arr[0].i = 1; arr[1].i = 2; printf("%d", arr[1].i); return 0; }'),
    ("global_union", 'union U { int i; }; union U gu; int main() { gu.i = 9; printf("%d", gu.i); return 0; }'),
    ("global_enum", 'enum E { A, B }; enum E ge = B; int main() { printf("%d", ge); return 0; }'),
    ("typedef_enum", 'typedef enum { X, Y } E; int main() { E e = Y; printf("%d", e); return 0; }'),
    ("ptr_arith_char", 'int main() { char arr[3] = {\'a\', \'b\', \'c\'}; char* p = arr; p++; printf("%c", *p); return 0; }'),
    ("ptr_arith_struct", 'struct S { int x; }; int main() { struct S arr[2]; struct S* p = arr; p++; printf("%d", p == &arr[1]); return 0; }'),
    ("func_void_ptr", 'void* f() { static int x = 1; return &x; } int main() { printf("%d", *(int*)f()); return 0; }'),
    ("nested_ternary2", 'int main() { int a = 1, b = 2, c = 3; printf("%d", a > b ? c : (b > c ? a : b)); return 0; }'),
    ("for_break", 'int main() { for (int i = 0; i < 10; i++) { if (i == 3) break; printf("%d", i); } return 0; }'),
    ("for_continue", 'int main() { for (int i = 0; i < 5; i++) { if (i == 2) continue; printf("%d", i); } return 0; }'),
    ("while_continue", 'int main() { int i = 0; while (i < 5) { i++; if (i == 2) continue; printf("%d", i); } return 0; }'),
    ("do_while_break", 'int main() { int i = 0; do { if (i == 2) break; printf("%d", i); i++; } while (i < 5); return 0; }'),
    ("empty_for", 'int main() { for (int i = 0; i < 0; i++) printf("no"); printf("ok"); return 0; }'),
    ("empty_while", 'int main() { while (0) printf("no"); printf("ok"); return 0; }'),
    ("empty_if", 'int main() { if (0) printf("no"); printf("ok"); return 0; }'),
    ("negate", 'int main() { int x = 5; printf("%d", -x); return 0; }'),
    ("bit_not_zero", 'int main() { int x = 0; printf("%d", ~x); return 0; }'),
    ("bit_xor", 'int main() { printf("%d", 5 ^ 3); return 0; }'),
    ("shl_rhs", 'int main() { int x = 2; printf("%d", 1 << x); return 0; }'),
    ("shr_rhs", 'int main() { int x = 2; printf("%d", 8 >> x); return 0; }'),
    ("div_by_var", 'int main() { int x = 3; printf("%d", 10 / x); return 0; }'),
    ("mod_by_var", 'int main() { int x = 4; printf("%d", 10 % x); return 0; }'),
    ("mul_by_var", 'int main() { int x = 5; printf("%d", x * 6); return 0; }'),
    ("sub_by_var", 'int main() { int x = 3; printf("%d", 10 - x); return 0; }'),
    ("compare_var", 'int main() { int a = 5, b = 3; printf("%d", a > b); return 0; }'),
    ("equal_var", 'int main() { int a = 5, b = 5; printf("%d", a == b); return 0; }'),
    ("not_equal_var", 'int main() { int a = 5, b = 3; printf("%d", a != b); return 0; }'),
    ("logical_and_var", 'int main() { int a = 1, b = 0; printf("%d", a && b); return 0; }'),
    ("logical_or_var", 'int main() { int a = 0, b = 0; printf("%d", a || b); return 0; }'),
    ("logical_not_var", 'int main() { int a = 1; printf("%d", !a); return 0; }'),
    ("increment_var", 'int main() { int x = 5; x++; printf("%d", x); return 0; }'),
    ("decrement_var", 'int main() { int x = 5; x--; printf("%d", x); return 0; }'),
    ("compound_add_var", 'int main() { int x = 5; x += 3; printf("%d", x); return 0; }'),
    ("compound_sub_var", 'int main() { int x = 5; x -= 3; printf("%d", x); return 0; }'),
    ("compound_mul_var", 'int main() { int x = 5; x *= 3; printf("%d", x); return 0; }'),
    ("compound_div_var", 'int main() { int x = 6; x /= 3; printf("%d", x); return 0; }'),
    ("compound_mod_var", 'int main() { int x = 7; x %= 3; printf("%d", x); return 0; }'),
    ("compound_bitand_var", 'int main() { int x = 5; x &= 3; printf("%d", x); return 0; }'),
    ("compound_bitor_var", 'int main() { int x = 5; x |= 3; printf("%d", x); return 0; }'),
    ("compound_bitxor_var", 'int main() { int x = 5; x ^= 3; printf("%d", x); return 0; }'),
    ("compound_shl_var", 'int main() { int x = 1; x <<= 3; printf("%d", x); return 0; }'),
    ("compound_shr_var", 'int main() { int x = 8; x >>= 2; printf("%d", x); return 0; }'),
    ("ptr_assign_deref", 'int main() { int x = 5; int* p = &x; *p = 10; printf("%d", x); return 0; }'),
    ("ptr_compare_null", 'int main() { int* p = NULL; printf("%d", p == NULL); return 0; }'),
    ("ptr_compare_not_null", 'int main() { int x = 5; int* p = &x; printf("%d", p != NULL); return 0; }'),
    ("array_sum_loop", 'int main() { int a[3] = {1,2,3}; int s = 0; for (int i = 0; i < 3; i++) s += a[i]; printf("%d", s); return 0; }'),
    ("array_max", 'int main() { int a[3] = {3,1,2}; int m = a[0]; for (int i = 1; i < 3; i++) if (a[i] > m) m = a[i]; printf("%d", m); return 0; }'),
    ("swap_by_ptr", 'void swap(int* a, int* b) { int t = *a; *a = *b; *b = t; } int main() { int x = 1, y = 2; swap(&x, &y); printf("%d %d", x, y); return 0; }'),
    ("factorial", 'int fact(int n) { if (n <= 1) return 1; return n * fact(n-1); } int main() { printf("%d", fact(6)); return 0; }'),
    ("fibonacci", 'int fib(int n) { if (n <= 1) return n; return fib(n-1) + fib(n-2); } int main() { printf("%d", fib(10)); return 0; }'),
    ("sum_1_to_n", 'int main() { int n = 10, s = 0; for (int i = 1; i <= n; i++) s += i; printf("%d", s); return 0; }'),
    ("is_prime", 'int main() { int n = 17, is_p = 1; for (int i = 2; i * i <= n; i++) if (n % i == 0) is_p = 0; printf("%d", is_p); return 0; }'),
    ("gcd_euclid", 'int gcd(int a, int b) { while (b != 0) { int t = b; b = a % b; a = t; } return a; } int main() { printf("%d", gcd(48, 18)); return 0; }'),
    ("bubble_sort", 'int main() { int a[5] = {5,3,4,1,2}; for (int i = 0; i < 4; i++) for (int j = 0; j < 4-i; j++) if (a[j] > a[j+1]) { int t = a[j]; a[j] = a[j+1]; a[j+1] = t; } printf("%d", a[0]); return 0; }'),
    ("linear_search", 'int main() { int a[5] = {1,3,5,7,9}; int key = 7, found = -1; for (int i = 0; i < 5; i++) if (a[i] == key) { found = i; break; } printf("%d", found); return 0; }'),
    ("reverse_array", 'int main() { int a[5] = {1,2,3,4,5}; for (int i = 0; i < 2; i++) { int t = a[i]; a[i] = a[4-i]; a[4-i] = t; } printf("%d", a[0]); return 0; }'),
    ("sum_of_digits", 'int main() { int n = 123, s = 0; while (n > 0) { s += n % 10; n /= 10; } printf("%d", s); return 0; }'),
    ("power_of_2", 'int main() { int n = 16; printf("%d", (n & (n-1)) == 0); return 0; }'),
    ("count_bits", 'int main() { int n = 7, c = 0; while (n) { c++; n &= n-1; } printf("%d", c); return 0; }'),
    ("string_len_manual", 'int main() { char* s = "hello"; int len = 0; while (s[len]) len++; printf("%d", len); return 0; }'),
    ("memcpy_manual", 'int main() { int src[3] = {1,2,3}; int dst[3]; for (int i = 0; i < 3; i++) dst[i] = src[i]; printf("%d", dst[2]); return 0; }'),
    ("matrix_trace", 'int main() { int a[2][2] = {{1,2},{3,4}}; int trace = a[0][0] + a[1][1]; printf("%d", trace); return 0; }'),
    ("pointer_to_const", 'int main() { int x = 5; const int* p = &x; printf("%d", *p); return 0; }'),
    ("const_ptr_assign", 'int main() { int x = 5, y = 10; const int* p = &x; p = &y; printf("%d", *p); return 0; }'),
    # ---- 函数指针 (C子集新增) ----
    ("function_pointer_decl", 'int add(int a, int b) { return a+b; } int main() { int (*fp)(int,int) = add; printf("%d", fp(1,2)); return 0; }'),
    ("function_pointer_array", 'int f1() { return 1; } int f2() { return 2; } int main() { int (*fp[2])() = {f1,f2}; printf("%d %d", fp[0](), fp[1]()); return 0; }'),
    ("function_pointer_arg", 'int apply(int (*op)(int), int x) { return op(x); } int inc(int n) { return n+1; } int main() { printf("%d", apply(inc, 5)); return 0; }'),
    ("function_pointer_typedef", 'typedef int (*Op)(int, int); int add(int a, int b) { return a+b; } int main() { Op op = add; printf("%d", op(2,3)); return 0; }'),
    ("function_pointer_sizeof", 'int main() { printf("%d", sizeof(int (*)(int))); return 0; }'),
    ("function_pointer_multi_level", 'int add(int a) { return a+1; } int main() { int (*fp)(int) = add; int (**pp)(int) = &fp; printf("%d", (*pp)(5)); return 0; }'),
    ("function_pointer_return_ptr", 'int* greet(int x) { static int r = 0; r = x; return &r; } int main() { int* (*fp)(int) = greet; int* p = fp(42); printf("%d", *p); return 0; }'),
    ("function_pointer_array_direct", 'int mul(int a, int b) { return a*b; } int divi(int a, int b) { return a/b; } int main() { int (*ops[2])(int, int) = {mul, divi}; printf("%d %d", ops[0](3,4), ops[1](8,2)); return 0; }'),
    ("function_pointer_local_typedef", 'int add(int a, int b) { return a+b; } int main() { typedef int (*Op)(int, int); Op op = add; printf("%d", op(2,3)); return 0; }'),
    # ---- double (C子集新增) ----
    ("double_basic", 'int main() { double d = 3.1415926535; printf("%.10f", d); return 0; }'),
    ("double_arr", 'int main() { double arr[3] = {1.1, 2.2, 3.3}; printf("%.1f", arr[1]); return 0; }'),
    ("double_printf_lf", 'int main() { double d = 3.14; printf("%lf", d); return 0; }'),
    ("double_arith", 'int main() { double a = 1.5, b = 2.5; printf("%.1f", a + b); return 0; }'),
    ("double_mul", 'int main() { double a = 2.0, b = 3.0; printf("%.1f", a * b); return 0; }'),
    ("double_cast_int", 'int main() { double d = 3.9; printf("%d", (int)d); return 0; }'),
    ("double_cmp_eq", 'int main() { double a = 2.5, b = 2.5; printf("%d", a == b); return 0; }'),
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
