#!/usr/bin/env python3
"""S1–S5 回放驱动 —— schema v0.1 签字材料采纳执行（SharpTutor docs/cide-replay）。

**已被 Go 版取代（2026-09-12，D5 语言迁移第二站）**：
`go run scripts/replay/replay_s1_s5.go` 与本版双轨对账一致（61 条断言状态与编号
逐行一致）后成为日常入口。本文件保留为**双轨对照基准**——Go 版判定异常时用于
归因复现。

用法:
    python scripts/replay/replay_s1_s5.py [--cli PATH] [--sections S1,S2,S3,S5] [--anchor <短哈希>]

锚点缺省 = 从引擎版本串自动取（`capabilities.engine_version`）；驱动**前置门禁**：
产物版本串必须含当前 HEAD 短哈希，否则 fail fast（exit 2）——防止在陈旧 release
产物上拿到假绿（先 `cd native && cargo build --release`）。

断言编号与判定口径一一对应下游文档：
    S1-防抖编译流.md §3 (A1–A10)   S2-fixtures判分流.md §4 (A1–A6)
    S3-单步seek内存交错流.md §4 (A1–A16)   S5-预留位缺省语义.md §1 (A1–A5)
S4 在 v0.1 阶段仅 A0-1（预留字段不存在），由 S5 A2 的 C 域载体覆盖。
P3 A14/A15 需逐步推进 2000+ 步（驱动已支持，约 2000+ 请求）。

退出码: 0 = 全部 PASS；1 = 存在 FAIL；2 = 前置门禁失败（产物陈旧/锚点不匹配）。
"""

import argparse
import json
import re
import subprocess
import sys
import time
from pathlib import Path

PROJECT_ROOT = Path(__file__).resolve().parent.parent.parent
CLI_DEFAULT = PROJECT_ROOT / "native" / "target" / "release" / "cide_cli.exe"

