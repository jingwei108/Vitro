#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
C++ Shadow Verification 框架
对比 Clang++ 和 Cide 对同一份 C++ 代码的编译/运行结果。

用法:
    python scripts/shadow_verify_cpp.py

**已被 Go 版取代（2026-09-12，D5 语言迁移第一站）**：`go run ./scripts/shadow_verify_cpp`
与本版双轨对账一致（94 用例 / 判定与 stdout 内容逐项一致）后接管 CI。
本文件保留为**双轨对照基准**——Go 版判定异常时用它复现归因；日常运行请用 Go 版。
"""

import os
import sys
import io
import subprocess
import tempfile
import time
import json
import ctypes
from pathlib import Path
from dataclasses import dataclass, asdict
from typing import Optional, List, Dict

sys.stdout = io.TextIOWrapper(sys.stdout.buffer, encoding='utf-8')
sys.stderr = io.TextIOWrapper(sys.stderr.buffer, encoding='utf-8')

SCRIPT_DIR = Path(__file__).parent.resolve()
PROJECT_ROOT = SCRIPT_DIR.parent
NATIVE_DIR = PROJECT_ROOT / "native"
CIDE_CLI = NATIVE_DIR / "target/release/cide_cli.exe"
DLL_PATH = NATIVE_DIR / "target/release/cide_native.dll"
CLANG_PATH = "clang++"

# E-P1-5：输出通道的结构化读取（与 C 版 shadow 驱动共用同一份入口，禁止再自行清洗）。
_SHADOW_HELPERS = NATIVE_DIR / "tests" / "shadow_verification"
if str(_SHADOW_HELPERS) not in sys.path:
    sys.path.insert(0, str(_SHADOW_HELPERS))
from cide_output import ensure_abi, read_program_stdout  # noqa: E402


@dataclass
class RunResult:
    compiler: str
    compile_success: bool
    compile_error: str
    run_success: bool
    run_error: str
    stdout: str
    stderr: str
    exit_code: int
    duration_ms: float


@dataclass
class ShadowCase:
    name: str
    source: str
    category: str


@dataclass
class ShadowDiff:
    case_name: str
    expected_category: str
    clang_result: RunResult
    cide_result: RunResult
    diff_type: str


CPP_CASES: List[ShadowCase] = [
    ShadowCase("cpp_class_field", r'''
#include <stdio.h>
class Point {
public:
    int x;
    Point() { x = 0; }
};
int main() {
    Point p;
    p.x = 42;
    printf("%d\n", p.x);
    return 0;
}
''', "baseline"),
    ShadowCase("cpp_new_delete", r'''
#include <stdio.h>
class Box {
public:
    int v;
    Box() { v = 0; }
};
int main() {
    Box* b = new Box();
    b->v = 7;
    printf("%d\n", b->v);
    delete b;
    return 0;
}
''', "baseline"),
    ShadowCase("cpp_virtual_call", r'''
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
''', "baseline"),
    ShadowCase("cpp_template_class", r'''
#include <stdio.h>
template<class T>
class Box {
public:
    T v;
    Box() { v = 0; }
};
int main() {
    Box<int> b;
    b.v = 99;
    printf("%d\n", b.v);
    return 0;
}
''', "baseline"),
    ShadowCase("cpp_lambda_capture", r'''
#include <stdio.h>
int main() {
    int x = 5;
    auto f = [x](int y) { return x + y; };
    printf("%d\n", f(3));
    return 0;
}
''', "baseline"),
    ShadowCase("cpp_reference_param", r'''
#include <stdio.h>
void inc(int& x) { x = x + 1; }
int main() {
    int a = 5;
    inc(a);
    printf("%d\n", a);
    return 0;
}
''', "baseline"),
    ShadowCase("cpp_range_for", r'''
#include <stdio.h>
int main() {
    int arr[] = {1, 2, 3};
    int sum = 0;
    for (int x : arr) sum = sum + x;
    printf("%d\n", sum);
    return 0;
}
''', "baseline"),
    ShadowCase("cpp_raii_dtor", r'''
#include <stdio.h>
int g = 0;
class A {
public:
    int id;
    A() { id = 0; }
    void init(int i) { id = i; }
    ~A() { g = g * 10 + id; }
};
void foo() {
    A a;
    a.init(1);
}
int main() {
    foo();
    printf("%d\n", g);
    return 0;
}
''', "baseline"),
    ShadowCase("cpp_new_array", r'''
#include <stdio.h>
int g = 0;
class A {
public:
    A() { g++; }
    ~A() { g--; }
};
int main() {
    A* arr = new A[3];
    printf("%d\n", g);
    delete[] arr;
    printf("%d\n", g);
    return 0;
}
''', "baseline"),
    ShadowCase("cpp_nested_class_new", r'''
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
    void push(T x) {
        Node* n = new Node;
        n->data = x;
        n->next = head;
        head = n;
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
    l.push(10);
    l.push(20);
    printf("%d\n", l.get(0));
    printf("%d\n", l.get(1));
    return 0;
}
''', "baseline"),
    ShadowCase("cpp_ctor_overload", r'''
#include <stdio.h>
class Box {
public:
    int x;
    Box() { x = 0; }
    Box(int v) { x = v; }
};
int main() {
    Box* a = new Box();
    Box* b = new Box(42);
    printf("%d %d\n", a->x, b->x);
    delete a;
    delete b;
    return 0;
}
''', "gap"),
    ShadowCase("cpp_rvalue_ref", r'''
#include <stdio.h>
int foo() { return 42; }
int main() {
    int&& r = foo();
    printf("%d\n", r);
    return 0;
}
''', "baseline"),
    ShadowCase("cpp_const_ref_rvalue", r'''
#include <stdio.h>
int main() {
    const int& r = 5;
    printf("%d\n", r);
    return 0;
}
''', "baseline"),
    ShadowCase("cpp_auto_ref", r'''
#include <stdio.h>
int main() {
    int x = 10;
    auto& r = x;
    r = r + 1;
    printf("%d\n", x);
    return 0;
}
''', "baseline"),
    ShadowCase("cpp_lambda_multi_capture", r'''
#include <stdio.h>
int main() {
    int a = 1, b = 2;
    auto f = [a, &b](int x) { return x + a + b; };
    b = 5;
    printf("%d\n", f(10));
    return 0;
}
''', "baseline"),
    ShadowCase("cpp_lambda_ref_capture", r'''
#include <stdio.h>
int main() {
    int x = 5;
    auto f = [&x]() { x = x + 1; };
    f();
    printf("%d\n", x);
    return 0;
}
''', "baseline"),
    ShadowCase("cpp_member_out_of_line", r'''
#include <stdio.h>
class Bar {
public:
    int x;
    void set(int v);
};
void Bar::set(int v) { x = v; }
int main() {
    Bar b;
    b.set(7);
    printf("%d\n", b.x);
    return 0;
}
''', "gap"),
    ShadowCase("cpp_auto_new_int", r'''
#include <stdio.h>
int main() {
    auto p = new int(99);
    printf("%d\n", *p);
    delete p;
    return 0;
}
''', "baseline"),
    ShadowCase("cpp_range_for_ref_modify", r'''
#include <stdio.h>
int main() {
    int arr[] = {1, 2, 3};
    for (auto& x : arr) x = x * 2;
    printf("%d %d %d\n", arr[0], arr[1], arr[2]);
    return 0;
}
''', "baseline"),
    ShadowCase("cpp_template_struct", r'''
#include <stdio.h>
template<class T> struct Pair { T a, b; };
int main() {
    Pair<int> p;
    p.a = 1;
    p.b = 2;
    printf("%d %d\n", p.a, p.b);
    return 0;
}
''', "gap"),
    ShadowCase("cpp_struct_tag_alias", r'''
#include <stdio.h>
struct Node { int x; };
int main() {
    Node n;
    n.x = 42;
    printf("%d\n", n.x);
    return 0;
}
''', "baseline"),
    ShadowCase("cpp_new_int_array", r'''
#include <stdio.h>
int main() {
    int* p = new int[3];
    p[0] = 1;
    p[1] = 2;
    p[2] = 3;
    printf("%d %d %d\n", p[0], p[1], p[2]);
    delete[] p;
    return 0;
}
''', "baseline"),
]


class _WorkDir:
    """自管生命周期的 Clang 工作目录（替代 `tempfile.TemporaryDirectory`）。

    受限（沙箱）环境下 `tempfile.TemporaryDirectory` 的 `mkdtemp` 会生成不可写目录，
    清理阶段抛 `PermissionError: [WinError 5]`（实测 2026-09-11）——Python 版 C shadow
    驱动（`shadow_verify.py`）此前已因同一问题改为自管目录，此处对齐。
    """

    def __init__(self, root: Path):
        self.root = root

    def __enter__(self) -> str:
        self.root.mkdir(parents=True, exist_ok=True)
        # 清掉上一次运行的产物，避免读到陈旧可执行文件
        for stale in self.root.glob("test*"):
            try:
                stale.unlink()
            except OSError:
                pass
        return str(self.root)

    def __exit__(self, *_exc) -> bool:
        return False


def run_with_clang(source: str) -> RunResult:
    """编译失败时重试（CI runner 上 clang 偶发瞬时失败——实测 2026-09-12：
    同一用例首跑 35s 后 CLANG_COMPILE_FAIL、复跑 0.15s 通过，属环境抖动）。
    最多 3 次尝试；真正的语法错误 3 次同样失败，代价可忽略（小文件）。"""
    result = None
    for attempt in range(3):
        result = _run_with_clang_once(source)
        if result.compile_success:
            return result
        time.sleep(0.5 * (attempt + 1))
    return result


def _run_with_clang_once(source: str) -> RunResult:
    start = time.time()
    with _WorkDir(PROJECT_ROOT / ".shadow_cpp_tmp") as tmpdir:
        cpp_file = Path(tmpdir) / "test.cpp"
        exe_file = Path(tmpdir) / "test.exe" if sys.platform == "win32" else Path(tmpdir) / "test"
        cpp_file.write_text(source, encoding="utf-8")

        compile_cmd = [CLANG_PATH, str(cpp_file), "-o", str(exe_file), "-std=c++14"]
        if sys.platform != "win32":
            compile_cmd.append("-lm")
        try:
            compile_proc = subprocess.run(
                compile_cmd, capture_output=True, text=True, encoding="utf-8", errors="replace", timeout=30
            )
        except Exception as e:
            return RunResult(
                compiler="clang++", compile_success=False, compile_error=str(e),
                run_success=False, run_error="", stdout="", stderr="",
                exit_code=-1, duration_ms=(time.time() - start) * 1000,
            )

        if compile_proc.returncode != 0:
            return RunResult(
                compiler="clang++", compile_success=False, compile_error=compile_proc.stderr,
                run_success=False, run_error="", stdout="", stderr=compile_proc.stderr,
                exit_code=compile_proc.returncode, duration_ms=(time.time() - start) * 1000,
            )

        try:
            run_proc = subprocess.run(
                [str(exe_file)], capture_output=True, text=True, encoding="utf-8", errors="replace", timeout=5
            )
            return RunResult(
                compiler="clang++", compile_success=True, compile_error="",
                run_success=run_proc.returncode == 0,
                run_error=run_proc.stderr if run_proc.returncode != 0 else "",
                stdout=run_proc.stdout, stderr=run_proc.stderr,
                exit_code=run_proc.returncode, duration_ms=(time.time() - start) * 1000,
            )
        except Exception as e:
            return RunResult(
                compiler="clang++", compile_success=True, compile_error="",
                run_success=False, run_error=str(e), stdout="", stderr="",
                exit_code=-1, duration_ms=(time.time() - start) * 1000,
            )


def run_with_cide(source: str) -> RunResult:
    """通过 C API 调用 Cide 编译并运行 C++ 代码（使用 main.cpp 自动启用 C++ 模式）。"""
    import ctypes

    start = time.time()
    dll = ctypes.CDLL(str(DLL_PATH))

    # C API 函数签名
    dll.cide_session_create.restype = ctypes.c_void_p
    dll.cide_session_destroy.argtypes = [ctypes.c_void_p]
    dll.cide_compile_unit.argtypes = [ctypes.c_void_p, ctypes.c_char_p, ctypes.c_char_p]
    dll.cide_compile_unit.restype = ctypes.c_int
    dll.cide_compile_all.argtypes = [ctypes.c_void_p]
    dll.cide_compile_all.restype = ctypes.c_int
    dll.cide_run.argtypes = [ctypes.c_void_p]
    dll.cide_run.restype = ctypes.c_int
    dll.cide_get_compile_errors.restype = ctypes.c_char_p
    dll.cide_get_compile_errors.argtypes = [ctypes.c_void_p]
    dll.cide_get_runtime_error.restype = ctypes.c_char_p
    dll.cide_get_runtime_error.argtypes = [ctypes.c_void_p]
    dll.cide_get_output_length.restype = ctypes.c_int
    dll.cide_get_output_length.argtypes = [ctypes.c_void_p]
    dll.cide_get_output.argtypes = [ctypes.c_void_p, ctypes.c_char_p, ctypes.c_int]
    # E-P1-5：结构化输出通道（纯程序 stdout / 引擎附注）。
    dll.cide_get_program_output_length.restype = ctypes.c_int
    dll.cide_get_program_output_length.argtypes = [ctypes.c_void_p]
    dll.cide_get_program_output.argtypes = [ctypes.c_void_p, ctypes.c_char_p, ctypes.c_int]
    dll.cide_get_engine_notes_length.restype = ctypes.c_int
    dll.cide_get_engine_notes_length.argtypes = [ctypes.c_void_p]
    dll.cide_get_engine_notes.argtypes = [ctypes.c_void_p, ctypes.c_char_p, ctypes.c_int]
    ensure_abi(dll)

    session = dll.cide_session_create()
    if not session:
        return RunResult(
            compiler="cide", compile_success=False, compile_error="session create failed",
            run_success=False, run_error="", stdout="", stderr="", exit_code=-1,
            duration_ms=(time.time() - start) * 1000,
        )

    try:
        dll.cide_compile_unit(session, b"main.cpp", source.encode("utf-8"))
        compile_ret = dll.cide_compile_all(session)
        if compile_ret != 0:
            err_ptr = dll.cide_get_compile_errors(session)
            err_msg = err_ptr.decode("utf-8", errors="replace") if err_ptr else "Unknown compile error"
            return RunResult(
                compiler="cide", compile_success=False, compile_error=err_msg,
                run_success=False, run_error="", stdout="", stderr=err_msg,
                exit_code=compile_ret, duration_ms=(time.time() - start) * 1000,
            )

        run_ret = dll.cide_run(session)

        # E-P1-5：直接读纯程序 stdout 通道（引擎附注走 note 通道）。此前的两条正则
        # 全局替换已废除——它会把程序自己打印的同类文本整段删掉（假阳性 output_gap）。
        stdout_str = read_program_stdout(dll, session)

        err_ptr = dll.cide_get_runtime_error(session)
        runtime_err = err_ptr.decode("utf-8", errors="replace") if err_ptr else ""

        return RunResult(
            compiler="cide", compile_success=True, compile_error="",
            run_success=run_ret == 0 and not runtime_err,
            run_error=runtime_err, stdout=stdout_str, stderr=runtime_err,
            exit_code=run_ret, duration_ms=(time.time() - start) * 1000,
        )
    finally:
        dll.cide_session_destroy(session)


def compare_results(clang: RunResult, cide: RunResult) -> str:
    if not clang.compile_success:
        return "clang_compile_fail"
    if not cide.compile_success:
        return "compile_gap"
    if not clang.run_success or not cide.run_success:
        if clang.run_success != cide.run_success:
            return "runtime_gap"
    clang_out = clang.stdout.strip().replace("\r\n", "\n")
    cide_out = cide.stdout.strip().replace("\r\n", "\n")
    if clang_out != cide_out:
        return "output_gap"
    return "match"


def load_directory_cases() -> List[ShadowCase]:
    """加载 native/tests/cases/cpp/ 目录下的 .cpp 文件作为影子验证用例。"""
    cases_dir = NATIVE_DIR / "tests/cases/cpp"
    cases = []
    if cases_dir.exists():
        for cpp_file in sorted(cases_dir.glob("*.cpp")):
            source = cpp_file.read_text(encoding="utf-8")
            category = "e2e_regression"
            first_line = source.lstrip().splitlines()[0] if source.lstrip() else ""
            if first_line.startswith("// category:"):
                category = first_line.split(":", 1)[1].strip()
            cases.append(ShadowCase(
                name=cpp_file.stem,
                source=source,
                category=category,
            ))
    return cases


def main():
    # R4/G10：目录 `cases/cpp/*.cpp` 是维护真相——与内嵌 CPP_CASES 同名时以
    # 目录版为准（曾发生内嵌副本内容漂移导致同一用例双跑、结果互相矛盾）
    dir_cases = load_directory_cases()
    dir_names = {c.name for c in dir_cases}
    all_cases = [c for c in CPP_CASES if c.name not in dir_names] + dir_cases
    diffs: List[ShadowDiff] = []
    for case in all_cases:
        print(f"Running {case.name} ...", flush=True)
        clang_res = run_with_clang(case.source)
        cide_res = run_with_cide(case.source)
        diff_type = compare_results(clang_res, cide_res)
        diffs.append(ShadowDiff(
            case_name=case.name,
            expected_category=case.category,
            clang_result=clang_res,
            cide_result=cide_res,
            diff_type=diff_type,
        ))
        status = "✅ MATCH" if diff_type == "match" else f"❌ {diff_type.upper()}"
        print(f"  {status}")

    total = len(diffs)
    match = sum(1 for d in diffs if d.diff_type == "match")
    compile_gap = sum(1 for d in diffs if d.diff_type == "compile_gap")
    runtime_gap = sum(1 for d in diffs if d.diff_type == "runtime_gap")
    output_gap = sum(1 for d in diffs if d.diff_type == "output_gap")
    clang_fail = sum(1 for d in diffs if d.diff_type == "clang_compile_fail")

    # 标记为 "gap" 的用例是已知的 Cide 限制/缺失特性，视为预期差异
    expected_gaps = [d for d in diffs if d.expected_category == "gap" and d.diff_type != "match"]
    unexpected_gaps = [d for d in diffs if d.expected_category != "gap" and d.diff_type != "match"]

    print("\n" + "=" * 60)
    print("C++ Shadow Verification 报告")
    print("=" * 60)
    print(f"总用例: {total}")
    print(f"  ✅ 一致: {match}")
    print(f"  ❌ 编译差异: {compile_gap}")
    print(f"  ❌ 运行时差异: {runtime_gap}")
    print(f"  ❌ 输出差异: {output_gap}")
    print(f"  ⚠️  Clang++ 编译失败: {clang_fail}")
    print(f"  📌 预期差异 (gap): {len(expected_gaps)}")
    print(f"  🚨 非预期差异: {len(unexpected_gaps)}")

    if expected_gaps:
        print("\n预期差异用例（已记录的 Cide 限制）：")
        for d in expected_gaps:
            print(f"  - {d.case_name}: {d.diff_type}")

    if unexpected_gaps:
        print("\n非预期差异用例（需要调查）：")
        for d in unexpected_gaps:
            print(f"  - {d.case_name}: {d.diff_type}")

    report_path = PROJECT_ROOT / "native/tests/shadow_verification/reports/cpp_shadow_report.json"
    report_path.parent.mkdir(parents=True, exist_ok=True)
    with open(report_path, "w", encoding="utf-8") as f:
        json.dump([asdict(d) for d in diffs], f, ensure_ascii=False, indent=2)
    print(f"\n报告已保存: {report_path}")

    # 只有非预期差异才导致 CI 失败
    return 0 if not unexpected_gaps else 1


if __name__ == "__main__":
    sys.exit(main())
