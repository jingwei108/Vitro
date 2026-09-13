# -*- coding: utf-8 -*-
""""无外部 golden"用例的可救性测量（Q2/Q3 证据）。

背景（本裁定独立实测）：662 用例的 clang 侧 golden 由驱动 `run_with_clang` 生成，
它对**文件用例**直接编译原文件、不注入 `stdio.h`；只有内联用例才走 `make_clang_header`。
clang 22 默认 C23，隐式函数声明是**错误**——因此缺少 `#include <stdio.h>` 的用例
clang 直接编译失败，落进 `vitro_better` 或"两侧都失败 = match"。

本脚本对这 28 个无 golden 用例做**可救性测量**：在临时副本上补 `stdio.h`/`stdlib.h`
后重跑 clang，看它是否恢复为有效 golden（编译成功且与 Vitro stdout 一致）。
**不改动用例文件本身**（只读原文件 + 写临时副本）。

用法：python scripts/core_asset_verdict/oracle_gap_probe.py [--jobs 8]
输出：oracle_gap_probe.json + 控制台摘要。标注 [动态实测]。
"""
import io
import json
import sys
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

sys.stdout = io.TextIOWrapper(sys.stdout.buffer, encoding="utf-8", errors="replace")

ROOT = Path(__file__).resolve().parent.parent.parent
SHADOW = ROOT / "native" / "tests" / "shadow_verification"
sys.path.insert(0, str(SHADOW))
import shadow_verify as sv  # noqa: E402

WORK = Path(__file__).parent / ".oracle_rescue"

AUGMENT = "#include <stdio.h>\n#include <stdlib.h>\n#include <string.h>\n#include <math.h>\n"


def probe(entry: dict) -> dict:
    name = entry["name"]
    case = next(c for c in sv.FILE_CASES if c.name == name)
    src = case.source
    # 已有 include 的用例：仅补缺失的头（保持原有顺序不动）
    added = [h for h in ("stdio.h", "stdlib.h", "string.h", "math.h") if f"<{h}>" not in src]
    augmented = "".join(f"#include <{h}>\n" for h in added) + src
    # 独占工作目录（首版用 hash 分桶共享目录，并发下会跑错可执行文件 —— 实测踩到）
    work = WORK / f"{case.src_dir}__{name}"
    work.mkdir(parents=True, exist_ok=True)
    tmp = work / f"{name}_aug.c"
    tmp.write_text(augmented, encoding="utf-8")
    clang = sv.run_with_clang(augmented, path=None, work_dir=work, stdin_text=case.stdin)
    vitro = sv.run_with_vitro(case.source, filename=str(case.path), stdin_text=case.stdin)
    rescued = clang.compile_success and clang.run_success
    agree = rescued and (clang.stdout or "").strip() == (vitro.stdout or "").strip()
    return {
        "name": name, "src_dir": case.src_dir,
        "original_verdict": entry["verdict"],
        "added_headers": added,
        "rescued_compile_run": rescued,
        "stdout_agrees_after_rescue": agree,
        "clang_stdout": (clang.stdout or "")[:120],
        "vitro_stdout": (vitro.stdout or "")[:120],
        "clang_err": (clang.compile_error or "")[:200],
    }


def main() -> int:
    jobs = 8
    if "--jobs" in sys.argv:
        jobs = int(sys.argv[sys.argv.index("--jobs") + 1])
    audit = json.loads((Path(__file__).parent / "clang_oracle_audit.json").read_text(encoding="utf-8"))
    no_golden = [r for r in audit if r["verdict"] != "golden_ok"]
    with ThreadPoolExecutor(max_workers=jobs) as ex:
        results = list(ex.map(probe, no_golden))
    out = Path(__file__).parent / "oracle_gap_probe.json"
    out.write_text(json.dumps(results, ensure_ascii=False, indent=1), encoding="utf-8")

    rescued = [r for r in results if r["rescued_compile_run"]]
    agree = [r for r in results if r["stdout_agrees_after_rescue"]]
    print(f"无外部 golden 用例: {len(results)}")
    print(f"补头文件后 clang 恢复可编译可运行: {len(rescued)}")
    print(f"  其中 stdout 与 Vitro 一致（本可成为有效 golden）: {len(agree)}")
    print(f"  其中 stdout 与 Vitro 不一致（真差异被'exemption'吞掉）: {len(rescued)-len(agree)}")
    print(f"补头后仍失败（用例本身非常规 / 双侧都失败）: {len(results)-len(rescued)}")
    for r in results:
        tag = ("RESCUED_MATCH" if r["stdout_agrees_after_rescue"] else
               "RESCUED_DIFF" if r["rescued_compile_run"] else "STILL_FAIL")
        print(f"  {tag:14s} {r['src_dir']:9s} {r['name']:32s} +{','.join(r['added_headers'])}")
    print(f"\nJSON 已写出: {out}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