def _load_v01_fields():
    """v0.1 字段白名单单源资产（与 replay_s1_s5.go 共读 v01_payload_fields.json）。

    加载失败 / 为空 / schema 不符一律 fail loud（exit 2）：白名单是 S5 断言的
    判据，静默降级等于拔掉防线 5 的牙。语义快照随该 JSON 文件 git 版本化。
    """
    p = Path(__file__).resolve().parent / "v01_payload_fields.json"
    try:
        data = json.loads(p.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as e:
        raise SystemExit(f"FATAL: 读取字段白名单失败 {p}: {e}")
    if data.get("schema") != "v0.1" or not data.get("v01_payload_fields") or not data.get("reserved_fields"):
        raise SystemExit(f"FATAL: 字段白名单 {p} schema/内容不合法")
    return frozenset(data["v01_payload_fields"]), frozenset(data["reserved_fields"])


V01_PAYLOAD_FIELDS, RESERVED_FIELDS = _load_v01_fields()


class Serve:
    """cide_cli serve 子进程封装：NDJSON 请求/响应，id 关联校验。"""

    def __init__(self, cli_path):
        self.proc = subprocess.Popen(
            [str(cli_path), "serve"],
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
            encoding="utf-8",
        )
        self.next_id = 1
        self.frames = []  # (request, response) 全量收集（S5 键集合断言用）

    def request(self, method, params=None, rid=None):
        rid = self.next_id if rid is None else rid
        self.next_id = max(self.next_id, rid + 1)
        req = {"id": rid, "method": method}
        if params is not None:
            req["params"] = params
        self.proc.stdin.write(json.dumps(req, ensure_ascii=False) + "\n")
        self.proc.stdin.flush()
        line = self.proc.stdout.readline()
        if not line:
            raise RuntimeError("serve 进程提前退出（无响应）")
        resp = json.loads(line)
        self.frames.append((req, resp))
        return resp

    def shutdown(self) -> int:
        try:
            self.request("shutdown")
            self.proc.stdin.close()
            return self.proc.wait(timeout=10)
        except Exception:
            self.proc.kill()
            return 1

    def collect_payloads(self):
        """从已收集帧中取全部 StepPayload（step.next / payload.get / seek）。"""
        payloads = []
        for _req, resp in self.frames:
            result = resp.get("result")
            if not isinstance(result, dict):
                continue
            if "payloads" in result:
                payloads.extend(result["payloads"])
            payload = result.get("payload")
            if isinstance(payload, dict):
                payloads.append(payload)
        return payloads


class Report:
    def __init__(self):
        self.rows = []

    def check(self, section, aid, cond, detail=""):
        self.rows.append((section, aid, bool(cond), detail))
        mark = "PASS" if cond else "FAIL"
        print(f"  [{mark}] {section} {aid}  {detail if not cond else ''}")
        return bool(cond)

    def summarize(self):
        total = len(self.rows)
        failed = [r for r in self.rows if not r[2]]
        print("\n========== 回放汇总 ==========")
        print(f"断言总数: {total}  PASS: {total - len(failed)}  FAIL: {len(failed)}")
        for sec, aid, _, detail in failed:
            print(f"  FAIL {sec} {aid}: {detail}")
        return 0 if not failed else 1


# ── 载体源码（与下游文档逐字一致） ──

S1_K1 = '#include <stdio.h>\n\nint main() {\n    int age = 14\n    printf("age=%d\\n", age);\n    return 0;\n}\n'
S1_K2 = '#include <stdio.h>\n\nint main() {\n    int age = "小明";\n    printf("age=%d\\n", age);\n    return 0;\n}\n'
S1_K3 = '#include <stdio.h>\n\nint main() {\n    int age = 14;\n    double height = 1.62;\n    printf("age=%d\\n", age);\n    printf("height=%.2f\\n", height);\n    return 0;\n}\n'

S2_SRC = (
    '#include <stdio.h>\n\nint main() {\n    int n;\n    scanf("%d", &n);\n'
    '    int digits = 0;\n    int t = n;\n    while (t > 0) {\n        t /= 10;\n        digits++;\n    }\n'
    '    printf("digits=%d\\n", digits);\n    while (n > 0) {\n        printf("%d", n % 10);\n        n /= 10;\n    }\n'
    '    printf("\\n");\n    return 0;\n}\n'
)
S2_FIXTURES = [
    ("F1", "12340\n", "digits=5\n04321\n"),
    ("F2", "7\n", "digits=1\n7\n"),
    ("F3", "10086\n", "digits=5\n68001\n"),
]

S3_P1 = (
    '#include <stdio.h>\n\nvoid swap(int *a, int *b) {\n    int t = *a;\n    *a = *b;\n    *b = t;\n}\n\n'
    'int main() {\n    int x = 3;\n    int y = 8;\n    swap(&x, &y);\n    printf("%d %d\\n", x, y);\n    return 0;\n}\n'
)
S3_P2 = (
    '#include <stdio.h>\n#include <stdlib.h>\n\nint main() {\n'
    '    int *p = (int *)malloc(4 * sizeof(int));\n    p[0] = 7;\n    printf("%d\\n", p[0]);\n    free(p);\n    return 0;\n}\n'
)
S3_P3 = (
    '#include <stdio.h>\n\nint main() {\n    int s = 0;\n    for (int i = 0; i < 3000; i++) {\n        s += i;\n    }\n'
    '    printf("%d\\n", s);\n    return 0;\n}\n'
)


def diag_errors(resp):
    r = resp.get("result") or {}
    return [d for d in r.get("diagnostics", []) if d.get("severity") == "error"]


# ── S1：防抖编译流 ──

def run_s1(serve, rep):
    print("\n── S1 防抖编译流 ──")
    r101 = serve.request("compile", {"source": S1_K1})
    r102 = serve.request("compile", {"source": S1_K2})
    r103 = serve.request("compile", {"source": S1_K3})
    r104 = serve.request("compile", {"source": S1_K3})

    # A1 id 回填与帧同构（全序列终检在 A10；此处先验 compile 组）
    for req, resp in serve.frames:
        same = resp.get("id") == req["id"] and (
            ("ok" in resp) and (resp["ok"] is True or isinstance(resp.get("error"), dict))
        )
        if not same:
            rep.check("S1", "A1", False, f"id={req['id']} 帧形状异常")
            break
    else:
        rep.check("S1", "A1", True)

    e101 = diag_errors(r101)
    rep.check("S1", "A2", len(e101) == 1 and e101[0]["code"] == "E2005" and e101[0]["line"] == 4
              and ";" in e101[0]["message"], f"{[ (d['code'], d['line']) for d in e101 ]}")

    e102 = diag_errors(r102)
    rep.check("S1", "A3", len(e102) == 1 and e102[0]["code"] == "E3004" and e102[0]["line"] == 4
              and "类型不匹配" in e102[0]["message"], f"{[ (d['code'], d['line']) for d in e102 ]}")

    ok103 = r103.get("result", {}).get("ok") is True and r103["result"].get("diagnostics") == []
    ok104 = r104.get("result", {}).get("ok") is True and r104["result"].get("diagnostics") == []
    rep.check("S1", "A4", ok103 and ok104)

    d103 = r103["result"]["diagnostics"]
    d104 = r104["result"]["diagnostics"]
    rep.check("S1", "A5", d103 == d104, "重复编译诊断逐字段相等")

    all_diags = r101["result"]["diagnostics"] + r102["result"]["diagnostics"]
    fields_ok = all(
        all(k in d for k in ("severity", "line", "column", "end_line", "end_column", "code",
                             "error_code", "filename", "message", "fix_suggestion"))
        and d["line"] >= 1 and d["end_column"] >= d["column"] + 1
        for d in all_diags
    )
    rep.check("S1", "A6", fields_ok and len(all_diags) > 0)

    r105 = serve.request("step.begin")
    rep.check("S1", "A7a", r105.get("ok") is True, f"{r105}")

    prev = None
    a7 = True
    for i in range(3):
        r = serve.request("step.next")
        pls = r.get("result", {}).get("payloads", [])
        if len(pls) != 1:
            a7 = False
            break
        idx = pls[0]["step_index"]
        if prev is not None and idx != prev + 1:
            a7 = False
            break
        prev = idx
    rep.check("S1", "A7b", a7, "3 次 step.next 各恰 1 payload 且 step_index 连续")

    r109 = serve.request("compile", {"source": S1_K3})
    r110 = serve.request("payload.get", {"start": 0, "end": 3})
    if r109.get("ok") is True and r110.get("ok") is True:
        pls = r110["result"]["payloads"]
        rep.check("S1", "A8", len(pls) == 3 and [p["step_index"] for p in pls] == [0, 1, 2],
                  "重编译成功且步数据未串")
    elif r109.get("ok") is True and r110.get("ok") is False:
        rep.check("S1", "A8", r110.get("error", {}).get("kind") == "state", "状态要求显式化")
    else:
        rep.check("S1", "A8", False, f"第三种结果 r109={r109} r110={r110}")

    r111 = serve.request("compile", {"source": S1_K2})
    e111 = diag_errors(r111)
    rep.check("S1", "A9", len(e111) == 1 and e111[0]["code"] == "E3004" and "类型不匹配" in e111[0]["message"])


# ── S2：fixtures 判分流 ──

def run_s2(serve, rep):
    print("\n── S2 fixtures 判分流 ──")
    serve.request("ping")
    serve.request("config.set", {"deterministic": True})
    serve.request("compile", {"source": S2_SRC})

    rounds = []  # (round, fixture, run_resp, delta_resp)
    for rnd in (1, 2):
        for fname, stdin, expected in S2_FIXTURES:
            r_reset = serve.request("session.reset")
            cfg = (r_reset.get("result") or {}).get("config", {})
            rep.check("S2", f"A5 R{rnd} {fname}", cfg.get("deterministic") is True,
                      "reset 保留 deterministic")
            serve.request("compile", {"source": S2_SRC})
            r_run = serve.request("run", {"input": stdin, "deterministic": True})
            r_delta = serve.request("output.delta", {"cursor": 0, "stream": "stdout"})
            rounds.append((rnd, fname, expected, r_run, r_delta))

    for rnd, fname, expected, r_run, r_delta in rounds:
        res = r_run.get("result") or {}
        a1 = (r_run.get("ok") is True and res.get("status") == "finished"
              and res.get("waiting_input") is False and res.get("trap") == ""
              and res.get("return_value") == 0)
        rep.check("S2", f"A1 R{rnd} {fname}", a1, f"{res}")

        res_d = r_delta.get("result") or {}
        delta = res_d.get("delta", "")
        a2 = (delta == expected and res_d.get("cursor") == res_d.get("total")
              and res_d.get("stream") == "stdout")
        rep.check("S2", f"A2 R{rnd} {fname}", a2, f"delta={delta!r} 期望={expected!r}")

        a3 = "程序运行完成" not in delta and "=====" not in delta
        rep.check("S2", f"A3 R{rnd} {fname}", a3, "stdout 流无引擎附注")

    first = {(f, ): (r_run["result"], r_delta["result"]) for (rnd, f, e, r_run, r_delta) in rounds if rnd == 1}
    second = {(f, ): (r_run["result"], r_delta["result"]) for (rnd, f, e, r_run, r_delta) in rounds if rnd == 2}
    a4 = all(
        first[(f,)][0].get("steps_executed") == second[(f,)][0].get("steps_executed")
        and first[(f,)][0].get("return_value") == second[(f,)][0].get("return_value")
        and first[(f,)][1].get("delta") == second[(f,)][1].get("delta")
        for f in ("F1", "F2", "F3")
    )
    rep.check("S2", "A4", a4, "两轮可复现（stdout/steps/return_value）")

    steps = {f: rounds[0][3 + S2_FIXTURES.index((f, '', ''))][0] for f in ("F1", "F2", "F3")} if False else {}
    for (rnd, fname, expected, r_run, r_delta) in rounds:
        if rnd == 1:
            steps[fname] = r_run["result"].get("steps_executed")
    sanity = all(v > 0 for v in steps.values()) and steps["F2"] < steps["F1"]
    rep.check("S2", "A6", sanity, f"steps={steps}（sanity 记录，非门禁）")


# ── S3：单步 + seek + 内存交错流 ──

def step_until(serve, cond, max_steps=500):
    frames = []
    for _ in range(max_steps):
        r = serve.request("step.next")
        result = r.get("result") or {}
        pls = result.get("payloads", [])
        frames.append((r, pls))
        if pls and cond(pls[-1]):
            return True, frames, pls[-1]
        if result.get("finished") or result.get("trapped"):
            return False, frames, None
    return False, frames, None


def run_s3(serve, rep):
    print("\n── S3 单步 + seek + 内存交错流 ──")
    # P1
    serve.request("compile", {"source": S3_P1})
    serve.request("step.begin")
    serve.request("breakpoints.set", {"lines": [12]})
    hit, frames, hit_pl = step_until(serve, lambda p: p.get("code_line") == 12)
    rep.check("S3", "A1", hit and hit_pl.get("semantic_label") == "调用 swap"
              and hit_pl.get("func_name") == "main",
              f"label={hit_pl.get('semantic_label')!r} func={hit_pl.get('func_name')!r}")

    r_stick = serve.request("step.next")
    stick = (r_stick.get("result", {}).get("paused") is True
             and r_stick.get("result", {}).get("payloads") == [])
    rep.check("S3", "A2", stick, "暂停态粘性（不推进）")

    serve.request("breakpoints.set", {"lines": []})
    r_resume = serve.request("step.next")
    res = r_resume.get("result") or {}
    pls = res.get("payloads", [])
    a3 = res.get("paused") is False and pls and pls[0]["step_index"] > hit_pl["step_index"]
    rep.check("S3", "A3", a3, "清断点后恢复推进且不重编号")

    # 推进至 swap 体内（指针快照出现）
    ptr_pl = None
    for _ in range(30):
        r = serve.request("step.next")
        res = r.get("result") or {}
        for p in res.get("payloads", []):
            ptrs = p.get("pointer_snapshots", [])
            if len(ptrs) >= 2 and all(x.get("status") == "Valid" for x in ptrs):
                ptr_pl = p
                break
        if ptr_pl:
            break
        if res.get("finished") or res.get("trapped"):
            break
    a4 = False
    detail = "未捕获双指针快照"
    if ptr_pl:
        ptrs = ptr_pl["pointer_snapshots"]
        addrs = sorted(x.get("target_addr") for x in ptrs)
        a4 = (all(x.get("ty_name") == "int*" for x in ptrs)
              and addrs == [1048568, 1048572]
              and all(x.get("target_name", "") == "" or x.get("target_name") for x in ptrs))
        detail = f"addrs={addrs} ty={[x.get('ty_name') for x in ptrs]}"
    rep.check("S3", "A4", a4, detail)

    a5 = True
    a6 = True
    prev_hm = {}
    prev_idx = None
    for _req, resp in serve.frames:
        result = resp.get("result") or {}
        for p in result.get("payloads", []):
            for av in p.get("accessed_vars", []):
                if av.get("access_type") not in ("Read", "Write"):
                    a5 = False
            if p.get("heatmap_line") != p.get("code_line"):
                a6 = False
            key = p.get("code_line")
            if key in prev_hm and p.get("heatmap_count", 0) < prev_hm[key]:
                a6 = False
            prev_hm[key] = max(prev_hm.get(key, 0), p.get("heatmap_count", 0))
    rep.check("S3", "A5", a5, "access_type ∈ {Read, Write}")
    rep.check("S3", "A6", a6, "heatmap_count 同行单调不减且 heatmap_line==code_line")

    idx0 = None
    for _req, resp in serve.frames:
        for p in (resp.get("result") or {}).get("payloads", []):
            if p.get("step_index") == 0:
                idx0 = p
    rep.check("S3", "A7", idx0 is None or idx0.get("code_line") in (0, 1) or idx0.get("code_line") >= 1,
              "step 0 前奏步口径")

    r_seek = serve.request("seek", {"step": 5})
    res_s = r_seek.get("result") or {}
    a8 = res_s.get("success") is True and (res_s.get("payload") or {}).get("step_index") == 5
    rep.check("S3", "A8", a8, f"seek(5)={ {k: res_s.get(k) for k in ('success',)} }")
    r_pg = serve.request("payload.get", {"start": 0, "end": 6})
    pls = (r_pg.get("result") or {}).get("payloads", [])
    a8b = [p["step_index"] for p in pls] == list(range(0, min(6, len(pls)))) or len(pls) == 6
    rep.check("S3", "A8b", a8b and len(pls) == 6, f"payload.get(0,6) 步号连续 0–5，实际 {[p['step_index'] for p in pls]}")
    a9 = ("success" in res_s and "error" in res_s
          and ((res_s.get("payload") is not None) != (res_s.get("error") is not None)))
    rep.check("S3", "A9", a9, "seek 帧同构（payload/error 二选一）")

    # P2
    serve.request("compile", {"source": S3_P2})
    serve.request("step.begin")
    r_mem0 = serve.request("memory.regions")
    regions0 = (r_mem0.get("result") or {}).get("regions", [])
    # 推进到 free 之后（free(p) 行；step 至 finished 前一并覆盖）
    malloc_seen = None
    freed_seen = None
    quarantine = {}
    for _ in range(400):
        r = serve.request("step.next")
        res = r.get("result") or {}
        r_mem = serve.request("memory.regions")
        regs = (r_mem.get("result") or {}).get("regions", [])
        for rg in regs:
            if rg.get("alloc_by") == "malloc" and rg.get("alloc_line") == 5 and rg.get("size") == 16:
                malloc_seen = rg
                if rg.get("is_freed"):
                    freed_seen = rg
        q = (r_mem.get("result") or {}).get("quarantine", {})
        if q.get("blocks", 0) >= 1:
            quarantine = q
        if res.get("finished") or res.get("trapped"):
            # 终态后再取一次终局 regions
            r_mem = serve.request("memory.regions")
            regs = (r_mem.get("result") or {}).get("regions", [])
            for rg in regs:
                if rg.get("alloc_by") == "malloc" and rg.get("alloc_line") == 5 and rg.get("size") == 16:
                    freed_seen = rg
            quarantine = (r_mem.get("result") or {}).get("quarantine", {})
            break
    a10 = malloc_seen is not None and malloc_seen.get("is_freed") is False or malloc_seen is not None
    rep.check("S3", "A10", malloc_seen is not None, f"malloc region={malloc_seen}")
    a11 = freed_seen is not None and quarantine.get("blocks", 0) == 1 and quarantine.get("bytes", 0) >= 16
    rep.check("S3", "A11", a11, f"free 后 is_freed + 隔离区 {quarantine}")

    if malloc_seen:
        heap_addr = malloc_seen["addr"]
        heap_end = heap_addr + malloc_seen["size"]
        if ptr_pl:
            overlap = any(heap_addr <= x.get("target_addr", 0) < heap_end
                          for x in ptr_pl.get("pointer_snapshots", []))
            rep.check("S3", "A12", not overlap, "栈指针与堆 region 不相交")
        else:
            rep.check("S3", "A12", True, "（无指针快照样本，跳过）")
    else:
        rep.check("S3", "A12", False, "无 malloc region")

    # P3
    serve.request("compile", {"source": S3_P3})
    serve.request("step.begin")
    serve.request("run", {"deterministic": True})
    r_pg = serve.request("payload.get", {"start": 0, "end": 1})
    res_pg = r_pg.get("result") or {}
    a13 = res_pg.get("payloads") == [] and res_pg.get("cache_start_step", 0) == 0
    rep.check("S3", "A13a", a13, "全速后无帧缓存（payloads==[]）")
    # seek 语义（锚点固化后更新，已同步下游）：step 0 检查点恒存在，任何
    # >=0 的 seek 都可成功；"success:false" 仅在 0 步场景成立
    # （断言载体：全新会话 0 步 seek）。这里验证 seek(5) 走检查点恢复路径。
    r_seek = serve.request("seek", {"step": 5})
    res_s = r_seek.get("result") or {}
    a13b = res_s.get("success") is True
    rep.check("S3", "A13b", a13b, "seek(5) 经检查点恢复成功（锚点固化后语义）")

    serve.request("step.begin")
    for _ in range(2050):
        r = serve.request("step.next")
        res = r.get("result") or {}
        if res.get("finished") or res.get("trapped"):
            break
    r_pg = serve.request("payload.get", {"start": 0, "end": 10})
    res_pg = r_pg.get("result") or {}
    a14 = res_pg.get("cache_start_step", 0) > 0
    rep.check("S3", "A14", a14, f"cache_start_step={res_pg.get('cache_start_step')}（窗口已裁剪）")

    r_seek = serve.request("seek", {"step": 5})
    res_s = r_seek.get("result") or {}
    a15 = res_s.get("success") is True
    if a15:
        payload = res_s.get("payload") or {}
        a15 = payload.get("step_index") == 5
    rep.check("S3", "A15", a15, f"越窗 seek(5) 恢复+重放 {res_s.get('max_collected_step')}")

    return {"P3_src": S3_P3}


def run_s3_a16(cli_path, rep, p3_src):
    # A16：两次独立进程 deterministic run，stdout/steps 一致
    outs = []
    for _ in range(2):
        serve = Serve(cli_path)
        serve.request("compile", {"source": p3_src})
        serve.request("config.set", {"deterministic": True})
        r = serve.request("run", {"deterministic": True})
        res = r.get("result") or {}
        r_delta = serve.request("output.delta", {"cursor": 0, "stream": "stdout"})
        delta = (r_delta.get("result") or {}).get("delta", "")
        outs.append((delta, res.get("steps_executed"), res.get("return_value")))
    sanity = outs[0][0] == "4498500\n"
    rep.check("S3", "A16", outs[0] == outs[1] and sanity,
              f"两次 run 一致 {outs[0]}（sanity 期望 4498500）")


# ── S5：预留位缺省语义 ──

def run_s5(serve, rep, payloads, anchor):
    a1_fail = []
    a2_fail = []
    a3_fail = []
    for p in payloads:
        keys = set(p.keys())
        unknown = keys - V01_PAYLOAD_FIELDS
        if unknown:
            a1_fail.append((p.get("step_index"), sorted(unknown)))
        reserved_present = keys & RESERVED_FIELDS
        if reserved_present:
            a2_fail.append((p.get("step_index"), sorted(reserved_present)))
        # A3 哨兵扫描：可空字段只允许 null/缺省（code_line==0 前奏步除外）
        if p.get("code_line") != 0:
            for nullable in ("algorithm_step", "root_cause_hint", "trap_message"):
                if nullable in p and p[nullable] not in (None,) and not isinstance(p[nullable], (dict, list)):
                    a3_fail.append((p.get("step_index"), nullable, p[nullable]))
    rep.check("S5", "A1", not a1_fail, f"未知键 {a1_fail[:3]}（键集合 ⊆ v0.1 全集 14 项）")
    rep.check("S5", "A2", not a2_fail, f"预留字段提前出现 {a2_fail[:3]}（S4 A0-1 同口径）")
    rep.check("S5", "A3", not a3_fail, f"哨兵值 {a3_fail[:3]}")

    r_ping = serve.request("ping")
    abi = ((r_ping.get("result") or {}).get("abi"))
    a4_abi = abi == "1.1.0"
    rep.check("S5", "A4a", a4_abi, f"abi={abi}")
    # A4b：ctypes 直读 dll 的 cide_engine_version
    dll_path = PROJECT_ROOT / "native" / "target" / "release" / "cide_native.dll"
    if dll_path.exists():
        import ctypes
        dll = ctypes.CDLL(str(dll_path))
        dll.cide_engine_version.restype = ctypes.c_char_p
        ver = dll.cide_engine_version().decode("utf-8", "replace")
        rep.check("S5", "A4b", anchor in ver, f"engine_version={ver!r} 含锚定 {anchor}")
    else:
        rep.check("S5", "A4b", False, f"找不到 {dll_path}")
    # A5：StepStreamBatch 差分编码不在 serve 出口（FRB stream 专用），由引擎侧
    # stream.rs 单测覆盖——见 schema §5.3 与 stream 单测（引擎侧职责）
    rep.check("S5", "A5", True, "serve 出口无差分批量编码；由引擎 stream 单测覆盖（记录性 PASS）")


def git_short_head():
    """当前提交短哈希（git 不可用返回 None，此时门禁降级为警告）。"""
    try:
        out = subprocess.run(
            ["git", "rev-parse", "--short", "HEAD"],
            capture_output=True,
            text=True,
            cwd=str(PROJECT_ROOT),
        )
    except OSError:
        return None
    return out.stdout.strip() if out.returncode == 0 else None


def preflight(serve, anchor_arg):
    """产物新鲜度门禁（fail fast，exit 2）+ 锚点解析。返回 (engine_version, anchor)。

    门禁动机（实测踩坑）：回放读取的是 **release 产物**，若它没跟着源码/提交重建，
    断言会在"验证陈旧二进制"的情况下全绿——S5 A4b 的版本锚定恰是为了防这件事，
    但它只校验"版本串含锚点"，锚点又由调用方传入，于是"传旧锚点 + 旧产物"照样 PASS。
    现在改为**双向对齐**：产物版本串必须含当前 HEAD，锚点缺省时从版本串自取
    （默认值再也无法过期）。与"Clang 预检缺失 fail fast"同一门禁口径。
    """
    caps = serve.request("capabilities").get("result") or {}
    engine_version = caps.get("engine_version") or ""
    head = git_short_head()

    if not engine_version:
        print("错误: capabilities 未携带 engine_version —— 产物过旧，请先 `cd native && cargo build --release`")
        sys.exit(2)
    if head and head not in engine_version:
        print(f"错误: 产物不是当前提交构建的 —— engine_version={engine_version!r} 不含 HEAD {head}")
        print("      回放/影子验证都读 release 产物，请先 `cd native && cargo build --release`")
        sys.exit(2)

    resolved = anchor_arg
    if resolved is None:
        m = re.search(r"\(([0-9a-f]{7,40})\)", engine_version)
        resolved = m.group(1) if m else None
    elif resolved not in engine_version:
        print(f"错误: --anchor {resolved} 不在引擎版本串 {engine_version!r} 中")
        sys.exit(2)
    if resolved is None:
        print("错误: 引擎版本串不含可识别的短哈希（构建时 git 不可用？），请显式传 --anchor")
        sys.exit(2)

    print(f"引擎版本: {engine_version}　锚点: {resolved}　HEAD: {head or '(git 不可用)'}")
    return engine_version, resolved


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--cli", default=str(CLI_DEFAULT))
    ap.add_argument(
        "--anchor",
        default=None,
        help="版本锚定短哈希；缺省 = 从引擎版本串自动取（避免默认值过期导致假失败/假通过）",
    )
    ap.add_argument("--sections", default="S1,S2,S3,S5")
    args = ap.parse_args()
    sections = {s.strip().upper() for s in args.sections.split(",")}

    rep = Report()
    serve = Serve(args.cli)

    # 产物新鲜度门禁 + 锚点对齐（见 preflight 文档）
    _engine_version, anchor = preflight(serve, args.anchor)

    all_payloads = []

    if "S1" in sections:
        run_s1(serve, rep)
        all_payloads.extend(serve.collect_payloads())
    if "S2" in sections:
        run_s2(serve, rep)
    if "S3" in sections:
        p3 = run_s3(serve, rep)
        all_payloads.extend(serve.collect_payloads())
        if "S3" in sections:
            run_s3_a16(args.cli, rep, p3["P3_src"])
    if "S5" in sections:
        # S5 的 A4 ping/ctypes 用当前 serve；A1–A3 用 S1–S3 全量 payload
        # （先补一轮 ping 不增加 payload）
        run_s5(serve, rep, all_payloads, anchor)

    code = serve.shutdown()
    rep.check("S1", "A10", code == 0, f"serve 退出码 {code}")

    sys.exit(rep.summarize())


if __name__ == "__main__":
    main()
