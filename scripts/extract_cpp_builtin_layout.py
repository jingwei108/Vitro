#!/usr/bin/env python3
"""
提取 Vitro 内置 C++ 容器类定义，生成 builtin_layout_data.json。
输入: native/runtime_libc/vitro/*.cpp
输出: native/src/compiler/cpp_frontend/builtin_layout_data.json

约定:
- 容器类使用标准模板写法，如 `template <class T> class vitro_vec { ... };`
- 文件名到 (base_vitro_name, base_cpp_name, type_args) 的映射由 FILE_RULES 维护
- method_map 输出 mangled 方法名: {vitro_name}__{method}
- sort_int.cpp 是自由函数模板，不参与布局提取
"""

import json
import re
import sys
from datetime import datetime, timezone
from pathlib import Path

# 相对于项目根目录
INPUT_DIR = Path("native/runtime_libc/vitro")
OUTPUT_PATH = Path("native/src/compiler/cpp_frontend/builtin_layout_data.json")

# 类型到 size 的映射（与 Vitro VM 32/64 位模型对齐，当前用 4 字节指针）
TYPE_SIZE = {
    "int": 4,
    "float": 4,
    "char": 1,
    "double": 8,
    "void": 1,      # void 本身不占空间，但 void* 占 4
    "void*": 4,
    "int*": 4,
    "float*": 4,
    "char*": 4,
    "double*": 4,
}

# 文件名 -> (base_vitro_name, base_cpp_name, [type_arg, ...])
# sort_int.cpp 是自由函数模板，不在这里注册
FILE_RULES = {
    "vector.cpp": ("vitro_vec", "vector<T>", ["int", "float", "char"]),
    "list.cpp": ("vitro_list", "list<T>", ["int"]),
    "string.cpp": ("vitro_string", "string", ["char"]),
}


def strip_comments(text: str) -> str:
    """移除 C/C++ 风格的 // 和 /* */ 注释。"""
    text = re.sub(r'//.*?$', '', text, flags=re.MULTILINE)
    text = re.sub(r'/\*.*?\*/', '', text, flags=re.DOTALL)
    return text


def parse_type_size(ty: str) -> int:
    """根据类型字符串计算简单 size。"""
    ty = ty.strip()
    if ty in TYPE_SIZE:
        return TYPE_SIZE[ty]
    if ty.endswith("*"):
        return 4
    return 4


def parse_type_str(s: str) -> str:
    """标准化类型字符串，目前直接返回原始字符串。"""
    return s.strip()


def mangle_template_name(base_vitro: str, type_arg: str) -> str:
    """与编译器 TypeChecker::mangle_template_name 保持一致的内置容器短名规则。"""
    special = {
        ("vitro_vec", "int"): "vitro_vec_int",
        ("vitro_vec", "float"): "vitro_vec_float",
        ("vitro_vec", "char"): "vitro_vec_char",
        ("vitro_list", "int"): "vitro_list_int",
        ("vitro_string", "char"): "vitro_string",
    }
    if (base_vitro, type_arg) in special:
        return special[(base_vitro, type_arg)]
    return f"{base_vitro}__{type_arg}"


def cpp_type_name(base_cpp: str, type_arg: str) -> str:
    """根据 base_cpp 模板形式和类型参数生成用户可见的 C++ 类型名。"""
    if "<T>" in base_cpp:
        return base_cpp.replace("<T>", f"<{type_arg}>")
    return base_cpp


def replace_type_param(text: str, param: str, replacement: str) -> str:
    """将类体中的模板参数 T 替换为实际类型（按单词边界匹配）。"""
    return re.sub(rf'\b{re.escape(param)}\b', replacement, text)


def extract_class_definition(text: str, class_name: str) -> str | None:
    """从文本中提取模板类/普通类定义体（大括号内内容）。"""
    # 支持 "template <class T> class Foo {" 或 "class Foo {"
    pattern = rf'(?:template\s*<[^>]+>\s+)?\bclass\s+{re.escape(class_name)}\s*\{{'
    m = re.search(pattern, text)
    if not m:
        return None

    start = m.end() - 1  # 指向 '{'
    brace_count = 0
    i = start
    while i < len(text):
        if text[i] == '{':
            brace_count += 1
        elif text[i] == '}':
            brace_count -= 1
            if brace_count == 0:
                return text[start + 1:i]
        i += 1
    return None


def split_top_level(body: str) -> list[str]:
    """
    将类体按顶层分号或访问说明符分割。
    返回每个顶层语句的字符串（不包含末尾分号）。
    方法体内部的大括号会被整体保留在一个 segment 中。
    """
    segments = []
    current = []
    brace_depth = 0
    i = 0
    while i < len(body):
        ch = body[i]
        if ch == '{':
            brace_depth += 1
            current.append(ch)
        elif ch == '}':
            brace_depth -= 1
            current.append(ch)
            if brace_depth == 0:
                # inline 方法定义以 '}' 结尾，没有顶层分号
                seg = ''.join(current).strip()
                if seg:
                    segments.append(seg)
                current = []
                i += 1
                # 跳过 '}' 后面的空白
                while i < len(body) and body[i] in ' \t\n\r':
                    i += 1
                continue
        elif ch == ';' and brace_depth == 0:
            seg = ''.join(current).strip()
            if seg:
                segments.append(seg)
            current = []
        elif brace_depth == 0:
            # 检测访问说明符，遇到时先结束当前 segment
            rest = body[i:]
            if rest.startswith('public:') or rest.startswith('private:') or rest.startswith('protected:'):
                seg = ''.join(current).strip()
                if seg:
                    segments.append(seg)
                current = []
                # 把 access specifier 单独作为一段
                end = i + rest.index(':') + 1
                segments.append(body[i:end].strip())
                i = end
                continue
            current.append(ch)
        else:
            current.append(ch)
        i += 1
    # 末尾无分号的 segment（理论上不应该有）
    seg = ''.join(current).strip()
    if seg:
        segments.append(seg)
    return segments


