#!/usr/bin/env python3
"""S3 全流程隔离复现 + serve 内存看门狗。
逐步采样 serve 提交大小；超过 400MB 立即击杀并打印当前执行位置。"""
import json
import subprocess
import sys
import time
import threading

CLI = "native/target/release/vitro_cli.exe"
P3 = ('#include <stdio.h>\n\nint main() {\n    int s = 0;\n    for (int i = 0; i < 3000; i++) {\n'
      '        s += i;\n    }\n    printf("%d\\n", s);\n    return 0;\n}\n')

import os
env = dict(os.environ, VITRO_SEEK_DEBUG="1")
serve = subprocess.Popen([CLI, "serve"], stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                         stderr=open("scripts/replay/serve_stderr.log", "w", encoding="utf-8"), text=True, encoding="utf-8", env=env)
kill = {"flag": False}


def watchdog(pid):
    import ctypes

    class PMC(ctypes.Structure):
        _fields_ = [("cb", ctypes.c_ulong),
                    ("PageFaultCount", ctypes.c_ulong),
                    ("PeakWorkingSetSize", ctypes.c_size_t),
                    ("WorkingSetSize", ctypes.c_size_t),
                    ("QuotaPeakPagedPoolUsage", ctypes.c_size_t),
                    ("QuotaPagedPoolUsage", ctypes.c_size_t),
                    ("QuotaPeakNonPagedPoolUsage", ctypes.c_size_t),
                    ("QuotaNonPagedPoolUsage", ctypes.c_size_t),
                    ("PagefileUsage", ctypes.c_size_t),
                    ("PeakPagefileUsage", ctypes.c_size_t)]

    while not kill["flag"]:
        try:
            h = ctypes.windll.kernel32.OpenProcess(0x0400, False, pid)
            if not h:
                return
            cb = PMC()
            cb.cb = ctypes.sizeof(cb)
            ok = ctypes.windll.psapi.GetProcessMemoryInfo(h, ctypes.byref(cb), ctypes.sizeof(cb))
            ctypes.windll.kernel32.CloseHandle(h)
            if ok and cb.PagefileUsage > 400 * 1024 * 1024:
                mb = cb.PagefileUsage // (1024 * 1024)
                print(f"[WATCHDOG] serve 提交 {mb} MB > 400MB，击杀", flush=True)
                kill["flag"] = True
                subprocess.run(["taskkill", "/PID", str(pid), "/F"], capture_output=True)
                return
        except Exception:
            return
        time.sleep(0.5)


threading.Thread(target=watchdog, args=(serve.pid,), daemon=True).start()

rid = 0


def req(method, params=None):
    global rid
    if kill["flag"]:
        print("看门狗已触发，中止", flush=True)
        sys.exit(1)
    rid += 1
    r = {"id": rid, "method": method}
    if params is not None:
        r["params"] = params
    serve.stdin.write(json.dumps(r) + "\n")
    serve.stdin.flush()
    return json.loads(serve.stdout.readline())


def sample(tag):
    if kill["flag"]:
        print("看门狗已触发，中止", flush=True)
        sys.exit(1)
    try:
        out = subprocess.run(
            ["powershell", "-NoProfile", "-Command",
             f"(Get-Process -Id {serve.pid} -ErrorAction SilentlyContinue).PrivateMemorySize64 / 1MB"],
            capture_output=True, text=True, timeout=10).stdout.strip()
        print(f"[{tag}] serve commit = {out} MB", flush=True)
    except Exception as e:
        print(f"[{tag}] 采样失败 {e}", flush=True)
        sys.exit(1)


req("compile", {"source": P3})
sample("compile")
req("step.begin")
sample("step.begin")
for i in range(1, 2101):
    req("step.next")
    if i % 100 == 0:
        sample(f"step.next x{i}")
    if kill["flag"]:
        sys.exit(1)
print("2050 步完成", flush=True)
r = req("payload.get", {"start": 0, "end": 10})
sample("payload.get")
res = r.get("result") or {}
print(f"cache_start_step={res.get('cache_start_step')} max={res.get('max_collected_step')} n={len(res.get('payloads', []))}", flush=True)
r = req("seek", {"step": 5})
sample("seek(5)")
res = r.get("result") or {}
print(f"seek success={res.get('success')} max={res.get('max_collected_step')} error={res.get('error')}", flush=True)
kill["flag"] = True
serve.stdin.close()
serve.terminate()
print("S3 隔离复现完成（无泄漏则全程 <400MB）", flush=True)
