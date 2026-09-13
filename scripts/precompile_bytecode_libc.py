#!/usr/bin/env python3
"""预编译 runtime_libc 为嵌入产物。

开发流程：
1. 修改 native/runtime_libc/src/*.c
2. 运行 python scripts/precompile_bytecode_libc.py
3. git add native/crates/vitro_vm/src/bytecode_libc_data.json native/crates/vitro_runtime/src/bytecode_libc_index.rs
4. git commit

CI 检查：
    python scripts/precompile_bytecode_libc.py --check

`--check` 以 `source_digest`（源文件内容 SHA-256）判定产物是否与
`native/runtime_libc/{src,vitro}/` 同步，**不依赖文件 mtime**。

> 2026-09-11 修复：旧实现比较 mtime。CI 干净检出时 git 不保留 mtime，
> 且 `actions/checkout` 按路径顺序写文件（`native/crates/...` 先于
> `native/runtime_libc/...`），使源文件 mtime 普遍晚于产物 → 检查在 CI 中
> **必然误报失败**（本地因为改过源码才重新生成，反而看不出问题）。
"""

import argparse
import hashlib
import json
import math
import os
import subprocess
import sys

PROJECT_ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
NATIVE_DIR = os.path.join(PROJECT_ROOT, "native")
RUNTIME_LIBC_SRC_DIRS = [
    os.path.join(NATIVE_DIR, "runtime_libc", "src"),
    os.path.join(NATIVE_DIR, "runtime_libc", "vitro"),
]
VM_DIR = os.path.join(NATIVE_DIR, "crates", "vitro_vm", "src")
OUTPUT_JSON = os.path.join(VM_DIR, "bytecode_libc_data.json")
OUTPUT_RS = os.path.join(
    NATIVE_DIR, "crates", "vitro_runtime", "src", "bytecode_libc_index.rs"
)
LAYOUT_JSON = os.path.join(
    NATIVE_DIR, "crates", "vitro_cpp_frontend", "src", "builtin_layout_data.json"
)

# 摘要 schema 版本：源文件集合或摘要算法变化时递增，
# 使旧产物无需比较内容即可判定为过期。
DIGEST_SCHEMA = 1

# 参与摘要的源文件扩展名
SOURCE_EXTS = (".c", ".cpp", ".h")


def source_files(include_headers: bool = True) -> list:
    """返回 runtime_libc 下参与预编译的源文件（按路径排序）。

    `include_headers=True` 用于内容摘要（头文件变化同样需要重新生成）；
    传给 `vitro_cli export` 时应为 `False`（只传 .c/.cpp）。
    """
    exts = SOURCE_EXTS if include_headers else (".c", ".cpp")
    files = []
    for src_dir in RUNTIME_LIBC_SRC_DIRS:
        if not os.path.isdir(src_dir):
            continue
        for fname in os.listdir(src_dir):
            if fname.endswith(exts):
                files.append(os.path.join(src_dir, fname))
    return sorted(files)


def compute_source_digest() -> str:
    """计算 runtime_libc 源文件内容摘要（SHA-256）。

    只取决于源文件的相对路径与内容，**与 mtime 无关** ——
    因此在 CI 干净检出与本地增量开发下判定结果一致。
    """
    h = hashlib.sha256()
    h.update(f"schema={DIGEST_SCHEMA}\n".encode("utf-8"))
    for path in source_files():
        rel = os.path.relpath(path, PROJECT_ROOT).replace(os.sep, "/")
        h.update(rel.encode("utf-8"))
        h.update(b"\0")
        with open(path, "rb") as f:
            data = f.read()
        # 行尾规范化：worktree 的 CRLF/LF 取决于 core.autocrlf，
        # 同一份逻辑内容不应因为平台/检出设置不同而被判成"产物过期"。
        h.update(data.replace(b"\r\n", b"\n"))
        h.update(b"\0")
    return "sha256:" + h.hexdigest()


def find_vitro_cli() -> str:
    """查找 vitro_cli 可执行文件路径。"""
    candidates = [
        os.path.join(NATIVE_DIR, "target", "release", "vitro_cli.exe"),
        os.path.join(NATIVE_DIR, "target", "release", "vitro_cli"),
        os.path.join(NATIVE_DIR, "target", "debug", "vitro_cli.exe"),
        os.path.join(NATIVE_DIR, "target", "debug", "vitro_cli"),
    ]
    for c in candidates:
        if os.path.exists(c):
            return c
    return ""


def build_vitro_cli() -> str:
    """构建 vitro_cli，返回可执行文件路径。"""
    print("Building vitro_cli...")
    subprocess.run(
        ["cargo", "build", "--release", "--bin", "vitro_cli"],
        cwd=NATIVE_DIR,
        check=True,
    )
    exe = find_vitro_cli()
    if not exe:
        raise RuntimeError("vitro_cli build succeeded but executable not found")
    print(f"  -> {exe}")
    return exe


