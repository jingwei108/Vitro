//go:build windows

// Package probeutil 探针共享工具：探针资产目录锚点 + vitro_cli 定位 +
// psapi 驻留内存采样 + 数值格式化（D5 收尾重构，自 resource_longrun /
// seek_accumulation / interaction_probe 的重复实现单源化）。
//
// 口径纪律：CommitMBRaw / PeakCommitMBRaw 返回**未取整**的原始采样值——
// interaction_probe 的历史口径是 round1 后使用、resource/seek 是原始值进
// sampler，两种语义都在调用侧保持；本包不替调用方做取整决定。
package probeutil

import (
	"os"
	"path/filepath"
	"syscall"
	"time"
	"unsafe"

	"vitro/scripts/internal/capi"
)

// VerdictDir 返回探针资产目录 scripts/core_asset_verdict（基线 JSON /
// .findings / .longrun / random_diff.json 等产物的锚点）。驱动 main.go 迁入
// 各自子目录后该锚点不变，产物路径保持稳定。
func VerdictDir() string {
	return filepath.Join(capi.ProjectRoot(), "scripts", "core_asset_verdict")
}

// MustFindCLI 定位 vitro_cli.exe（release 优先，debug 兜底；找不到 fail loud）。
func MustFindCLI() string {
	native := filepath.Join(capi.ProjectRoot(), "native")
	for _, rel := range []string{
		filepath.Join("target", "release", "vitro_cli.exe"),
		filepath.Join("target", "debug", "vitro_cli.exe"),
	} {
		p := filepath.Join(native, rel)
		if _, err := os.Stat(p); err == nil {
			return p
		}
	}
	capi.Fatal("找不到 vitro_cli.exe（请先 cd native && cargo build --release）")
	return ""
}

// ---------------------------------------------------------------- psapi 采样

type processMemoryCounters struct {
	CB                         uint32
	PageFaultCount             uint32
	PeakWorkingSetSize         uintptr
	WorkingSetSize             uintptr
	QuotaPeakPagedPoolUsage    uintptr
	QuotaPagedPoolUsage        uintptr
	QuotaPeakNonPagedPoolUsage uintptr
	QuotaNonPagedPoolUsage     uintptr
	PagefileUsage              uintptr
	PeakPagefileUsage          uintptr
}

var (
	kernel32        = syscall.NewLazyDLL("kernel32.dll")
	procOpenProc    = kernel32.NewProc("OpenProcess")
	procCloseHandle = kernel32.NewProc("CloseHandle")
	psapiDLL        = syscall.NewLazyDLL("psapi.dll")
	procGetMemInfo  = psapiDLL.NewProc("GetProcessMemoryInfo")
)

func memCountersRaw(pid int) *processMemoryCounters {
	const processQueryLimitedInformation = 0x1000
	h, _, _ := procOpenProc.Call(processQueryLimitedInformation, 0, uintptr(pid))
	if h == 0 {
		return nil
	}
	defer procCloseHandle.Call(h)
	var c processMemoryCounters
	c.CB = uint32(unsafe.Sizeof(c))
	ok, _, _ := procGetMemInfo.Call(h, uintptr(unsafe.Pointer(&c)), uintptr(unsafe.Sizeof(c)))
	if ok == 0 {
		return nil
	}
	return &c
}

// CommitMBRaw 当前驻留提交（PagefileUsage / 1MB），未取整；进程不可读时 -1。
func CommitMBRaw(pid int) float64 {
	c := memCountersRaw(pid)
	if c == nil {
		return -1
	}
	return float64(c.PagefileUsage) / 1048576.0
}

// PeakCommitMBRaw 峰值驻留提交（PeakPagefileUsage / 1MB），未取整。
func PeakCommitMBRaw(pid int) float64 {
	c := memCountersRaw(pid)
	if c == nil {
		return -1
	}
	return float64(c.PeakPagefileUsage) / 1048576.0
}

// KillPID 打开进程句柄并 Terminate（对齐 resource_longrun.killPID /
// seek_accumulation.terminatePID 的历史实现；Python 版经 taskkill 子进程）。
func KillPID(pid int) {
	const processTerminate = 0x0001
	h, _, _ := procOpenProc.Call(processTerminate, 0, uintptr(pid))
	if h != 0 {
		syscall.NewLazyDLL("kernel32.dll").NewProc("TerminateProcess").Call(h, 1)
		procCloseHandle.Call(h)
	}
}

// ---------------------------------------------------------------- 数值格式化

// Round1 四舍五入到 0.1（interaction_probe 历史口径）。
func Round1(f float64) float64 { return float64(int(f*10+0.5)) / 10 }

// Round2 四舍五入到 0.01（resource_longrun 历史口径）。
func Round2(f float64) float64 { return float64(int(f*100+0.5)) / 100 }

// Round3 四舍五入到 0.001（seek_accumulation 历史口径）。
func Round3(f float64) float64 { return float64(int(f*1000+0.5)) / 1000 }

// Sec 时长取整到 0.01s。
func Sec(d time.Duration) float64 { return float64(int(d.Seconds()*100)) / 100 }

// Tail 字节尾截断（resource_longrun 历史口径；语义是字节不是字符）。
func Tail(s string, n int) string {
	if len(s) > n {
		return s[len(s)-n:]
	}
	return s
}
