#!/usr/bin/env python3
"""统一模式后端性能基线测试脚本。

用法:
    python scripts/unified_perf_baseline.py [--report reports/unified_perf_baseline.md]

说明:
    使用 release 模式的 vitro_cli 运行 native/benches/unified_perf_baseline.c，
    测量产生约 10 万 VM 步的冒泡排序用例在后端统一模式下的执行耗时，
    并生成 Markdown 格式的基线报告。
"""

import argparse
import os
import re
import subprocess
import io
import sys
# Windows CI 控制台默认 cp1252，编码不了中文/✓ 等字符（UnicodeEncodeError）。
# 与 shadow_verify.py 的入口处理一致：显式把 stdout/stderr 重包为 UTF-8。
sys.stdout = io.TextIOWrapper(sys.stdout.buffer, encoding="utf-8", errors="replace")
sys.stderr = io.TextIOWrapper(sys.stderr.buffer, encoding="utf-8", errors="replace")

import time
from pathlib import Path


def _ensure_utf8_console() -> None:
    """在 Windows 上确保 Python 标准 IO 使用 UTF-8，避免中文乱码。"""
    if sys.platform == "win32":
        try:
            import ctypes
            ctypes.windll.kernel32.SetConsoleOutputCP(65001)
            ctypes.windll.kernel32.SetConsoleCP(65001)
        except Exception:
            pass
        try:
            sys.stdout.reconfigure(encoding="utf-8")
            sys.stderr.reconfigure(encoding="utf-8")
        except Exception:
            pass


_ensure_utf8_console()


ROOT = Path(__file__).resolve().parent.parent
NATIVE_DIR = ROOT / "native"
BENCH_SOURCE = NATIVE_DIR / "benches" / "unified_perf_baseline.c"
CLI_EXE = NATIVE_DIR / "target" / "release" / "vitro_cli.exe"
DEFAULT_MAX_STEPS = 200_000


def build_cli() -> None:
    """编译 release 版本的 vitro_cli。"""
    print("编译 release 版本 vitro_cli...")
    result = subprocess.run(
        ["cargo", "build", "--release", "--bin", "vitro_cli"],
        cwd=NATIVE_DIR,
        text=True,
        encoding="utf-8",
        errors="replace",
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
    )
    if result.returncode != 0:
        print(result.stdout)
        raise RuntimeError("vitro_cli 编译失败")


def run_baseline() -> dict:
    """运行基准用例并收集指标。"""
    if not CLI_EXE.exists():
        build_cli()

    cmd = [
        str(CLI_EXE),
        "unified",
        str(BENCH_SOURCE),
        "--max-steps",
        str(DEFAULT_MAX_STEPS),
    ]

    print(f"运行基准: {' '.join(cmd)}")
    start = time.perf_counter()
    result = subprocess.run(
        cmd,
        text=True,
        encoding="utf-8",
        errors="replace",
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
    )
    elapsed = time.perf_counter() - start

    output = result.stdout
    if result.returncode != 0:
        print(output)
        raise RuntimeError("基准运行失败")

    total_steps_match = re.search(r"总步数:\s*(\d+)", output)
    status_match = re.search(r"状态:\s*(.+)", output)
    if not total_steps_match:
        print(output)
        raise RuntimeError("无法解析总步数")

    total_steps = int(total_steps_match.group(1))
    status = status_match.group(1).strip() if status_match else "未知"
    steps_per_sec = total_steps / elapsed if elapsed > 0 else 0.0

    return {
        "total_steps": total_steps,
        "elapsed_sec": elapsed,
        "steps_per_sec": steps_per_sec,
        "status": status,
        "max_steps": DEFAULT_MAX_STEPS,
    }


def format_report(metrics: dict) -> str:
    """生成 Markdown 格式报告。"""
    now = time.strftime("%Y-%m-%dT%H:%M:%S%z", time.localtime())
    return f"""# Vitro 统一模式后端性能基线

> 生成时间: {now}
> 测试用例: `native/benches/unified_perf_baseline.c`（50 个逆序元素冒泡排序）

## 测试环境

| 指标 | 数值 |
|------|------|
| CLI 可执行文件 | `native/target/release/vitro_cli.exe` |
| 统一模式最大步数限制 | {metrics['max_steps']:,} |
| 编译配置 | release |

## 当前基线

| 指标 | 数值 |
|------|------|
| 实际执行步数 | {metrics['total_steps']:,} |
| 执行耗时 | {metrics['elapsed_sec']:.3f} s |
| 平均吞吐 | {metrics['steps_per_sec']:,.0f} 步/秒 |
| 单步耗时 | {1_000_000 / metrics['steps_per_sec']:.1f} μs/步 |
| 执行状态 | {metrics['status']} |

## 结论与说明

- 该基线仅测量**后端**统一模式生成帧数据的速度，不包含 Flutter 前端渲染、差分解码与绘制的开销。
- 前端 55fps 目标需在上述后端吞吐基础上，叠加 Dart 端 `UnifiedNotifier` 的差分解码、`StepPayload` 反序列化以及 `CustomPainter` 绘制耗时。
- 若单步耗时持续低于 18ms/帧（≈ 55fps 预算），则前端回放 10 万步在理论上具备达到 55fps 的后端基础；实际帧率需在完整 Flutter 桌面端环境中进一步实测。

## 历史记录

| 日期 | 步数 | 耗时 | 吞吐 | 备注 |
|------|------|------|------|------|
| {now.split('T')[0]} | {metrics['total_steps']:,} | {metrics['elapsed_sec']:.3f}s | {metrics['steps_per_sec']:,.0f} 步/秒 | 初始基线 |
"""


def main() -> int:
    parser = argparse.ArgumentParser(description="统一模式后端性能基线测试")
    parser.add_argument(
        "--report",
        type=Path,
        default=ROOT / "reports" / "unified_perf_baseline.md",
        help="报告输出路径",
    )
    args = parser.parse_args()

    if not BENCH_SOURCE.exists():
        print(f"错误: 基准用例不存在: {BENCH_SOURCE}", file=sys.stderr)
        return 1

    metrics = run_baseline()
    report = format_report(metrics)

    args.report.parent.mkdir(parents=True, exist_ok=True)
    args.report.write_text(report, encoding="utf-8")

    print(f"\n基线测试完成:")
    print(f"  总步数: {metrics['total_steps']:,}")
    print(f"  耗时:   {metrics['elapsed_sec']:.3f}s")
    print(f"  吞吐:   {metrics['steps_per_sec']:,.0f} 步/秒")
    print(f"  报告:   {args.report}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