def precompile(exe: str) -> dict:
    """调用 vitro_cli export 预编译 runtime_libc。"""
    # Stage 2b: .cpp files now contain full C++ implementations,
    # and legacy .c container implementations are being removed.
    source_paths = source_files(include_headers=False)

    print(f"Precompiling {len(source_paths)} files:")
    for p in source_paths:
        print(f"  - {os.path.basename(p)}")

    cmd = [exe, "export"] + source_paths + ["--builtin-libc", "-o", OUTPUT_JSON]
    subprocess.run(cmd, check=True)

    with open(OUTPUT_JSON, "r", encoding="utf-8") as f:
        data = json.load(f)

    # 将 code 中的 Call/CallPtr operand 从原始索引重定位为固定索引，
    # 避免 setup_vm 同时注册原始索引和固定索引时发生冲突。
    raw_func_index = data["func_index"]
    base_index = 1000
    sorted_raw = sorted(raw_func_index.items(), key=lambda x: x[1])
    raw_to_fixed = {raw: base_index + i for i, (_, raw) in enumerate(sorted_raw)}

    for inst in data["code"]:
        if inst.get("op") in ("Call", "CallPtr"):
            raw = inst.get("operand", 0)
            inst["operand"] = raw_to_fixed.get(raw, raw)

    data["func_index"] = {name: raw_to_fixed[raw] for name, raw in raw_func_index.items()}

    # 记录源文件内容摘要：供 --check 在 CI 干净检出下做稳定判定（与 mtime 无关）
    data["source_digest"] = compute_source_digest()

    validate_precompiled(data)

    print(f"  code_len: {data['code_len']}")
    print(f"  func_count: {len(data['func_index'])}")
    print(f"  globals_size: {data['globals_size']}")
    return data


def validate_precompiled(data: dict) -> None:
    """校验预编译产物符合 Stage 2b 要求：

    1. runtime_libc/vitro/ 下不应存在旧 .c 实现。
    2. builtin_layout 中注册的方法必须在产物中可用。
    3. 不应残留旧 C 风格函数名（如 vitro_vec_init_int）。
    """
    vitro_dir = os.path.join(NATIVE_DIR, "runtime_libc", "vitro")
    legacy_c_files = [
        f for f in os.listdir(vitro_dir) if f.endswith(".c")
    ] if os.path.isdir(vitro_dir) else []
    if legacy_c_files:
        raise RuntimeError(
            f"发现遗留 C 容器实现: {legacy_c_files}. "
            "Stage 2b 要求 runtime_libc/vitro/ 只保留 .cpp 实现。"
        )

    if not os.path.exists(LAYOUT_JSON):
        raise RuntimeError(f"布局文件不存在: {LAYOUT_JSON}")

    with open(LAYOUT_JSON, "r", encoding="utf-8") as f:
        layout = json.load(f)

    func_index = data["func_index"]
    missing = []
    for vitro_name, cls in layout["classes"].items():
        method_map = layout.get("method_map", {}).get(vitro_name, {})
        for method, mangled in method_map.items():
            if mangled not in func_index:
                missing.append(f"{vitro_name}.{method} -> {mangled}")
    if missing:
        raise RuntimeError(
            "以下内置容器方法未在预编译产物中找到:\n  " + "\n  ".join(missing)
        )

    # 旧 C 风格函数名黑名单（Stage 2b 之前的命名）
    old_prefixes = (
        "vitro_vec_init_",
        "vitro_vec_push_",
        "vitro_vec_pop_",
        "vitro_vec_get_",
        "vitro_vec_size_",
        "vitro_vec_destroy_",
        "vitro_vec_clear_",
        "vitro_list_init_",
        "vitro_list_push_",
        "vitro_list_pop_",
        "vitro_list_get_",
        "vitro_list_size_",
        "vitro_list_destroy_",
        "vitro_list_clear_",
        "vitro_string_init",
        "vitro_string_push_",
        "vitro_string_pop_",
        "vitro_string_get_",
        "vitro_string_size",
        "vitro_string_destroy",
        "vitro_string_clear",
        "vitro_string_c_str",
    )
    stale = [name for name in func_index if name.startswith(old_prefixes)]
    if stale:
        raise RuntimeError(
            "预编译产物中残留旧 C 风格函数名:\n  " + "\n  ".join(stale)
        )

    print("  validation: ok")


