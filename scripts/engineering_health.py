#!/usr/bin/env python3
"""
工程健康度看板脚本

目标：
1. 统计项目关键健康度指标：超大文件、TODO/FIXME/HACK、unwrap/expect、
    失败记录活跃问题、Shadow Verification 匹配率。
2. 生成 `reports/engineering_health.md`，供维护者定期 review。

使用方式：
    python scripts/engineering_health.py
"""

import re
import subprocess
import io
import sys
# Windows CI 控制台默认 cp1252，编码不了中文/✓ 等字符（UnicodeEncodeError）。
# 与 shadow_verify.py 的入口处理一致：显式把 stdout/stderr 重包为 UTF-8。
sys.stdout = io.TextIOWrapper(sys.stdout.buffer, encoding="utf-8", errors="replace")
sys.stderr = io.TextIOWrapper(sys.stderr.buffer, encoding="utf-8", errors="replace")

from collections import Counter
from datetime import datetime, timezone
from pathlib import Path

# 确保 Windows 控制台输出 UTF-8 不乱码
if hasattr(sys.stdout, "reconfigure"):
    sys.stdout.reconfigure(encoding="utf-8")

# ─── 配置 ────────────────────────────────────────────────────────────────────

PROJECT_ROOT = Path(__file__).resolve().parent.parent
REPORTS_DIR = PROJECT_ROOT / "reports"
NATIVE_SRC = PROJECT_ROOT / "native" / "src"
NATIVE_CRATES = PROJECT_ROOT / "native" / "crates"
NATIVE_CODE_DIRS = [NATIVE_SRC, NATIVE_CRATES]
SHADOW_REPORT_DIR = PROJECT_ROOT / "native" / "tests" / "shadow_verification" / "reports"
FAILURES_FILES = [
    PROJECT_ROOT / "native" / "tests" / "FUZZ_FAILURES.md",
    PROJECT_ROOT / "native" / "tests" / "HOST_CONTRACT_FAILURES.md",
    PROJECT_ROOT / "native" / "tests" / "BYTECODE_LIBC_FAILURES.md",
    PROJECT_ROOT / "native" / "tests" / "DIFFERENTIAL_FAILURES.md",
    # 注意：根目录 GOLDEN_FAILURES.md 已迁移至 cases_golden/ 下
    PROJECT_ROOT / "native" / "tests" / "KR_FAILURES.md",
    PROJECT_ROOT / "native" / "tests" / "E2E_FAILURES.md",
    PROJECT_ROOT / "native" / "tests" / "LEETCODE_FAILURES.md",
    PROJECT_ROOT / "native" / "tests" / "CPP_FAILURES.md",
    PROJECT_ROOT / "native" / "tests" / "DOGFOODING_FAILURES.md",
    PROJECT_ROOT / "native" / "tests" / "cases_golden" / "GOLDEN_FAILURES.md",
]

TOP_N = 20

# ─── 工具函数 ─────────────────────────────────────────────────────────────────


def count_lines(path: Path) -> int:
    """统计文件非空行数。"""
    if not path.exists():
        return 0
    try:
        text = path.read_text(encoding="utf-8", errors="ignore")
    except Exception:
        return 0
    return sum(1 for line in text.splitlines() if line.strip())


def gather_files(root: Path | list[Path], suffix: str) -> list[Path]:
    """递归收集指定后缀文件，排除生成目录与 target。"""
    roots = [root] if isinstance(root, Path) else root
    exclude = {"target", ".dart_tool", "build"}
    ignored_names = {"frb_generated.rs", "frb_generated.dart", "frb_generated.io.dart", "frb_generated.web.dart"}
    files = []
    for r in roots:
        if not r.exists():
            continue
        for p in r.rglob(f"*.{suffix}"):
            if any(part in exclude for part in p.parts):
                continue
            if p.name in ignored_names:
                continue
            files.append(p)
    return files


def top_files_by_lines(root: Path, suffix: str, n: int = TOP_N) -> list[tuple[Path, int]]:
    files = gather_files(root, suffix)
    return sorted(((p, count_lines(p)) for p in files), key=lambda x: x[1], reverse=True)[:n]


def count_pattern_in_files(root: Path, suffix: str, pattern: str) -> tuple[int, Counter]:
    regex = re.compile(pattern)
    total = 0
    per_file = Counter()
    for p in gather_files(root, suffix):
        try:
            text = p.read_text(encoding="utf-8", errors="ignore")
        except Exception:
            continue
        count = len(regex.findall(text))
        if count:
            total += count
            per_file[p.relative_to(PROJECT_ROOT)] += count
    return total, per_file


