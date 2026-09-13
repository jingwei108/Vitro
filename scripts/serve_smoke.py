# -*- coding: utf-8 -*-
"""`cide_cli serve`（JSON-lines 会话模式）端到端冒烟 + 协议断言。

防线定位：出口 3 的协议契约验证 —— id 关联 / 错误帧同构 / session.reset /
与 capi 共用入口语义（StepPayload 字段、隔离预算默认值等）。

运行：`python scripts/serve_smoke.py`（需先构建 cide_cli：`cargo build --bin cide_cli`）
或经环境变量指定可执行文件：`CIDE_CLI=/path/to/cide_cli python scripts/serve_smoke.py`
"""
import json
import os
import subprocess
import io
import sys
# Windows CI 控制台默认 cp1252，编码不了中文/✓ 等字符（UnicodeEncodeError）。
# 与 shadow_verify.py 的入口处理一致：显式把 stdout/stderr 重包为 UTF-8。
sys.stdout = io.TextIOWrapper(sys.stdout.buffer, encoding="utf-8", errors="replace")
sys.stderr = io.TextIOWrapper(sys.stderr.buffer, encoding="utf-8", errors="replace")

from pathlib import Path

PROJECT_ROOT = Path(__file__).resolve().parent.parent


def resolve_exe() -> Path:
    override = os.environ.get("CIDE_CLI")
    if override:
        return Path(override)
    name = "cide_cli.exe" if sys.platform == "win32" else "cide_cli"
    debug = PROJECT_ROOT / "native" / "target" / "debug" / name
    release = PROJECT_ROOT / "native" / "target" / "release" / name
    # 取 mtime 较新的产物——固定 debug 优先会在陈旧 debug 上拿假绿
    # （J9 埋雷实测踩中：埋雷注入 release 后 smoke 仍跑旧 debug 全绿）
    candidates = [p for p in (debug, release) if p.exists()]
    if not candidates:
        return release
    return max(candidates, key=lambda p: p.stat().st_mtime)


PROGRAM = (
    '#include <stdio.h>\n'
    'int main(){ int a = 1; int b = 2; printf("%d", a + b); return 0; }\n'
)

REQUESTS = [
    {"id": 1, "method": "ping"},
    {"id": 2, "method": "compile", "params": {"source": PROGRAM}},
    {"id": 3, "method": "run"},
    {"id": 4, "method": "output.delta", "params": {"cursor": 0}},
    {"id": 5, "method": "step.begin"},
    {"id": 6, "method": "step.next"},
    {"id": 7, "method": "step.next"},
    {"id": 8, "method": "payload.get", "params": {"start": 0, "end": 50}},
    {"id": 9, "method": "breakpoints.set", "params": {"lines": [3]}},
    {"id": 10, "method": "seek", "params": {"step": 1}},
    {"id": 11, "method": "memory.regions"},
    {"id": 12, "method": "config.set", "params": {"quarantine_budget": 0}},
    {"id": 13, "method": "compile", "params": {"source": "int main(){ int x = ; }"}},
    {"id": 14, "method": "no.such.method"},
    {"id": 15, "method": "session.reset"},
    {"id": 16, "method": "capabilities"},
    {"id": 17, "method": "semantic_labels"},
    {"id": 18, "method": "contracts"},
    {"id": 19, "method": "session.create"},
    {"id": 20, "method": "shutdown"},
]

DEFAULT_QUARANTINE_BUDGET = 256 * 1024  # 1MB 堆上限的 1/4（堆决议 §1）

failures = []


def check(cond, label, detail=""):
    if cond:
        print(f"  PASS  {label}")
    else:
        print(f"  FAIL  {label}  {detail}")
        failures.append(label)


