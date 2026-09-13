#!/usr/bin/env python3
"""
三层契约验证 CI 脚本（Phase F）

目标：
1. 独立运行 Phase A/B/C/E 测试，确保三层契约在 CI 中被显式验证。
2. 生成综合报告，记录每层契约的通过状态。
3. 检查 *_FAILURES.md 的一致性：
   - 若文档中标记为 KNOWN_FAILURE / KNOWN_DIVERGENCE 的测试现在通过了，
     提示更新文档。
   - 若测试失败了但文档中没有对应记录，提示添加记录。
4. 报告输出到 reports/three_tier_report.md，并作为 CI artifact 上传。

使用方式：
    python scripts/ci_three_tier_check.py
"""

import os
import re
import subprocess
import io
import sys
# Windows CI 控制台默认 cp1252，编码不了中文/✓ 等字符（UnicodeEncodeError）。
# 与 shadow_verify.py 的入口处理一致：显式把 stdout/stderr 重包为 UTF-8。
sys.stdout = io.TextIOWrapper(sys.stdout.buffer, encoding="utf-8", errors="replace")
sys.stderr = io.TextIOWrapper(sys.stderr.buffer, encoding="utf-8", errors="replace")

from dataclasses import dataclass
from pathlib import Path

# ─── 配置 ────────────────────────────────────────────────────────────────────

NATIVE_DIR = Path("native")
REPORTS_DIR = Path("reports")
TESTS_DIR = NATIVE_DIR / "tests"

TIER_TESTS = [
    ("Phase A", "host_contract_tests", "HOST_CONTRACT_FAILURES.md"),
    ("Phase B", "bytecode_libc_consistency", "BYTECODE_LIBC_FAILURES.md"),
    ("Phase C", "differential_stress", "DIFFERENTIAL_FAILURES.md"),
    ("Phase E", "fuzz_stress_test", "FUZZ_FAILURES.md"),
    ("K&R / E2E / LeetCode / C++", "vitro_e2e", ["KR_FAILURES.md", "E2E_FAILURES.md", "LEETCODE_FAILURES.md", "CPP_FAILURES.md"]),
    ("C++ Parser", "parser_cpp_unit_test", "CPP_FAILURES.md"),
    ("C++ TypeChecker", "typeck_cpp_unit_test", "CPP_FAILURES.md"),
    ("C++ BytecodeGen", "bytecode_gen_cpp_unit_test", "CPP_FAILURES.md"),
    ("C++ Dogfooding", "cpp_dogfooding_test", "DOGFOODING_FAILURES.md"),
]

# 已知为设计决策、不需要修复的差异（不视为 CI 失败）
EXPECTED_DIVERGENCES = {
    "test_diff_abs",
    "test_diff_atoi",
}


# ─── 数据类 ──────────────────────────────────────────────────────────────────

@dataclass
class TierResult:
    phase: str
    test_file: str
    failures_md: list
    passed: bool
    tests_run: int = 0
    tests_passed: int = 0
    tests_failed: int = 0
    stdout: str = ""
    stderr: str = ""
    failed_tests: list = None

    def __post_init__(self):
        if self.failed_tests is None:
            self.failed_tests = []
        if isinstance(self.failures_md, str):
            self.failures_md = [self.failures_md]


# ─── 测试运行 ─────────────────────────────────────────────────────────────────


_test_cache: dict[str, subprocess.CompletedProcess] = {}


def run_cargo_test(test_file: str) -> subprocess.CompletedProcess:
    """运行指定的 cargo integration test（带缓存，避免同一 test_file 重复运行）。"""
    if test_file not in _test_cache:
        _test_cache[test_file] = _run_cargo_test_uncached(test_file)
    return _test_cache[test_file]


def _run_cargo_test_uncached(test_file: str) -> subprocess.CompletedProcess:
    """运行指定的 cargo integration test。"""
    cmd = [
        "cargo",
        "test",
        "--test",
        test_file,
        "--",
        "--test-threads=1",
    ]
    return subprocess.run(
        cmd,
        cwd=NATIVE_DIR,
        capture_output=True,
        text=True,
        encoding="utf-8",
        errors="replace",
    )


def parse_test_output(output: str) -> dict:
    """从 cargo test 输出中提取统计信息。

    E-P0-2：新增 "found" 标志。cargo 编译失败/依赖拉取失败时输出中没有任何
    "test result:" 行，此前返回全 0 统计 → failed==0 → 被误判 PASS。
    """
    stats = {
        "run": 0,
        "passed": 0,
        "failed": 0,
        "ignored": 0,
        "failed_names": [],
        "found": False,
    }

    # 匹配 "test result: ok. 38 passed; 0 failed; 0 ignored"
    result_line = re.search(
        r"test result:\s+(ok|FAILED)\.\s+(\d+)\s+passed;\s+(\d+)\s+failed;\s+(\d+)\s+ignored",
        output,
    )
    if result_line:
        stats["found"] = True
        stats["passed"] = int(result_line.group(2))
        stats["failed"] = int(result_line.group(3))
        stats["ignored"] = int(result_line.group(4))
        stats["run"] = stats["passed"] + stats["failed"]

    # 匹配失败的测试名 "test xxx ... FAILED"
    for m in re.finditer(r"^test\s+(\S+)\s+\.\.\.\s+FAILED", output, re.MULTILINE):
        stats["failed_names"].append(m.group(1))

    return stats