def count_production_unwrap_expect() -> tuple[int, Counter]:
    """统计生产代码中的 unwrap/expect 数量。

    排除：
    - `#[cfg(test)]` 模块
    - `#[test]` 标注的测试函数
    （R2 后：FRB 生成文件已随前端切割移除，保留排除逻辑无副作用）
    """
    pattern = re.compile(r"\b(unwrap\(\)|expect\()")
    per_file = Counter()
    total = 0
    for p in gather_files(NATIVE_CODE_DIRS, "rs"):
        if p.name == "frb_generated.rs":
            continue
        text = p.read_text(encoding="utf-8", errors="ignore")
        lines = text.splitlines()
        in_test_mod = False
        in_test_fn = False
        mod_brace_depth = 0
        fn_brace_depth = 0
        local_count = 0
        i = 0
        while i < len(lines):
            line = lines[i]
            stripped = line.strip()
            if re.search(r"#\[cfg\(test\)\]", stripped):
                in_test_mod = True
                mod_brace_depth = 0
                i += 1
                continue
            if in_test_mod:
                mod_brace_depth += stripped.count("{") - stripped.count("}")
                if mod_brace_depth < 0:
                    in_test_mod = False
                i += 1
                continue
            if stripped == "#[test]":
                in_test_fn = True
                fn_brace_depth = 0
                i += 1
                continue
            if in_test_fn:
                fn_brace_depth += stripped.count("{") - stripped.count("}")
                if fn_brace_depth < 0:
                    in_test_fn = False
                i += 1
                continue
            if pattern.search(line):
                local_count += 1
            i += 1
        if local_count:
            total += local_count
            per_file[p.relative_to(PROJECT_ROOT)] += local_count
    return total, per_file


def count_active_failure_entries() -> tuple[int, dict[Path, int]]:
    """统计各失败记录文件中的活跃条目数。

    规则：
    - 仅统计位于 `KNOWN_FAILURE` / `KNOWN_DIVERGENCE` / `KNOWN_LIMITATION`
      二级标题下的条目。
    - 条目包括：
      - `### ` 开头的独立条目。
      - KNOWN_* section 下的表格数据行（表头与分隔线不计入）。
    - 包含 `已修复`、`FIXED`、`RESOLVED`、`不再失败` 等字样的条目不计入。
    """
    active_section = re.compile(
        r"^#{2}\s+.*?(KNOWN_FAILURE|KNOWN_DIVERGENCE|KNOWN_LIMITATION)",
        re.I,
    )
    entry_start = re.compile(r"^#{3}\s+")
    section_start = re.compile(r"^#{2}\s+")
    resolved_markers = re.compile(r"已修复|FIXED|RESOLVED|不再失败|no longer fails", re.I)
    table_divider = re.compile(r"^\|[-:\|\s]+\|$")
    per_file = {}
    total = 0
    for md in FAILURES_FILES:
        if not md.exists():
            per_file[md] = 0
            continue
        text = md.read_text(encoding="utf-8", errors="ignore")
        lines = text.splitlines()
        count = 0
        in_active_section = False
        inside_table = False
        for line in lines:
            stripped = line.strip()
            if section_start.match(stripped):
                in_active_section = bool(active_section.match(stripped))
                inside_table = False
                continue
            if not in_active_section:
                continue
            # 独立条目
            if entry_start.match(stripped):
                if not resolved_markers.search(stripped):
                    count += 1
                inside_table = False
                continue
            # 表格分隔线标志后续行为表格数据行
            if table_divider.match(stripped):
                inside_table = True
                continue
            # 表格数据行
            if inside_table and stripped.startswith("|") and stripped.endswith("|"):
                if not resolved_markers.search(stripped):
                    count += 1
                continue
            # 非空非表格行，退出表格状态
            if stripped and not stripped.startswith("|"):
                inside_table = False
        per_file[md] = count
        total += count
    return total, per_file


def read_shadow_match_rate() -> dict[str, str]:
    """读取 Shadow Verification 报告中的匹配率摘要。"""
    import json

    result: dict[str, str] = {}

    # C++ 报告
    cpp_report = SHADOW_REPORT_DIR / "cpp_shadow_report.json"
    if cpp_report.exists():
        try:
            data = json.loads(cpp_report.read_text(encoding="utf-8"))
            if isinstance(data, list):
                total = len(data)
                matched = sum(1 for c in data if c.get("diff_type") == "match")
            else:
                total = data.get("total", 0)
                matched = data.get("matched", 0)
            result["C++"] = f"{matched}/{total}"
        except Exception:
            result["C++"] = "N/A"
    else:
        result["C++"] = "N/A"

    # C 报告：读取 shadow_data_latest.json（由 scripts/shadow_verify.go 同步更新）
    latest = SHADOW_REPORT_DIR / "shadow_data_latest.json"
    if latest.exists():
        try:
            data = json.loads(latest.read_text(encoding="utf-8"))
            summary = data.get("summary", {})
            details = data.get("details", [])
            total = summary.get("total", 0)
            # 与 README/AGENTS.md 对外口径一致：完全匹配 + Vitro 更优 + 已知差异均计入匹配
            matched = sum(
                1
                for c in details
                if c.get("diff_type") in ("match", "vitro_better", "known_issue")
            )
            result["C"] = f"{matched}/{total}"
        except Exception:
            result["C"] = "N/A"
    else:
        result["C"] = "N/A"
    return result


