# -*- coding: utf-8 -*-
"""Clang 外部 oracle 审计：逐个用例独立跑 clang，判定"外部 golden 是否成立"。

Q3 行为债量化需要回答的第一个问题：**662 个用例里，有多少个的"预期值"是外部可推导的
（clang 可编译 + 可运行），有多少个没有任何外部 oracle（clang 失败 / 自我豁免 / 已知偏差）**。

本脚本不读 shadow 报告、不读缓存，直接用 clang 子进程重算一遍（规模 ~662 次编译）。
判定口径：
  * golden_ok      clang 编译+运行成功 → stdout 是外部 golden，任何重写都必须复刻
  * clang_compile_fail  clang 编译失败 → 该用例无外部 golden（vitro_better 通道）
  * clang_run_fail      clang 能编但运行失败 → 无 golden
  * self_exempt        源码 `@category` 含 "bug" → shadow 驱动 analyze_diff 直接判 known_issue，
                       输出差异被豁免（"自我豁免通道"，驱动源码为证）
  * known_issue_pin    KNOWN_FAILURE_CASES 常量登记 → 行为由文档约定而非 clang 决定

输出：clang_oracle_audit.json + 控制台摘要。标注 [动态实测]。

用法：python scripts/core_asset_verdict/clang_oracle_audit.py [--jobs 8]
"""
import io
import json
import re
import sys
import time
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

sys.stdout = io.TextIOWrapper(sys.stdout.buffer, encoding="utf-8", errors="replace")

ROOT = Path(__file__).resolve().parent.parent.parent
SHADOW_DIR = ROOT / "native" / "tests" / "shadow_verification"
sys.path.insert(0, str(SHADOW_DIR))

import shadow_verify as sv  # noqa: E402

WORK = Path(__file__).parent / ".oracle_work"


def audit_one(case) -> dict:
    # 每个用例独占工作目录：共享目录会让并发用例互相覆盖 test.c/test.exe，
    # 产生"运行的是别人的程序"的静默错配（本探针首版实测踩到并修正）。
    work = WORK / f"{case.src_dir}__{case.name}"
    work.mkdir(parents=True, exist_ok=True)
    res = sv.run_with_clang(case.source, path=case.path, work_dir=work, stdin_text=case.stdin)
    if res.compile_success and res.run_success:
        verdict = "golden_ok"
    elif not res.compile_success:
        verdict = "clang_compile_fail"
    else:
        verdict = "clang_run_fail"
    return {
        "name": case.name,
        "src_dir": case.src_dir,
        "category": case.category,
        "verdict": verdict,
        "clang_compile": res.compile_success,
        "clang_run": res.run_success,
        "stdout_len": len(res.stdout or ""),
        "self_exempt": "bug" in case.category,
        "known_issue_pin": case.name in sv.KNOWN_FAILURE_CASES,
        "has_stdin": bool(case.stdin),
    }


def main() -> int:
    jobs = 8
    if "--jobs" in sys.argv:
        jobs = int(sys.argv[sys.argv.index("--jobs") + 1])
    t0 = time.time()
    cases = sv.FILE_CASES
    results = []
    with ThreadPoolExecutor(max_workers=jobs) as ex:
        for i, r in enumerate(ex.map(audit_one, cases), 1):
            results.append(r)
            if i % 100 == 0:
                print(f"  ... {i}/{len(cases)}", flush=True)
    out = Path(__file__).parent / "clang_oracle_audit.json"
    out.write_text(json.dumps(results, ensure_ascii=False, indent=1), encoding="utf-8")

    by_dir = {}
    for r in results:
        d = by_dir.setdefault(r["src_dir"], {"n": 0, "golden_ok": 0, "compile_fail": 0, "run_fail": 0})
        d["n"] += 1
        if r["verdict"] == "golden_ok":
            d["golden_ok"] += 1
        elif r["verdict"] == "clang_compile_fail":
            d["compile_fail"] += 1
        else:
            d["run_fail"] += 1

    golden = [r for r in results if r["verdict"] == "golden_ok"]
    no_golden = [r for r in results if r["verdict"] != "golden_ok"]
    exempt = [r for r in results if r["self_exempt"]]
    ki = [r for r in results if r["known_issue_pin"]]

    print(f"\n总用例 {len(results)}  耗时 {time.time()-t0:.1f}s")
    print(f"外部 golden 成立（clang 编+跑成功）: {len(golden)} ({len(golden)*100//len(results)}%)")
    print(f"无外部 golden（clang 失败）        : {len(no_golden)}")
    print(f"  其中 clang 编译失败              : {sum(1 for r in no_golden if r['verdict']=='clang_compile_fail')}")
    print(f"  其中 clang 运行失败              : {sum(1 for r in no_golden if r['verdict']=='clang_run_fail')}")
    print(f"自我豁免通道（@category 含 bug）   : {len(exempt)}  {[r['name'] for r in exempt]}")
    print(f"known_issue 常量登记              : {len(ki)}  {[r['name'] for r in ki]}")
    print("\n按目录:")
    for d, v in sorted(by_dir.items()):
        print(f"  {d:10s} n={v['n']:4d} golden_ok={v['golden_ok']:4d} "
              f"compile_fail={v['compile_fail']:3d} run_fail={v['run_fail']:3d}")
    print("\n无外部 golden 用例清单:")
    for r in no_golden:
        print(f"  {r['src_dir']:9s} {r['name']:32s} {r['verdict']}")
    print(f"\nJSON 已写出: {out}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
