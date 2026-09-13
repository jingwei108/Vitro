# -*- coding: utf-8 -*-
"""按使用切面的突变测试：测量每类语义"能拦截它的防线数"（margin）。

动机（T2/评测纪律）：662 全绿不能作为质量论据；防线强度必须由**独立于用例作者**的机制测量。
既有突变实测（工作记录20260912_突变测试.md）只覆盖 VM 算术三层。本脚本把突变扩展到
**使用切面维度**，每个切面注入一个语义突变，测两条防线各自的检出：

  防线 A：影子验证（662 例 stdout 批处理）—— 统计分类变化
  防线 B：cargo test --workspace（891 测试，含步级 payload / 诊断 / 可视化断言）—— 统计失败测试名

margin = 检出该突变的独立判定数（影子变化用例数 + 失败测试数）。

流程（每个突变）：校验目标串唯一 → 打补丁 → cargo build --release → 影子 → cargo test
→ `git checkout --` 还原 → 断言 git diff 干净。任何异常都走 finally 还原。
**不改动用例文件、不改预期值**（只改被测引擎源码，且必须还原）。

用法：python scripts/core_asset_verdict/mutation_facet_test.py [--only M1] [--skip-build-baseline]
输出：mutation_facet.json + .mutation_logs/*.log。标注 [动态实测]。
"""
import io
import json
import re
import subprocess
import sys
import time
from pathlib import Path

sys.stdout = io.TextIOWrapper(sys.stdout.buffer, encoding="utf-8", errors="replace")
HERE = Path(__file__).resolve().parent
ROOT = HERE.parent.parent
NATIVE = ROOT / "native"
LOGS = HERE / ".mutation_logs"
LOGS.mkdir(exist_ok=True)

MUTATIONS = [
    {
        "id": "M1-ALGO-LABEL",
        "facet": "教学内容层（算法标注 description / semantic_label）",
        "file": "native/crates/cide_algorithm_steps/src/sorting.rs",
        "old": "            let kth = if n > 0 && i >= 0 && i < n {\n                (i + 1).to_string()",
        "new": "            let kth = if n > 0 && i >= 0 && i < n {\n                (n - i).to_string()",
        "desc": "重引入已修复的 P0-1「冒泡趟数教反」：第 k 趟显示第 n-k 大（埋雷验证）",
    },
    {
        "id": "M2-PTR-STATUS",
        "facet": "可视化层（指针四状态 pointer_snapshots）",
        "file": "native/src/unified/collector.rs",
        "old": "            PointerStatus::Freed",
        "new": "            PointerStatus::Valid",
        "desc": "已释放堆指针显示为 Valid（UAF 教学可视化失真，stdout 不变）",
    },
    {
        "id": "M3-TEMP-SLOT",
        "facet": "codegen 槽位系统（批量正确性）",
        "file": "native/crates/cide_codegen/src/lib.rs",
        "old": ("        let slot = match index {\n"
                "            0 => &mut self.temp_slot0,\n"
                "            1 => &mut self.temp_slot1,\n"
                "            2 => &mut self.temp_slot2,\n"
                "            3 => &mut self.temp_slot3,\n"
                "            _ => &mut self.temp_slot0,\n"
                "        };"),
        "new": "        let _ = index;\n        let slot = &mut self.temp_slot0;",
        "desc": "4 个临时槽塌缩为 1 个（历史 3 起槽位 bug 的病灶形态）",
    },
    {
        "id": "M4-DIAG-TEXT",
        "facet": "诊断内容层（错误文案）",
        "file": "native/crates/cide_typeck/src/expr/var.rs",
        "old": '"未声明的变量 \'{}\'"',
        "new": '"未定义标识符 \'{}\'"',
        "desc": "E3023 诊断文案改写（错误码不变，仅文案）",
    },
    {
        "id": "M5-LEXER-DECIMAL",
        "facet": "词法（十进制整数字面量值）",
        "file": "native/crates/cide_lexer/src/number.rs",
        "old": "            val = text.parse::<u64>().unwrap_or(0);",
        "new": "            val = text.parse::<u64>().map(|v| v.wrapping_add(1)).unwrap_or(0);",
        "desc": "每个十进制整数字面量 +1（词法层语义破坏）",
    },
    {
        "id": "M6-PAYLOAD-LABEL",
        "facet": "步级 payload（semantic_label 与行为的对照）",
        "file": "native/src/unified/collector.rs",
        "old": "        format!(\"第 {} 行\", code_line)",
        "new": "        let _ = code_line;\n        \"第 1 行\".to_string()",
        "desc": "所有步的 semantic_label 恒为「第 1 行」（词汇合法但与行为不符）",
    },
]

