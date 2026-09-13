# -*- coding: utf-8 -*-
"""662 用例的**执行规模**实测：逐个跑 `vitro_cli unified`，记录真实步数。

Q3/规模 realism 需要的是步数分布，不是行数。本脚本对每个现有用例实际执行一遍
（解释器 + 统一模式），记录：总步数 / 是否正常结束 / 是否 trap / 墙钟耗时。

用法：python scripts/core_asset_verdict/measure_steps.py [--jobs 8] [--max-steps 200000]
输出：case_steps.json + 控制台摘要。标注 [动态实测]。
"""
import io
import json
import re
import subprocess
import sys
import time
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

sys.stdout = io.TextIOWrapper(sys.stdout.buffer, encoding="utf-8", errors="replace")

ROOT = Path(__file__).resolve().parent.parent.parent
NATIVE = ROOT / "native"
CLI = NATIVE / "target/release/vitro_cli.exe"
if not CLI.exists():
    CLI = NATIVE / "target/debug/vitro_cli.exe"

SHADOW_DIR = NATIVE / "tests" / "shadow_verification"
sys.path.insert(0, str(SHADOW_DIR))
import shadow_verify as sv  # noqa: E402

STEPS_RE = re.compile(r"总步数:\s*(-?\d+)")


def one(case, max_steps: int) -> dict:
    t0 = time.time()
    try:
        p = subprocess.run(
            [str(CLI), "unified", str(case.path), "--max-steps", str(max_steps)],
            capture_output=True, text=True, encoding="utf-8", errors="replace", timeout=90,
        )
        out = p.stdout or ""
        m = STEPS_RE.search(out)
        steps = int(m.group(1)) if m else None
        return {
            "name": case.name, "src_dir": case.src_dir,
            "steps": steps,
            "finished": "状态: 正常结束" in out,
            "trapped": "状态: 异常终止" in out,
            "waiting_input_gave_up": "输入已耗尽" in out,
            "wall_ms": int((time.time() - t0) * 1000),
            "exit": p.returncode,
        }
    except subprocess.TimeoutExpired:
        return {"name": case.name, "src_dir": case.src_dir, "steps": None,
                "finished": False, "trapped": False, "timeout": True,
                "wall_ms": int((time.time() - t0) * 1000), "exit": None}


def main() -> int:
    jobs, max_steps = 8, 200_000
    if "--jobs" in sys.argv:
        jobs = int(sys.argv[sys.argv.index("--jobs") + 1])
    if "--max-steps" in sys.argv:
        max_steps = int(sys.argv[sys.argv.index("--max-steps") + 1])
    t0 = time.time()
    cases = sv.FILE_CASES
    results = []
    with ThreadPoolExecutor(max_workers=jobs) as ex:
        for i, r in enumerate(ex.map(lambda c: one(c, max_steps), cases), 1):
            results.append(r)
            if i % 100 == 0:
                print(f"  ... {i}/{len(cases)}", flush=True)
    out = Path(__file__).parent / "case_steps.json"
    out.write_text(json.dumps(results, ensure_ascii=False, indent=1), encoding="utf-8")

    steps = sorted(r["steps"] for r in results if r["steps"] is not None)
    n = len(results)
    def pct(p):
        return steps[min(len(steps) - 1, int(len(steps) * p / 100))] if steps else None
    print(f"\n总用例 {n}  耗时 {time.time()-t0:.1f}s  max_steps={max_steps}")
    print(f"测得步数 {len(steps)} 例；中位 {pct(50)}  均值 {sum(steps)/max(1,len(steps)):.0f}  "
          f"p90 {pct(90)}  p99 {pct(99)}  最大 {steps[-1] if steps else 0}")
    for thr in (100, 1000, 10000, 100000):
        print(f"  步数 >= {thr:6d}: {sum(1 for s in steps if s >= thr)}")
    print(f"正常结束 {sum(1 for r in results if r['finished'])}  "
          f"异常终止 {sum(1 for r in results if r['trapped'])}  "
          f"超时 {sum(1 for r in results if r.get('timeout'))}")
    print(f"\nJSON 已写出: {out}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