# ─── 文档一致性检查 ───────────────────────────────────────────────────────────


def extract_md_status(md_path: Path) -> dict:
    """从 *_FAILURES.md 中提取状态信息。"""
    if not md_path.exists():
        return {"missing": True, "entries": []}

    content = md_path.read_text(encoding="utf-8")

    # 先移除 HTML 注释（包括多行），避免占位符 <case_name> 被误判
    content = re.sub(r"<!--[\s\S]*?-->", "", content)

    entries = []

    # 按二级标题拆分文档，逐个 section 解析
    section_pattern = re.compile(r"^##\s+", re.MULTILINE)
    parts = section_pattern.split(content)
    for part in parts[1:]:
        lines = part.splitlines()
        if not lines:
            continue
        section_title = lines[0].strip()
        section_text = "\n".join(lines[1:])

        is_known_failure = "KNOWN_FAILURE" in section_title or "已知失败" in section_title
        is_divergence = "KNOWN_DIVERGENCE" in section_title or "已知偏差" in section_title

        # 提取该 section 下的所有三级标题
        for tm in re.finditer(r"^###\s+(.+)$", section_text, re.MULTILINE):
            title = tm.group(1).strip()

            # 标题本身带删除线 → 已修复
            fixed_match = re.match(r"~~(.+?)~~\s*(?:→\s*(?:已修复|FIXED))?", title)
            if fixed_match:
                entries.append({
                    "title": fixed_match.group(1).strip(),
                    "status": "FIXED",
                })
                continue

            # 检查该标题所在段落是否包含"已修复"字样
            paragraph_start = tm.end()
            next_heading = re.search(r"^###\s+", section_text[paragraph_start:], re.MULTILINE)
            paragraph_end = paragraph_start + next_heading.start() if next_heading else len(section_text)
            paragraph = section_text[paragraph_start:paragraph_end]
            if "已修复" in paragraph or "FIXED" in paragraph:
                entries.append({"title": title, "status": "FIXED"})
                continue

            # 根据 section 类型分类
            if is_divergence:
                entries.append({"title": title, "status": "DIVERGENCE"})
            elif is_known_failure:
                entries.append({"title": title, "status": "KNOWN"})

    return {"missing": False, "entries": entries}


def check_consistency(result: TierResult) -> tuple:
    """检查测试结果与 *_FAILURES.md 的一致性。

    E-P0-3：返回 (hard_issues, soft_issues)。
    - hard（计入 CI 退出码）：确定性不一致 ——
      1) 文档声明 KNOWN_FAILURE 但测试现在全部通过（文档过期，须更新）；
      2) 失败记录文件本身缺失。
      这实现 AGENTS.md 防线 5 声明的「KNOWN_FAILURE 现在通过 → 报错」方向。
    - soft（仅 [WARN] 提示，不阻塞）：测试有失败时的"请确保已记录"提醒 ——
      文档为自由文本，无法精确匹配失败用例名，硬失败会产生持续误报；
      精确双向对账由 vitro_e2e.rs 的 KNOWN_* 常量机制闭环承担。
    """
    hard, soft = [], []
    for failures_md in result.failures_md:
        md_path = TESTS_DIR / failures_md
        md_info = extract_md_status(md_path)

        if md_info["missing"]:
            hard.append(f"缺少失败记录文件: {failures_md}")
            continue

        # 检查 KNOWN_FAILURE 是否仍然失败
        # KNOWN_DIVERGENCE（设计决策导致的偏差）不视为需要修复的故障，测试通过是正常的
        if result.passed:
            known_entries = [e for e in md_info["entries"] if e["status"] == "KNOWN"]
            if known_entries:
                titles = ", ".join(e["title"][:40] for e in known_entries)
                hard.append(
                    f"Tests all passed, but {failures_md} still has {len(known_entries)} un-fixed KNOWN entries: {titles}"
                    f"（KNOWN_FAILURE 已通过，请更新文档标记为已修复）"
                )
        else:
            # 测试有失败，检查是否都已在文档中记录
            # 由于文档是自由文本，这里只能做粗略提醒（soft）
            fixed_count = len([e for e in md_info["entries"] if e["status"] == "FIXED"])
            soft.append(
                f"Tests have failures. Ensure all are recorded in {failures_md} (currently {fixed_count} FIXED records)"
            )

    return hard, soft


# ─── 报告生成 ─────────────────────────────────────────────────────────────────