def generate_index_rs(data: dict) -> str:
    """生成 bytecode_libc_index.rs。"""
    func_index = data["func_index"]
    base_index = min(func_index.values()) if func_index else 1000
    globals_reserved = max(1024, math.ceil(data["globals_size"] / 1024) * 1024)
    code_len = data["code_len"]

    # 预编译阶段已将 func_index 重定位为固定索引，直接复用。
    # 按固定索引排序
    sorted_funcs = sorted(func_index.items(), key=lambda x: x[1])

    lines = [
        "// AUTO-GENERATED by scripts/precompile_bytecode_libc.py",
        "// DO NOT EDIT MANUALLY",
        "//",
        "// To regenerate:",
        "//   python scripts/precompile_bytecode_libc.py",
        "",
        f"pub const BYTECODE_LIBC_CODE_LEN: usize = {code_len};",
        f"pub const BYTECODE_LIBC_BASE_INDEX: i32 = {base_index};",
        f"pub const BYTECODE_LIBC_GLOBALS_RESERVED: u32 = {globals_reserved};",
        f"pub const BYTECODE_LIBC_FUNC_COUNT: usize = {len(func_index)};",
        "",
        "/// Bytecode Libc 中所有可用的函数名（供索引查询使用）。",
        "/// 注意：并非所有函数都默认走 Bytecode 路径，",
        "/// 实际路径由 `host_func_id::BYTECODE_LIBC_PURE_FUNCS` 控制。",
        "pub const BYTECODE_LIBC_ALL_FUNCS: &[&str] = &[",
    ]
    for name, _ in sorted_funcs:
        lines.append(f'    "{name}",')
    lines.append("];")
    lines.append("")
    lines.append("/// 将函数名解析为 Bytecode Libc 固定索引。")
    lines.append("pub fn bytecode_libc_index(name: &str) -> Option<i32> {")
    lines.append("    match name {")
    for name, idx in sorted_funcs:
        lines.append(f'        "{name}" => Some({idx}),')
    lines.append("        _ => None,")
    lines.append("    }")
    lines.append("}")
    lines.append("")
    lines.append("/// 判断函数是否在 Bytecode Libc 预编译产物中存在。")
    lines.append("pub fn is_bytecode_libc(name: &str) -> bool {")
    lines.append("    bytecode_libc_index(name).is_some()")
    lines.append("}")
    lines.append("")

    return "\n".join(lines) + "\n"


def write_outputs(data: dict) -> None:
    """写入 JSON 和 Rust 常量文件。"""
    # sort_keys：产物由 Rust 侧的 HashMap 派生（func_index / func_table 等），
    # 其序列化键顺序随进程随机种子变化，会让同一份语义内容的 JSON 文本每次不同
    # （提交到版本库后表现为巨大的无意义 diff）。按键排序保证产物可重现
    # （2026-09-11 定位）。
    with open(OUTPUT_JSON, "w", encoding="utf-8") as f:
        json.dump(data, f, ensure_ascii=False, indent=2, sort_keys=True)
    print(f"Written: {OUTPUT_JSON}")

    rs_content = generate_index_rs(data)
    with open(OUTPUT_RS, "w", encoding="utf-8") as f:
        f.write(rs_content)
    print(f"Written: {OUTPUT_RS}")


def check_up_to_date() -> bool:
    """检查预编译产物是否与 runtime_libc 源码同步。

    以 `source_digest`（源文件内容摘要）判定，**不使用文件 mtime**：
    CI 干净检出时 git 不保留 mtime，且按路径顺序写文件使源文件看起来
    比产物更新，mtime 判定在 CI 中必然误报（2026-09-11 修复）。
    """
    for path in (OUTPUT_JSON, OUTPUT_RS):
        if not os.path.exists(path):
            print(f"  missing artifact: {os.path.relpath(path, PROJECT_ROOT)}")
            return False

    try:
        with open(OUTPUT_JSON, "r", encoding="utf-8") as f:
            data = json.load(f)
    except (OSError, ValueError) as e:
        print(f"  unreadable artifact: {e}")
        return False

    recorded = data.get("source_digest")
    if not recorded:
        print(
            "  artifact has no source_digest field "
            "(generated by an older script) -> regenerate once"
        )
        return False

    current = compute_source_digest()
    if recorded != current:
        print(f"  recorded digest: {recorded}")
        print(f"  current  digest: {current}")
        print(f"  source files: {len(source_files())} under native/runtime_libc/")
        return False

    return True


def main() -> int:
    parser = argparse.ArgumentParser(description="Precompile runtime_libc Bytecode")
    parser.add_argument(
        "--check",
        action="store_true",
        help="Check if precompiled artifacts are up-to-date (CI mode)",
    )
    args = parser.parse_args()

    if args.check:
        if check_up_to_date():
            print("Precompiled artifacts are up-to-date.")
            return 0
        else:
            print(
                "ERROR: Precompiled artifacts are out-of-date. "
                "Run: python scripts/precompile_bytecode_libc.py",
                file=sys.stderr,
            )
            return 1

    # 清理旧产物，避免残留索引与旧函数名
    for f in (OUTPUT_JSON, OUTPUT_RS):
        if os.path.exists(f):
            os.remove(f)
            print(f"  removed: {f}")

    exe = find_vitro_cli()
    if not exe:
        exe = build_vitro_cli()

    data = precompile(exe)
    write_outputs(data)
    print("Done.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