def get_git_rev() -> str:
    try:
        return subprocess.check_output(
            ["git", "rev-parse", "--short", "HEAD"],
            cwd=PROJECT_ROOT,
            text=True,
            stderr=subprocess.DEVNULL,
        ).strip()
    except Exception:
        return "unknown"


def get_git_dirty() -> bool:
    try:
        out = subprocess.check_output(
            ["git", "status", "--short"],
            cwd=PROJECT_ROOT,
            text=True,
            stderr=subprocess.DEVNULL,
        ).strip()
        return bool(out)
    except Exception:
        return False


# ─── 报告生成 ─────────────────────────────────────────────────────────────────


def generate_report() -> str:
    now = datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%S%z")
    rev = get_git_rev()
    dirty = " (dirty)" if get_git_dirty() else ""

    rust_top = top_files_by_lines(NATIVE_CODE_DIRS, "rs")

    todo_total, todo_per_file = count_pattern_in_files(
        PROJECT_ROOT / "native", "rs", r"//\s*(TODO|FIXME|HACK)"
    )

    unwrap_total, unwrap_per_file = count_pattern_in_files(
        NATIVE_CODE_DIRS, "rs", r"\b(unwrap\(\)|expect\()"
    )
    # 生产代码 unwrap/expect：排除测试代码（FRB 生成文件已移除，注释保留历史口径）
    prod_unwrap_total, prod_unwrap_per_file = count_production_unwrap_expect()

    active_failures, failures_per_file = count_active_failure_entries()
    shadow_rates = read_shadow_match_rate()

    lines: list[str] = []
    lines.append("# Vitro 工程健康度看板")
    lines.append("")
    lines.append(f"> 生成时间: {now}")
    lines.append(f"> 基线提交: `{rev}{dirty}`")
    lines.append("")
    lines.append("## 摘要")
    lines.append("")
    lines.append("| 指标 | 数值 |")
    lines.append("|------|------|")
    lines.append(f"| Rust TODO/FIXME/HACK | {todo_total} |")
    lines.append(f"| Rust unwrap/expect（全量） | {unwrap_total} |")
    lines.append(f"| Rust unwrap/expect（生产代码） | {prod_unwrap_total} |")
    lines.append(f"| 活跃失败记录条目 | {active_failures} |")
    lines.append(f"| C Shadow Verification | {shadow_rates.get('C', 'N/A')} |")
    lines.append(f"| C++ Shadow Verification | {shadow_rates.get('C++', 'N/A')} |")
    lines.append("")

    lines.append("## Rust 源文件行数 Top 20")
    lines.append("")
    lines.append("| 排名 | 文件 | 非空行数 |")
    lines.append("|------|------|----------|")
    for i, (p, n) in enumerate(rust_top, 1):
        rel = p.relative_to(PROJECT_ROOT)
        lines.append(f"| {i} | `{rel}` | {n} |")
    lines.append("")


    lines.append("## Rust TODO/FIXME/HACK 分布（Top 10）")
    lines.append("")
    lines.append("| 文件 | 数量 |")
    lines.append("|------|------|")
    for p, n in todo_per_file.most_common(10):
        lines.append(f"| `{p}` | {n} |")
    lines.append("")


    lines.append("## Rust unwrap/expect 分布（Top 10）")
    lines.append("")
    lines.append("| 文件 | 数量 |")
    lines.append("|------|------|")
    for p, n in unwrap_per_file.most_common(10):
        lines.append(f"| `{p}` | {n} |")
    lines.append("")

    lines.append("## Rust 生产代码 unwrap/expect 分布（Top 10）")
    lines.append("")
    lines.append("| 文件 | 数量 |")
    lines.append("|------|------|")
    for p, n in prod_unwrap_per_file.most_common(10):
        lines.append(f"| `{p}` | {n} |")
    lines.append("")

    lines.append("## 失败记录文件活跃条目")
    lines.append("")
    lines.append("| 文件 | 活跃条目 |")
    lines.append("|------|----------|")
    for md, n in failures_per_file.items():
        rel = md.relative_to(PROJECT_ROOT)
        lines.append(f"| `{rel}` | {n} |")
    lines.append("")

    lines.append("## 趋势说明")
    lines.append("")
    lines.append("- 本报告仅反映当前快照，建议与历史报告对比观察趋势。")
    lines.append("- 若 Rust/Dart 超大文件持续膨胀，应触发新一轮拆分评估。")
    lines.append("- 若 `unwrap/expect` 数量持续上升，应评估错误处理健壮性。")
    lines.append("")

    return "\n".join(lines)


def main() -> None:
    REPORTS_DIR.mkdir(parents=True, exist_ok=True)
    report = generate_report()
    out_path = REPORTS_DIR / "engineering_health.md"
    out_path.write_text(report, encoding="utf-8")
    print(f"工程健康度看板已生成: {out_path}")


if __name__ == "__main__":
    main()