SHADOW_RE = {
    "match": re.compile(r"^\s*match:\s*(\d+)", re.M),
    "known_issue": re.compile(r"^\s*known_issue:\s*(\d+)", re.M),
    "cide_better": re.compile(r"^\s*cide_better:\s*(\d+)", re.M),
}
FAILED_TEST_RE = re.compile(r"^test (\S+) \.\.\. FAILED", re.M)
TEST_SUMMARY_RE = re.compile(r"test result: (ok|FAILED)\. (\d+) passed; (\d+) failed")


def run(cmd, cwd, timeout, log_path):
    t0 = time.time()
    p = subprocess.run(cmd, cwd=str(cwd), capture_output=True, text=True,
                       encoding="utf-8", errors="replace", timeout=timeout)
    out = (p.stdout or "") + "\n--- STDERR ---\n" + (p.stderr or "")
    log_path.write_text(out, encoding="utf-8")
    return {"exit": p.returncode, "out": out, "wall_s": round(time.time() - t0, 1)}


def parse_shadow(out: str) -> dict:
    d = {k: (int(rx.search(out).group(1)) if rx.search(out) else None)
         for k, rx in SHADOW_RE.items()}
    d["gate_pass"] = "门禁通过" in out
    d["unexpected"] = (out.count("non-match") if "non-match" in out else None)
    return d


def parse_tests(out: str) -> dict:
    failed = sorted(set(FAILED_TEST_RE.findall(out)))
    passed = failed_n = 0
    for st, p, f in TEST_SUMMARY_RE.findall(out):
        passed += int(p)
        failed_n += int(f)
    return {"failed_tests": failed, "failed_count": failed_n, "passed_count": passed}


def git_clean(rel: str) -> bool:
    p = subprocess.run(["git", "diff", "--quiet", "--", rel], cwd=str(ROOT),
                       capture_output=True)
    return p.returncode == 0