def main():
    exe = resolve_exe()
    if not exe.exists():
        print(f"错误: 找不到 {exe}，请先 `cd native && cargo build --bin cide_cli`")
        return 2
    print(f"cide_cli: {exe}")

    payload = "\n".join(json.dumps(r, ensure_ascii=False) for r in REQUESTS) + "\n"
    proc = subprocess.run(
        [str(exe), "serve"],
        input=payload,
        capture_output=True,
        text=True,
        encoding="utf-8",
        timeout=120,
    )
    print(f"exit={proc.returncode}")
    lines = [l for l in proc.stdout.splitlines() if l.strip()]
    print(f"responses={len(lines)} (requests={len(REQUESTS)})")
    check(proc.returncode == 0, "进程正常退出", proc.stderr[:300])
    check(len(lines) == len(REQUESTS), "每个请求一行响应")

    responses = []
    for i, line in enumerate(lines):
        try:
            responses.append(json.loads(line))
        except json.JSONDecodeError as e:
            check(False, f"响应 {i} 是合法 JSON", f"{e}: {line[:120]}")
            return 1

    # id 关联 + 帧同构（每帧都有 id/ok，二选一携带 result/error）
    check(
        [r.get("id") for r in responses] == [r["id"] for r in REQUESTS],
        "响应 id 与请求一一对应",
        str([r.get("id") for r in responses]),
    )
    check(
        all(("result" in r) ^ ("error" in r) for r in responses),
        "帧同构：result / error 二选一",
    )
    check(
        all(r.get("ok") is (("result" in r)) for r in responses),
        "ok 与 result/error 一致",
    )

    by_id = {r["id"]: r for r in responses}

    check(by_id[1]["result"].get("pong") is True, "ping 回应 pong")
    check(bool(by_id[1]["result"].get("abi")), "ping 携带 ABI 版本")

    check(by_id[2]["ok"] and by_id[2]["result"]["ok"] is True, "compile 成功")
    check(isinstance(by_id[2]["result"]["diagnostics"], list), "compile 返回 diagnostics 数组")

    check(by_id[3]["result"]["status"] == "finished", "run 正常结束", str(by_id[3]))
    delta = by_id[4]["result"]["delta"]
    check("3" in delta, "output.delta 含程序输出", repr(delta[:80]))
    check(by_id[4]["result"]["cursor"] == by_id[4]["result"]["total"], "游标推进到末尾")

    step = by_id[6]["result"]
    check(isinstance(step.get("payloads"), list) and bool(step["payloads"]), "step.next 返回 payload")
    payload_keys = set(step["payloads"][0].keys())
    check(
        {"step_index", "code_line", "func_name", "local_vars", "pointer_snapshots"} <= payload_keys,
        "StepPayload 含 schema 字段",
        str(sorted(payload_keys)),
    )

    pg = by_id[8]["result"]
    check("cache_start_step" in pg and "max_collected_step" in pg, "payload.get 携带窗口字段")

    check(by_id[9]["result"]["lines"] == [3], "breakpoints.set 回显行号")
    check(by_id[10]["result"].get("success") is True, "seek 成功", str(by_id[10])[:200])

    regions = by_id[11]["result"]
    check("regions" in regions and "quarantine" in regions, "memory.regions 结构完整")
    check(
        regions["quarantine"]["budget"] == DEFAULT_QUARANTINE_BUDGET,
        "默认隔离预算 256KB（与 capi 一致）",
        str(regions["quarantine"]),
    )
    # C2：三段式内存地图（kind + region_counts + 栈/全局的 name/alloc_line）
    counts = regions.get("region_counts") or {}
    check(
        {"global", "stack", "heap"} <= set(counts.keys()),
        "memory.regions 三段式计数（C2）",
        str(counts),
    )
    check(
        all(isinstance(r.get("kind"), str) for r in regions["regions"]),
        "每个 region 都带 kind 段标识（C2）",
        str([r.get("kind") for r in regions["regions"]]),
    )
    stack_regions = [r for r in regions["regions"] if r.get("kind") == "stack"]
    check(
        counts.get("stack", 0) >= 1 and any(r.get("name") == "main" for r in stack_regions),
        "栈帧区域带函数名（C2）",
        str(stack_regions[:2]),
    )
    check(
        all(r.get("alloc_by") == "call" and r.get("alloc_line") is not None for r in stack_regions),
        "栈帧区域带 alloc_by=call / alloc_line（C2）",
        str(stack_regions[:2]),
    )

    check(by_id[12]["result"]["quarantine_budget"] == 0, "config.set 生效（预算可调）")

    diag_bad = by_id[13]["result"]
    check(diag_bad["ok"] is False and bool(diag_bad["diagnostics"]), "非法程序编译失败并给诊断")

    err = by_id[14]["error"]
    check(err["kind"] == "protocol" and "未知方法" in err["message"], "未知方法 → protocol 错误帧")

    check(by_id[15]["result"]["reset"] is True, "session.reset 成功")
    check(by_id[15]["result"]["config"]["compiled"] is False, "reset 后 compiled=false")
    check(
        by_id[15]["result"]["config"]["quarantine_budget"] == 0,
        "reset 保留会话级配置（隔离预算）",
    )
    check(by_id[16]["ok"] is True, "capabilities 可用")
    caps = by_id[16]["result"]
    check(
        caps.get("languages", {}).get("c", {}).get("stdc_version_macro_nominal") == "202311L",
        "capabilities 版本宏名义锚点",
    )
    check(
        caps.get("memory_model", {}).get("global_region_limit") == 65536,
        "capabilities 内存模型常量（单源 cide_runtime）",
    )
    # B2：schema 轨道与行为契约进能力清单（消费方可直读版本协商信息）
    check(
        caps.get("schema", {}).get("version") == "v0.1"
        and caps.get("schema", {}).get("reserved_fields_v0_2")
        == ["handler_depth", "unwinding", "unwind_frames_left", "current_exception"],
        "capabilities 携带 schema 轨道与预留位（B2）",
        str(caps.get("schema")),
    )
    check(
        any(c.get("id") == "unwinding_step_granularity" for c in caps.get("behavior_contracts", [])),
        "capabilities 携带行为契约（B2：UNWINDING 不合并单步）",
    )
    # 产物自检字段：版本串含构建期 git 短哈希（影子/回放驱动据此 fail fast）
    check(
        isinstance(caps.get("engine_version"), str) and caps["engine_version"].count("(") == 1,
        "capabilities 携带 engine_version（产物自检用）",
        str(caps.get("engine_version")),
    )

    # B2：词汇表导出（词汇只增不改；异常域条目以 reserved 预登记）
    labels = by_id[17]["result"]
    label_ids = [l.get("id") for l in labels.get("labels", [])]
    check(
        "swap" in label_ids and "loop" in label_ids,
        "semantic_labels 导出 C 域词汇（B2）",
        str(label_ids),
    )
    check(
        {"throw", "unwind", "catch_enter", "finally"}
        <= {l.get("id") for l in labels.get("labels", []) if l.get("status") == "reserved"},
        "semantic_labels 预登记异常域词汇（reserved，B2）",
    )

    # B2：契约导出（预留位 + v0.2 台账 + 激活清单）
    contracts = by_id[18]["result"]
    check(
        len(contracts.get("v0_2_activation_checklist", [])) >= 4
        and any(f.get("field") == "code_file" for f in contracts.get("v0_2_field_ledger", [])),
        "contracts 导出激活清单与 v0.2 台账（B2）",
    )

    # D2：单 serve 进程 = 单活跃会话，响应显式回带拓扑字段
    created = by_id[19]["result"]
    sess = created.get("session") or {}
    check(
        created.get("created") is True
        and sess.get("model") == "single-active-session"
        and sess.get("active_sessions") == 1
        and sess.get("concurrent_sessions") is False,
        "session.create 显式回带单会话语义（D2）",
        str(sess),
    )
    check(by_id[20]["result"]["shutdown"] is True, "shutdown 回应")

    edge_failures = run_edge_batch(exe)
    failures.extend(edge_failures)

    rss_failures = run_rss_guard_batch(exe)
    failures.extend(rss_failures)

    print()
    if failures:
        print(f"FAILED: {len(failures)} 项 -> {failures}")
        return 1
    print("serve 冒烟全部通过")
    return 0


