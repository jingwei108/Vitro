# -*- coding: utf-8 -*-
"""最小复现：serve 出口的两处 panic（本裁定独立实测发现/确认）。

R-2026-09-A：`seek` 越过程序末尾 → `engine.rs:400` `split_off` panic（新发现，不在既有清单）
R-2026-09-B：`payload.get` 的 `end=-1` → `engine.rs:416` 切片 panic（既有清单 R9b，本次独立复现）

两者都是「一条 JSON 请求杀死整个 serve 会话」，且 serve 循环无 catch_unwind ——
违反路线图 U0 验收标准"一行畸形输入不得到达 panic"。

用法：python scripts/core_asset_verdict/repro_panics.py
输出：repro_panics.json + 控制台（含 stderr 原文）。标注 [动态实测]。
"""
import io
import json
import subprocess
import sys
from pathlib import Path

sys.stdout = io.TextIOWrapper(sys.stdout.buffer, encoding="utf-8", errors="replace")
HERE = Path(__file__).resolve().parent
ROOT = HERE.parent.parent
CLI = ROOT / "native/target/release/vitro_cli.exe"
if not CLI.exists():
    CLI = ROOT / "native/target/debug/vitro_cli.exe"

TINY = '#include <stdio.h>\nint main() { printf("hi"); return 0; }\n'


def run(requests, label):
    payload = "\n".join(json.dumps(r) for r in requests) + "\n"
    p = subprocess.run([str(CLI), "serve"], input=payload, capture_output=True,
                       text=True, encoding="utf-8", errors="replace", timeout=60)
    lines = [l for l in (p.stdout or "").splitlines() if l.strip()]
    return {
        "label": label,
        "requests": len(requests),
        "responses": len(lines),
        "exit_code": p.returncode,
        "panicked": "panicked at" in (p.stderr or ""),
        "panic_line": next((l for l in (p.stderr or "").splitlines() if "panicked at" in l), ""),
        "panic_msg": next((l for l in (p.stderr or "").splitlines()
                           if "should be <= len" in l or "out of range for slice" in l), ""),
        "stderr": (p.stderr or "")[-600:],
        "responses_json": lines,
    }


def main() -> int:
    cases = [
        ("A_seek_past_program_end", [
            {"id": 1, "method": "compile", "params": {"source": TINY}},
            {"id": 2, "method": "step.begin"},
            {"id": 3, "method": "seek", "params": {"step": 50000}},
        ]),
        ("A2_seek_moderate_past_end", [
            {"id": 1, "method": "compile", "params": {"source": TINY}},
            {"id": 2, "method": "step.begin"},
            {"id": 3, "method": "step.next"},
            {"id": 4, "method": "seek", "params": {"step": 3000}},
        ]),
        ("B_payload_get_negative_end", [
            {"id": 1, "method": "compile", "params": {"source": TINY}},
            {"id": 2, "method": "step.begin"},
            {"id": 3, "method": "step.next"},
            {"id": 4, "method": "payload.get", "params": {"start": 0, "end": -1}},
        ]),
        ("C_control_seek_within_range", [
            {"id": 1, "method": "compile", "params": {"source":
                '#include <stdio.h>\nint main(){int s=0;for(int i=0;i<100;i++){s+=i;}\nprintf("%d",s);return 0;}\n'}},
            {"id": 2, "method": "step.begin"},
            {"id": 3, "method": "seek", "params": {"step": 100}},
            {"id": 4, "method": "payload.get", "params": {"start": 0, "end": 10}},
        ]),
    ]
    out = []
    for label, reqs in cases:
        r = run(reqs, label)
        out.append(r)
        print(f"=== {label}")
        print(f"  请求 {r['requests']}  响应 {r['responses']}  exit={r['exit_code']}  "
              f"panicked={r['panicked']}")
        if r["panic_line"]:
            print(f"  {r['panic_line']}")
            print(f"  {r['panic_msg']}")
    p = HERE / "repro_panics.json"
    p.write_text(json.dumps(out, ensure_ascii=False, indent=1), encoding="utf-8")
    print(f"\nJSON 已写出: {p}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