def main() -> int:
    only = None
    if "--only" in sys.argv:
        only = sys.argv[sys.argv.index("--only") + 1]
    muts = [m for m in MUTATIONS if only is None or m["id"] == only]
    results = []

    # 基线（只在需要时重建一次）
    print("== 基线：cargo build --release + 影子 + cargo test ==", flush=True)
    b = run(["cargo", "build", "--release"], NATIVE, 1800, LOGS / "baseline_build.log")
    print(f"  build exit={b['exit']} ({b['wall_s']}s)", flush=True)
    s = run(["go", "run", "./scripts/shadow_verify", "--jobs", "8"],
            ROOT, 900, LOGS / "baseline_shadow.log")
    base_shadow = parse_shadow(s["out"])
    print(f"  影子基线 {base_shadow}", flush=True)
    t = run(["cargo", "test", "--workspace", "--all-features"], NATIVE, 1800,
            LOGS / "baseline_test.log")
    base_tests = parse_tests(t["out"])
    print(f"  测试基线 passed={base_tests['passed_count']} failed={base_tests['failed_count']}",
          flush=True)

    for m in muts:
        fpath = ROOT / m["file"]
        rel = m["file"]
        rec = {"id": m["id"], "facet": m["facet"], "desc": m["desc"], "file": rel}
        print(f"\n===== {m['id']} :: {m['facet']}", flush=True)
        try:
            if not git_clean(rel):
                rec["error"] = "目标文件在打补丁前不干净（拒绝执行）"
                print("  " + rec["error"], flush=True)
                results.append(rec)
                continue
            text = fpath.read_text(encoding="utf-8")
            cnt = text.count(m["old"])
            if cnt != 1:
                rec["error"] = f"目标串出现 {cnt} 次（要求恰好 1 次）"
                print("  " + rec["error"], flush=True)
                results.append(rec)
                continue
            fpath.write_text(text.replace(m["old"], m["new"]), encoding="utf-8")
            rec["patch_applied"] = True

            b = run(["cargo", "build", "--release"], NATIVE, 1800, LOGS / f"{m['id']}_build.log")
            rec["build_exit"] = b["exit"]
            rec["build_wall_s"] = b["wall_s"]
            print(f"  build exit={b['exit']} ({b['wall_s']}s)", flush=True)
            if b["exit"] != 0:
                rec["verdict"] = "build_failed（突变不可编译，不计入 margin）"
                continue

            s = run(["go", "run", "./scripts/shadow_verify",
                     "--jobs", "8"], ROOT, 900, LOGS / f"{m['id']}_shadow.log")
            sh = parse_shadow(s["out"])
            rec["shadow"] = sh
            rec["shadow_exit"] = s["exit"]
            print(f"  影子: {sh} exit={s['exit']}", flush=True)

            t = run(["cargo", "test", "--workspace", "--all-features"], NATIVE, 1800,
                    LOGS / f"{m['id']}_test.log")
            tt = parse_tests(t["out"])
            rec["tests"] = tt
            rec["test_exit"] = t["exit"]
            print(f"  测试: failed={tt['failed_count']} -> {tt['failed_tests'][:8]}", flush=True)

            # margin 计算
            shadow_changed = None
            if all(sh.get(k) is not None for k in ("match", "known_issue", "cide_better")):
                shadow_changed = abs((sh["match"] or 0) - (base_shadow["match"] or 0)) + \
                                 abs((sh["known_issue"] or 0) - (base_shadow["known_issue"] or 0)) + \
                                 abs((sh["cide_better"] or 0) - (base_shadow["cide_better"] or 0))
            rec["shadow_cases_changed"] = shadow_changed
            rec["margin"] = (shadow_changed or 0) + tt["failed_count"]
            rec["detected"] = rec["margin"] > 0
            rec["verdict"] = ("检出" if rec["detected"] else "**零检出（该语义无防线）**")
            print(f"  => margin={rec['margin']}（影子变化 {shadow_changed} + 测试失败 "
                  f"{tt['failed_count']}） {rec['verdict']}", flush=True)
        except subprocess.TimeoutExpired as e:
            rec["error"] = f"超时: {e}"
            print("  " + rec["error"], flush=True)
        finally:
            subprocess.run(["git", "checkout", "--", rel], cwd=str(ROOT), capture_output=True)
            rec["reverted_clean"] = git_clean(rel)
            print(f"  还原完成，git clean={rec['reverted_clean']}", flush=True)
            results.append(rec)

    # 最终：还原后重建并复跑，证明修复与防线回到基线
    print("\n== 还原验证：重建 + 影子复跑 ==", flush=True)
    b = run(["cargo", "build", "--release"], NATIVE, 1800, LOGS / "restore_build.log")
    s = run(["go", "run", "./scripts/shadow_verify", "--jobs", "8"],
            ROOT, 900, LOGS / "restore_shadow.log")
    restore = {"build_exit": b["exit"], "shadow": parse_shadow(s["out"]), "shadow_exit": s["exit"]}
    print(f"  restore {restore}", flush=True)
    all_clean = all(r.get("reverted_clean", True) for r in results)
    print(f"  git 全目标文件干净: {all_clean}", flush=True)

    out = {"baseline_shadow": base_shadow, "baseline_tests": base_tests,
           "mutations": results, "restore": restore, "all_files_reverted": all_clean}
    p = HERE / "mutation_facet.json"
    p.write_text(json.dumps(out, ensure_ascii=False, indent=1), encoding="utf-8")
    print(f"\nJSON 已写出: {p}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