# ─── U0#2 RSS 护栏批（2026-09-13）───────────────────────────────────────────
# 防线对宿主内存零观测是两次 GB 级泄漏事故的制度性根因（裁定 §13.2 /
# 事故202609_Seek重放泄漏）。本批在已知压力形状（远距 seek 重放——U2 未
# 修复的瞬时尖峰源）下监控 serve 子进程的提交峰值，超预算即红并回显峰值。
#
# 预算语义（诚实分层）：
# - 默认预算是**当前基线的宽松护栏**（远低于事故量级、高于正常尖峰），
#   不是 J5 的 64B/步——那要等 U2 生命周期重构后才收紧；
# - 证红方式：`CIDE_RSS_BUDGET_MB=5 python scripts/serve_smoke.py`
#   必红（护栏有牙，U0#2"先证会红"义务）。
# 采样：驱动侧 ctypes 直调 psapi（GetProcessMemoryInfo 的提交峰值，
# 与 scripts/internal/probeutil 同口径——不信被测代码自报）。

RSS_PROGRAM = (
    'int main(){ int s=0; for(int i=0;i<60;i++) for(int j=0;j<60;j++) '
    'for(int k=0;k<60;k++) s+=1; return s; }'  # ~65 万逻辑步，远距 seek 的重放源
)