def generate_report(results: list, consistency_issues: dict) -> str:
    """生成 Markdown 报告。"""
    lines = [
        "# 三层契约验证报告（Three Tier Test Report）",
        "",
        f"生成时间: {__import__('datetime').datetime.now().isoformat()}",
        "",
        "> 本报告由 CI 自动生成，对应 Phase F 要求。",
        "",
        "## 摘要",
        "",
        "| 阶段 | 测试文件 | 状态 | 通过 | 失败 | 忽略 |",
        "|------|----------|------|------|------|------|",
    ]

    all_passed = True
    for r in results:
        status = "✅ PASS" if r.passed else "❌ FAIL"
        if not r.passed:
            all_passed = False
        lines.append(
            f"| {r.phase} | `{r.test_file}` | {status} | {r.tests_passed} | {r.tests_failed} | {r.tests_run - r.tests_passed - r.tests_failed} |"
        )

    lines.extend([
        "",
        "## 一致性检查",
        "",
    ])

    has_issues = False
    for r in results:
        issues = consistency_issues.get(r.phase, [])
        if issues:
            has_issues = True
            lines.append(f"### {r.phase}")
            lines.append("")
            for issue in issues:
                lines.append(f"- ⚠️ {issue}")
            lines.append("")

    if not has_issues:
        lines.append("✅ 所有失败记录文档与测试结果一致。")
        lines.append("")

    lines.extend([
        "## 详细输出",
        "",
    ])

    for r in results:
        lines.append(f"### {r.phase}: {r.test_file}")
        lines.append("")
        if r.failed_tests:
            lines.append("**失败的测试:**")
            for name in r.failed_tests:
                lines.append(f"- `{name}`")
            lines.append("")
        lines.append("```")
        # 截取最后的 800 字符，避免报告过长
        tail = (r.stdout + "\n" + r.stderr)[-800:]
        lines.append(tail)
        lines.append("```")
        lines.append("")

    return "\n".join(lines) + "\n"


# ─── 主流程 ───────────────────────────────────────────────────────────────────


def main() -> int:
    REPORTS_DIR.mkdir(parents=True, exist_ok=True)

    results = []
    consistency_issues = {}
    any_hard_issue = False

    print("=" * 60)
    print("Three Tier Verification Start")
    print("=" * 60)

    for phase, test_file, failures_md in TIER_TESTS:
        print(f"\n>> {phase}: cargo test --test {test_file}")
        proc = run_cargo_test(test_file)
        stats = parse_test_output(proc.stdout + proc.stderr)

        # E-P0-2：cargo 本身失败（编译错误/依赖拉取失败，returncode != 0）或
        # 输出中解析不到 "test result:" 行时，不得误判 PASS。
        if not stats["found"]:
            print(f"   [ERROR] 未能从 cargo test 输出解析到 'test result:' 行 —— cargo 可能编译失败")
            print(f"   [ERROR] returncode={proc.returncode}")
            tail = ((proc.stderr or "") + "\n" + (proc.stdout or ""))[-600:]
            if tail.strip():
                print("   [ERROR] 输出尾部:")
                for line in tail.strip().splitlines()[-12:]:
                    print(f"      {line}")
        passed = proc.returncode == 0 and stats["found"] and stats["failed"] == 0
        result = TierResult(
            phase=phase,
            test_file=test_file,
            failures_md=failures_md,
            passed=passed,
            tests_run=stats["run"],
            tests_passed=stats["passed"],
            tests_failed=stats["failed"],
            stdout=proc.stdout,
            stderr=proc.stderr,
            failed_tests=stats["failed_names"],
        )
        results.append(result)

        status = "[PASS]" if passed else "[FAIL]"
        print(f"   {status} — {stats['passed']} passed, {stats['failed']} failed")

        # 一致性检查（E-P0-3：hard 计入退出码，soft 仅提示）
        hard_issues, soft_issues = check_consistency(result)
        consistency_issues[phase] = hard_issues + soft_issues
        if hard_issues:
            any_hard_issue = True
            for issue in hard_issues:
                print(f"   [ERROR] {issue}")
        for issue in soft_issues:
            print(f"   [WARN] {issue}")

    # 生成报告
    report_md = generate_report(results, consistency_issues)
    report_path = REPORTS_DIR / "three_tier_report.md"
    report_path.write_text(report_md, encoding="utf-8")
    print(f"\n[REPORT] Generated: {report_path}")

    # 最终判定（E-P0-3：一致性 hard 问题与测试失败同等阻塞 CI）
    all_passed = all(r.passed for r in results)
    if all_passed and not any_hard_issue:
        print("\n[SUCCESS] All three tier tests passed!")
        return 0
    else:
        if not all_passed:
            print("\n[FAILED] Some three tier tests failed. See report and *_FAILURES.md.")
        if any_hard_issue:
            print("[FAILED] 一致性检查存在 hard 问题（文档与测试结果矛盾），见上方 [ERROR]。")
        return 1


if __name__ == "__main__":
    sys.exit(main())
