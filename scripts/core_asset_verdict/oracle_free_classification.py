# -*- coding: utf-8 -*-
"""无外部 oracle 用例在**影子门禁口径下**被算作什么（Q2 伪绿通道量化）。

对 clang 侧 golden 不成立的那批用例，按影子驱动 `analyze_diff` 的真实规则重新判定一次
（不读报告文件、直接跑两侧），回答："有多少用例的『通过』完全不包含输出比对"。

按驱动规则：
  * clang 编译失败 + vitro 编译/运行成功 → `vitro_better`（**通过，且不比对任何输出**）
  * 两侧都失败                        → `match`（**通过，stdout 均为空**）
  * clang 运行失败 + vitro 运行失败     → `match`（同上）

用法：python scripts/core_asset_verdict/oracle_free_classification.py
输出：oracle_free_classification.json。标注 [动态实测]。
"""
import io
import json
import sys
from pathlib import Path

sys.stdout = io.TextIOWrapper(sys.stdout.buffer, encoding="utf-8", errors="replace")
HERE = Path(__file__).resolve().parent
ROOT = HERE.parent.parent
SHADOW = ROOT / "native/tests/shadow_verification"
sys.path.insert(0, str(SHADOW))
import shadow_verify as sv  # noqa: E402

WORK = HERE / ".oracle_work2"


def main() -> int:
    audit = json.loads((HERE / "clang_oracle_audit.json").read_text(encoding="utf-8"))
    targets = [r for r in audit if r["verdict"] != "golden_ok"]
    out = []
    for r in targets:
        case = next(c for c in sv.FILE_CASES if c.name == r["name"])
        work = WORK / f"{case.src_dir}__{case.name}"
        work.mkdir(parents=True, exist_ok=True)
        clang = sv.run_with_clang(case.source, path=case.path, work_dir=work, stdin_text=case.stdin)
        vitro = sv.run_with_vitro(case.source, filename=str(case.path), stdin_text=case.stdin)
        diff = sv.analyze_diff(case, clang, vitro)
        # 该分类下驱动是否真的做过 stdout 比对（按 analyze_diff 的实际分支）
        both_compiled = clang.compile_success and vitro.compile_success
        both_ran_failed = not clang.run_success and not vitro.run_success
        output_compared = bool(both_compiled and not both_ran_failed)
        out.append({
            "name": case.name, "src_dir": case.src_dir, "category": case.category,
            "clang_compile": clang.compile_success, "clang_run": clang.run_success,
            "vitro_compile": vitro.compile_success, "vitro_run": vitro.run_success,
            "clang_stdout": (clang.stdout or "")[:80], "vitro_stdout": (vitro.stdout or "")[:80],
            "driver_diff_type": diff.diff_type,
            "gated_as_pass": diff.diff_type in ("match", "known_issue", "vitro_better"),
            "output_compared": output_compared,
        })
    p = HERE / "oracle_free_classification.json"
    p.write_text(json.dumps(out, ensure_ascii=False, indent=1), encoding="utf-8")
    from collections import Counter
    print(f"无外部 oracle 用例: {len(out)}")
    print("驱动分类:", dict(Counter(r["driver_diff_type"] for r in out)))
    print(f"被门禁算作通过: {sum(1 for r in out if r['gated_as_pass'])}")
    print(f"  其中**零 stdout 比对**（vitro_better / 两侧都失败）: "
          f"{sum(1 for r in out if not r['output_compared'])}")
    for r in out:
        print(f"  {r['driver_diff_type']:12s} {r['src_dir']:9s} {r['name']:28s} "
              f"clang(compile={r['clang_compile']},run={r['clang_run']}) "
              f"vitro(compile={r['vitro_compile']},run={r['vitro_run']})")
    print(f"\nJSON 已写出: {p}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