def _peak_commit_mb_windows(pid: int) -> int:
    import ctypes
    from ctypes import wintypes

    class PMC(ctypes.Structure):
        _fields_ = [("cb", wintypes.DWORD), ("PageFaultCount", wintypes.DWORD),
                    ("PeakWorkingSetSize", ctypes.c_size_t),
                    ("WorkingSetSize", ctypes.c_size_t),
                    ("QuotaPeakPagedPoolUsage", ctypes.c_size_t),
                    ("QuotaPagedPoolUsage", ctypes.c_size_t),
                    ("QuotaPeakNonPagedPoolUsage", ctypes.c_size_t),
                    ("QuotaNonPagedPoolUsage", ctypes.c_size_t),
                    ("PagefileUsage", ctypes.c_size_t),
                    ("PeakPagefileUsage", ctypes.c_size_t)]

    PROCESS_QUERY_LIMITED_INFORMATION = 0x1000
    kernel32 = ctypes.windll.kernel32
    psapi = ctypes.windll.psapi
    handle = kernel32.OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, False, pid)
    if not handle:
        return -1
    try:
        pmc = PMC()
        pmc.cb = ctypes.sizeof(PMC)
        if psapi.GetProcessMemoryInfo(handle, ctypes.byref(pmc), pmc.cb):
            return pmc.PeakPagefileUsage // (1024 * 1024)
        return -1
    finally:
        kernel32.CloseHandle(handle)


def run_rss_guard_batch(exe: Path):
    print("\n== RSS 护栏批（远距 seek 压力形状，超预算即红）==")
    budget_mb = int(os.environ.get("CIDE_RSS_BUDGET_MB", "512"))
    if sys.platform != "win32":
        print("  SKIP  非 Windows 平台（CI runner 为 windows-latest；采样走 psapi）")
        return []

    proc = subprocess.Popen(
        [str(exe), "serve"],
        stdin=subprocess.PIPE, stdout=subprocess.PIPE,
        stderr=subprocess.DEVNULL, text=True, encoding="utf-8", bufsize=1,
    )
    fails = []

    def send_and_read(obj):
        proc.stdin.write(json.dumps(obj) + "\n")
        proc.stdin.flush()
        line = proc.stdout.readline()
        return json.loads(line) if line.strip() else None

    try:
        r = send_and_read({"id": 1, "method": "compile", "params": {"source": RSS_PROGRAM}})
        if not (r and r.get("ok")):
            fails.append("rss-batch-compile")
            return fails
        send_and_read({"id": 2, "method": "step.begin"})
        # 前进到程序中段（产生跨检查点分布的帧），随后多轮远距 seek 制造重放尖峰
        for i in range(30):
            send_and_read({"id": 10 + i, "method": "step.next"})
        for round_no, target in enumerate([8000, 20000, 5000, 20000]):
            send_and_read({"id": 100 + round_no, "method": "seek", "params": {"step": target}})
            peak = _peak_commit_mb_windows(proc.pid)
            if peak >= 0:
                print(f"  seek({target:>5}) 后提交峰值 ≈ {peak} MB（预算 {budget_mb} MB）")
        send_and_read({"id": 999, "method": "shutdown"})
    finally:
        try:
            proc.stdin.close()
        except OSError:
            pass
        try:
            proc.wait(timeout=30)
        except subprocess.TimeoutExpired:
            proc.kill()

    peak = _peak_commit_mb_windows(proc.pid)
    # 进程已退出后句柄失效的兜底：以批内打印的采样为准；这里用退出前最后一次为准
    # （OpenProcess 对已退出进程仍可查询峰值——保持断言）
    ok = lambda c, label, detail="": print(f"  {'PASS' if c else 'FAIL'}  {label}" + ("" if c else f"  {detail}")) or (None if c else fails.append(label))
    if peak >= 0:
        ok(peak <= budget_mb, "RSS 护栏：提交峰值在预算内",
           f"peak={peak}MB budget={budget_mb}MB（U2 完成后按 J5 收紧；证红：CIDE_RSS_BUDGET_MB=5）")
    else:
        ok(False, "RSS 护栏：采样可用", "psapi 采样失败")
    return fails