def parse_fields_and_methods(body: str, class_name: str, base_class_name: str) -> tuple[list[dict], list[dict]]:
    """
    解析类体中的字段和方法声明。
    假设方法在类内 inline 实现。

    class_name 用于输出 method_map 中的 mangled 名；
    base_class_name 是源码中的模板类名（如 vitro_vec），用于识别构造函数。
    """
    fields = []
    methods = []
    access = "private"

    segments = split_top_level(body)
    for seg in segments:
        # 访问说明符（可能带冒号）
        seg_no_colon = seg.rstrip(":")
        if seg_no_colon in ("public", "private", "protected"):
            access = seg_no_colon
            continue

        # 字段声明: type name;（支持模板指针类型如 vitro_list_node<int>*）
        field_m = re.match(r'^([^(]+?)\s+(\w+)\s*$', seg)
        if field_m and '(' not in seg and ')' not in seg:
            ty = field_m.group(1).strip()
            name = field_m.group(2).strip()
            fields.append({"name": name, "type": parse_type_str(ty)})
            continue

        if access != "public":
            continue

        # 方法需要包含 '('
        if '(' not in seg:
            continue

        # 析构函数: ~ClassName() { ... }
        dm = re.match(r'~\s*(\w+)\s*\(\s*\)', seg)
        if dm:
            methods.append({
                "name": "destroy",
                "params": [],
                "ret": "void",
                "is_virtual": False,
            })
            continue

        # 普通方法: ret name(params) { ... }
        # 支持返回类型如: void, int, float*, vitro_list_node_int*
        mm = re.match(r'((?:[\w:]+(?:\s*\*)?\s+)+)(\w+)\s*\(([^)]*)\)', seg)
        if mm:
            ret_raw = mm.group(1).strip()
            name = mm.group(2).strip()
            params_raw = mm.group(3).strip()

            if name in ("return", "if", "while", "for"):
                continue

            # 跳过构造函数（与模板类同名且无返回类型）
            if not ret_raw and name == base_class_name:
                continue

            ret = ret_raw if ret_raw else "void"

            params = []
            if params_raw:
                for p in params_raw.split(','):
                    p = p.strip()
                    if not p:
                        continue
                    pm = re.match(r'((?:[\w:]+(?:\s*\*)?\s+)+)(?:\w+)', p)
                    if pm:
                        pty = pm.group(1).strip()
                    else:
                        pty = p
                    params.append(parse_type_str(pty))

            methods.append({
                "name": name,
                "params": params,
                "ret": parse_type_str(ret),
                "is_virtual": False,
            })

    return fields, methods


def derive_mangled_method_name(vitro_class: str, method: str) -> str:
    """根据 Vitro mangling 规则生成方法函数名。

    普通方法: {class}__{method}
    析构函数 (源码 ~Class()): __dtor__{class}
    """
    if method == "destroy":
        return f"__dtor__{vitro_class}"
    return f"{vitro_class}__{method}"


def process_cpp_file(path: Path) -> list[dict] | None:
    """处理单个 .cpp 文件，返回每个显式实例化的 class_data 列表；非容器文件返回 None。"""
    if path.name not in FILE_RULES:
        return None

    base_vitro, base_cpp, type_args = FILE_RULES[path.name]
    text = path.read_text(encoding='utf-8')
    text = strip_comments(text)

    body = extract_class_definition(text, base_vitro)
    if not body:
        print(f"Warning: class {base_vitro} not found in {path}", file=sys.stderr)
        return None

    results = []
    for type_arg in type_args:
        vitro_name = mangle_template_name(base_vitro, type_arg)
        cpp_name = cpp_type_name(base_cpp, type_arg)

        # 替换字段和方法中的模板参数 T
        replaced_body = replace_type_param(body, "T", type_arg)
        fields, methods = parse_fields_and_methods(replaced_body, vitro_name, base_vitro)
        size = sum(parse_type_size(f["type"]) for f in fields)

        method_map = {}
        for m in methods:
            mangled = derive_mangled_method_name(vitro_name, m["name"])
            method_map[m["name"]] = mangled

        results.append({
            "vitro_name": vitro_name,
            "cpp_name": cpp_name,
            "source_file": str(path).replace("\\", "/"),
            "size": size,
            "fields": fields,
            "methods": methods,
            "method_map": method_map,
        })

    return results


def main():
    all_classes = {}
    all_method_map = {}

    for cpp_path in sorted(INPUT_DIR.glob("*.cpp")):
        results = process_cpp_file(cpp_path)
        if results is None:
            continue
        for data in results:
            vitro_name = data["vitro_name"]
            all_classes[vitro_name] = {
                "cpp_name": data["cpp_name"],
                "source_file": data["source_file"],
                "size": data["size"],
                "fields": data["fields"],
                "methods": data["methods"],
            }
            all_method_map[vitro_name] = data["method_map"]

    output = {
        "version": 2,
        "generated_at": datetime.now(timezone.utc).isoformat(),
        "classes": all_classes,
        "method_map": all_method_map,
    }

    OUTPUT_PATH.parent.mkdir(parents=True, exist_ok=True)
    with open(OUTPUT_PATH, 'w', encoding='utf-8') as f:
        json.dump(output, f, indent=2, ensure_ascii=False)

    print(f"Generated {OUTPUT_PATH} with {len(all_classes)} classes.")


if __name__ == "__main__":
    main()
