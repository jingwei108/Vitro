# -*- coding: utf-8 -*-
"""测试套件切面普查：把 native/tests/*.rs 的每个 #[test] 归到使用切面。

目的（T3 / Q1）：量化"交互切面"在**测试防线**里的覆盖密度，而不是只看影子防线。
判定按测试体引用的 API/字段（不是文件名、不是测试名），每个测试只归一类（优先级从高到低）。
标注 [静态亲读 + 机检]。

用法：python scripts/core_asset_verdict/test_facet_census.py
输出：test_facet_census.json + 控制台摘要
"""
import io
import json
import re
import sys
from collections import Counter, defaultdict
from pathlib import Path

sys.stdout = io.TextIOWrapper(sys.stdout.buffer, encoding="utf-8", errors="replace")

NATIVE = Path(__file__).resolve().parent.parent.parent / "native"
TESTS = NATIVE / "tests"

# 切面判定：按优先级顺序，第一个命中的即归类（后面的不覆盖）
FACET_RULES = [
    ("interaction_payload", re.compile(
        r"step_next|step_begin|payloads?\b|payloads\(|StepPayload|semantic_label|"
        r"pointer_snapshots|array_snapshots|memory_regions|memory\.regions|\bseek\b|"
        r"breakpoints|code_line|local_vars")),
    ("resource_lifecycle", re.compile(
        r"quarantine|\brss\b|memory_usage|working_set|leak|is_freed|free_region|regions")),
    ("diagnostics", re.compile(
        r"error_catalog|Diagnostic|diagnostic|auto_fix|E3\d{3}|E1\d{3}|knowledge_graph")),
    ("batch_stdout", re.compile(
        r"program_output|get_output|vitro_run|stdout|printf\(")),
]
DEFAULT_FACET = "unit_other"

TEST_SPLIT = re.compile(r"#\[test\]")


def c_programs_in(body: str) -> int:
    """统计测试体里内嵌的 C 程序个数（`r#"` 原始字符串，含 main 的估计）。"""
    return len(re.findall(r'r#"\s*(?:#include|\s*\n)?', body)) or body.count("int main(")


def main() -> int:
    per_test = []
    for path in sorted(TESTS.glob("*.rs")):
        text = path.read_text(encoding="utf-8", errors="replace")
        chunks = TEST_SPLIT.split(text)[1:]
        for ch in chunks:
            m = re.search(r"fn\s+(\w+)", ch)
            if not m:
                continue
            name = m.group(1)
            # 只取函数体（到下一个 fn 定义前的段落足够做关键词判定）
            body = ch[:6000]
            facet = DEFAULT_FACET
            for fname, pat in FACET_RULES:
                if pat.search(body):
                    facet = fname
                    break
            per_test.append({
                "file": path.name,
                "test": name,
                "facet": facet,
                "c_programs": c_programs_in(body),
            })
    out = Path(__file__).parent / "test_facet_census.json"
    out.write_text(json.dumps(per_test, ensure_ascii=False, indent=1), encoding="utf-8")

    counts = Counter(t["facet"] for t in per_test)
    total = len(per_test)
    print(f"#[test] 总数: {total}（来自 {len(set(t['file'] for t in per_test))} 个测试文件）")
    for f, c in counts.most_common():
        print(f"  {f:20s} {c:4d}  ({c*100/max(1,total):.1f}%)")
    print("\n交互切面测试分布：")
    by_file = defaultdict(list)
    for t in per_test:
        if t["facet"] == "interaction_payload":
            by_file[t["file"]].append(t["test"])
    for f, names in sorted(by_file.items()):
        print(f"  {f}: {len(names)} 个 -> {', '.join(names[:6])}{' ...' if len(names) > 6 else ''}")
    print(f"\nJSON 已写出: {out}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