# U0#8 / W0-2 边界样例批：越界 seek / 负参 payload.get / 畸形行——历史上这三个
# 形状直接 panic 杀死会话（R-2026-09-01/02/03，exit 101），修复后必须返回
# 结果帧且进程存活。**任何一处 panic 回归（子进程死亡/响应行缺失）本批必红**，
# 这就是 serve_smoke 对已知 panic 的"埋雷可触发性"（J9）。
EDGE_LINES = [
    '{"id": 101, "method": "compile", "params": {"source": "int main(){ int s=0; for(int i=0;i<3;i++) s+=i; return s; }"}}',
    '{"id": 102, "method": "step.begin"}',
    # 先执行若干步产生帧缓存与检查点——无检查点时 seek_to 在"没有可用的检查点"
    # 提前返回，走不到 finish_replay_window 的窗口路径（埋雷触发的必要前置）
    '{"id": 1021, "method": "step.next"}',
    '{"id": 1022, "method": "step.next"}',
    '{"id": 1023, "method": "step.next"}',
    '{"id": 1024, "method": "step.next"}',
    '{"id": 1025, "method": "step.next"}',
    '{"id": 103, "method": "seek", "params": {"step": 50000}}',          # R-2026-09-01 越程
    '{"id": 104, "method": "seek", "params": {"step": -7}}',             # 负值
    '{"id": 105, "method": "payload.get", "params": {"start": 0, "end": -1}}',  # R-2026-09-03 负 end
    '{"id": 106, "method": "payload.get", "params": {"start": -5, "end": 3}}',  # 负 start
    '{"id": 107, "method": "step.next"}',                                # 越界 seek 后会话仍可用
    '{"id": 108, "method": "no.such.method"}',
    '{"id": 109, "method": "seek"}',                                     # 缺 params
    '{"id": 110, "method": "ping"}',
    '{ this is not json',                                                 # 非法 JSON 行
    '{"id": 111, "method": "ping"}',                                     # 畸形行后进程仍活
]


def run_edge_batch(exe: Path):
    print("\n== 边界/负值/极值批（panic 回归即红）==")
    fails = []
    payload = "\n".join(EDGE_LINES) + "\n"
    try:
        proc = subprocess.run(
            [str(exe), "serve"],
            input=payload,
            capture_output=True,
            text=True,
            encoding="utf-8",
            timeout=120,
        )
    except subprocess.TimeoutExpired:
        print("  FAIL  边界批超时（疑似挂起）")
        return ["edge-batch-timeout"]

    lines = [l for l in proc.stdout.splitlines() if l.strip()]
    ok = lambda c, label, detail="": print(f"  {'PASS' if c else 'FAIL'}  {label}" + ("" if c else f"  {detail}")) or (None if c else fails.append(label))
    ok(proc.returncode == 0, "边界批进程正常退出", f"exit={proc.returncode} stderr={proc.stderr[-300:]}")
    ok(len(lines) == len(EDGE_LINES), "边界批逐行响应（panic 死亡即缺行）",
       f"responses={len(lines)} expected={len(EDGE_LINES)}")

    parsed = []
    for i, line in enumerate(lines):
        try:
            parsed.append(json.loads(line))
        except json.JSONDecodeError as e:
            ok(False, f"边界批响应 {i} 是合法 JSON", f"{e}: {line[:120]}")
            return fails

    by_id = {}
    null_id_frames = []
    for r in parsed:
        rid = r.get("id")
        if rid is None:
            null_id_frames.append(r)
        else:
            by_id[rid] = r

    # 合法请求的 id 关联（101~111 全部在场 = 进程全程存活）
    expected_ids = [101, 102, 1021, 1022, 1023, 1024, 1025, 103, 104, 105, 106, 107, 108, 109, 110, 111]
    ok(all(i in by_id for i in expected_ids), "合法边界请求 id 全部关联（进程存活）",
       f"missing={[i for i in expected_ids if i not in by_id]}")

    # 越界/负参 seek 与 payload.get：返回结果帧或错误帧——禁止 panic（进程死亡已在上面抓）
    for rid in (103, 104):
        r = by_id.get(rid, {})
        ok(("result" in r) ^ ("error" in r), f"seek 边界({rid}) 返回完整帧", str(r)[:150])
    for rid in (105, 106):
        r = by_id.get(rid, {})
        ok(("result" in r) ^ ("error" in r), f"payload.get 负参({rid}) 返回完整帧", str(r)[:150])

    # 越界 seek 后会话仍可用：107 有响应且帧完整
    r107 = by_id.get(107, {})
    ok(("result" in r107) ^ ("error" in r107), "越界 seek 后 step.next 仍可用", str(r107)[:150])

    # 非法 JSON 行 → protocol 错误帧（id null 可接受）
    ok(
        len(null_id_frames) >= 1
        and all(f.get("error", {}).get("kind") == "protocol" for f in null_id_frames),
        "非法 JSON 行 → protocol 错误帧",
        str(null_id_frames[:1])[:150],
    )
    # 存活探针：110/111 pong
    ok(by_id.get(111, {}).get("result", {}).get("pong") is True, "畸形行之后 ping 仍 pong（进程存活）")
    return fails


if __name__ == "__main__":
    sys.exit(main())
