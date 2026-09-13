//go:build windows

// Package capi 是 Vitro 引擎 DLL（vitro_native.dll）C ABI 绑定与字符串读取的
// 单源封装（D5 收尾重构，2026-09-13）。
//
// 背景：cBytes / ptrToGoString / readChannel / DLL 绑定曾在 shadow_verify /
// shadow_verify_cpp / random_diff / replay 各写一份——口径分叉是本仓库的顽疾
// （对账挖出的三类隐性口径均产生于"同一语义写在多处"），现唯一一份在这里，
// 各驱动只保留判定与流程逻辑。
//
// 契约（与引擎 native/src/capi 一致）：
//   - engine_version 为 rust-alloc，须 vitro_free_string 释放；
//   - 编译错误读取走 CompileErrorsExact（ABI 1.2.0 length API 定长读取），
//     不依赖变长窗口扫描；
//   - Session 句柄非线程安全（DLL 并发调用 → 堆损坏实证）：调用方自行互斥。
package capi

import (
	"fmt"
	"os"
	"os/exec"
	"path/filepath"
	"runtime"
	"strings"
	"sync"
	"syscall"
	"time"
	"unsafe"
)

// ---------------------------------------------------------------- 路径与版本

var (
	projectRootOnce sync.Once
	projectRoot     string
)

// ProjectRoot 返回仓库根（含 native/ 与 scripts/ 的目录），找不到时 exit 2。
// Go 没有 Python 的 __file__ 锚点（go run 的可执行文件在 GOCACHE 临时目录），
// 按"包含 native/ 与 scripts/ 的目录"向上探测，允许从仓库根、scripts/ 或更深
// 子目录运行。
func ProjectRoot() string {
	projectRootOnce.Do(func() { projectRoot = findProjectRoot() })
	return projectRoot
}

func findProjectRoot() string {
	wd, err := os.Getwd()
	if err != nil {
		fmt.Fprintln(os.Stderr, "无法确定工作目录:", err)
		os.Exit(2)
	}
	dir := wd
	for i := 0; i < 5; i++ {
		if isProjectRoot(dir) {
			return dir
		}
		parent := filepath.Dir(dir)
		if parent == dir {
			break
		}
		dir = parent
	}
	fmt.Fprintln(os.Stderr, "请在仓库内运行：找不到包含 native/ 与 scripts/ 的项目根")
	os.Exit(2)
	return ""
}

func isProjectRoot(dir string) bool {
	for _, marker := range []string{"native", "scripts"} {
		if fi, err := os.Stat(filepath.Join(dir, marker)); err != nil || !fi.IsDir() {
			return false
		}
	}
	return true
}

// GitShortHead 当前提交短哈希；git 不可用（导出源码、无 .git）时返回 ""。
func GitShortHead() string {
	cmd := exec.Command("git", "rev-parse", "--short", "HEAD")
	cmd.Dir = ProjectRoot()
	out, err := cmd.Output()
	if err != nil {
		return ""
	}
	return strings.TrimSpace(string(out))
}

// Fatal fail loud：判定型脚本的统一异常出口（exit 2，区别于门禁失败的 exit 1）。
func Fatal(format string, args ...any) {
	fmt.Fprintf(os.Stderr, "FATAL: "+format+"\n", args...)
	os.Exit(2)
}

// ---------------------------------------------------------------- 工具函数

// CBytes 把 Go 字符串转为 NUL 结尾字节切片（调用方须 runtime.KeepAlive）。
func CBytes(s string) []byte {
	b := make([]byte, len(s)+1)
	copy(b, s)
	return b
}

// PtrToGoString 读取 NUL 结尾 UTF-8 C 字符串（只读扫描，4KB→1MB 变长窗口）。
//
// ⚠️ 已声明的假设（PR 评审裁定记录，2026-09-13）：unsafe.Slice(ptr, win) 声明的
// 可读窗口可能大于实际分配——NUL 不在当前窗口内时翻倍重扫，严格说是越界读。
// 实用风险低且已有收口：编译错误（最长的串）改走 CompileErrorsExact 定长读取
// （ABI 1.2.0 length API）；本函数剩余调用方只有 engine_version / runtime_error
// 这类短而有界的串（NUL 在首个 4KB 窗口内命中，窗口不翻倍即无越界）。
// 若未来要彻底消除：引擎为相应出口补 length API 后改定长读取（同
// CompileErrorsExact 模式），或 VirtualQuery 逐页探测（成本不匹配收益）。
func PtrToGoString(ptr uintptr) string {
	if ptr == 0 {
		return ""
	}
	for win := 4096; win <= 1<<20; win *= 2 {
		s := unsafe.Slice((*byte)(unsafe.Pointer(ptr)), win)
		for i, b := range s {
			if b == 0 {
				return string(s[:i])
			}
		}
	}
	Fatal("C 字符串超过 1MB 扫描上限（指针 0x%x），拒绝静默截断", ptr)
	return ""
}

// ReadChannel 读取带 length API 的输出通道：length<=0 → ""，否则取 NUL 结尾缓冲。
func ReadChannel(h uintptr, lenProc, copyProc *syscall.LazyProc) string {
	r, _, _ := lenProc.Call(h)
	n := int(int32(r))
	if n <= 0 {
		return ""
	}
	buf := make([]byte, n+1)
	copyProc.Call(h, uintptr(unsafe.Pointer(&buf[0])), uintptr(n+1))
	runtime.KeepAlive(buf)
	for i, b := range buf {
		if b == 0 {
			return string(buf[:i])
		}
	}
	return string(buf)
}

// Normalize 输出比对口径：strip 两端 + CRLF → LF
// （等价 Python text=True 的 universal newlines + .strip()）。
func Normalize(s string) string {
	return strings.ReplaceAll(strings.TrimSpace(s), "\r\n", "\n")
}

// TruncateRunes 按字符数截断（Python 的 s[:n] 是字符截断，Go 切片是字节）。
func TruncateRunes(s string, n int) string {
	r := []rune(s)
	if len(r) <= n {
		return s
	}
	return string(r[:n])
}

// ---------------------------------------------------------------- DLL 绑定

// E-P1-5：结构化输出通道必需符号（ABI >= 1.1.0），缺失即 fail fast，不退回文本清洗。
var requiredSymbols = []string{
	"vitro_get_program_output_length",
	"vitro_get_program_output",
	"vitro_get_engine_notes_length",
	"vitro_get_engine_notes",
}

// DLL 引擎 C ABI 的绑定集合（各驱动共用；字段名与旧驱动内联版一一对应）。
type DLL struct {
	DLL                 *syscall.LazyDLL
	SessionCreate       *syscall.LazyProc
	SessionDestroy      *syscall.LazyProc
	Compile             *syscall.LazyProc
	CompileUnit         *syscall.LazyProc
	CompileAll          *syscall.LazyProc
	Run                 *syscall.LazyProc
	SetInputMode        *syscall.LazyProc
	SetInput            *syscall.LazyProc
	CompileErrors       *syscall.LazyProc
	CompileErrorsLength *syscall.LazyProc
	RuntimeError        *syscall.LazyProc
	ProgOutLen          *syscall.LazyProc
	ProgOut             *syscall.LazyProc
	NotesLen            *syscall.LazyProc
	Notes               *syscall.LazyProc
	EngineVersion       *syscall.LazyProc // 可选：缺失时跳过产物新鲜度校验
	FreeString          *syscall.LazyProc
}

// Load 加载并绑定引擎 DLL。
//
//   - E-P1-5 必需符号缺失 → fail fast（exit 2，不退回文本清洗）；
//   - 其余符号即时绑定（缺符号同样 fail loud——ABI 1.2.0 起全部存在）；
//   - engine_version / free_string 可选（老产物缺失时跳过新鲜度校验，ABI 校验兜底）；
//   - 加载后即做产物新鲜度校验：版本串必须含当前 HEAD（陈旧二进制上全量假绿，
//     实测事故见 AGENTS.md 产物新鲜度门禁）。
func Load(path string) *DLL {
	if _, err := os.Stat(path); err != nil {
		Fatal("找不到引擎 DLL：%s（请先 cd native && cargo build --release）", path)
	}
	d := &DLL{DLL: syscall.NewLazyDLL(path)}
	bind := func(name string) *syscall.LazyProc {
		p := d.DLL.NewProc(name)
		if err := p.Find(); err != nil {
			Fatal("DLL 缺少符号 %s：%v\n需要 ABI >= 1.1.0 的结构化输出通道。请重建引擎：cd native && cargo build --release\n不要退回文本清洗：E-P1-5 已废除该口径。", name, err)
		}
		return p
	}
	for _, name := range requiredSymbols {
		bind(name)
	}
	d.SessionCreate = bind("vitro_session_create")
	d.SessionDestroy = bind("vitro_session_destroy")
	d.Compile = bind("vitro_compile")
	d.CompileUnit = bind("vitro_compile_unit")
	d.CompileAll = bind("vitro_compile_all")
	d.Run = bind("vitro_run")
	d.SetInputMode = bind("vitro_set_input_mode")
	d.SetInput = bind("vitro_set_input")
	d.CompileErrors = bind("vitro_get_compile_errors")
	d.CompileErrorsLength = bind("vitro_get_compile_errors_length")
	d.RuntimeError = bind("vitro_get_runtime_error")
	d.ProgOutLen = bind("vitro_get_program_output_length")
	d.ProgOut = bind("vitro_get_program_output")
	d.NotesLen = bind("vitro_get_engine_notes_length")
	d.Notes = bind("vitro_get_engine_notes")
	if d.DLL.NewProc("vitro_engine_version").Find() == nil && d.DLL.NewProc("vitro_free_string").Find() == nil {
		d.EngineVersion = d.DLL.NewProc("vitro_engine_version")
		d.FreeString = d.DLL.NewProc("vitro_free_string")
	}
	d.EnsureFreshArtifacts()
	return d
}

// EnsureFreshArtifacts：DLL 版本串必须含当前 HEAD 短哈希，否则 fail fast（exit 2）。
func (d *DLL) EnsureFreshArtifacts() {
	if d.EngineVersion == nil {
		return
	}
	head := GitShortHead()
	if head == "" {
		return // git 不可用（导出源码、无 .git）：跳过，ABI 校验兜底
	}
	raw, _, _ := d.EngineVersion.Call()
	version := PtrToGoString(raw)
	if raw != 0 {
		d.FreeString.Call(raw)
	}
	if !strings.Contains(version, head) {
		Fatal("引擎产物不是当前提交构建的：vitro_engine_version()=%q 不含 HEAD %s。\n请先 cd native && cargo build --release —— 否则会在陈旧二进制上得到假绿。",
			version, head)
	}
}

// CompileErrorsExact 定长读取编译错误 JSON（ABI 1.2.0）：先取字节长度再按
// NUL 终止契约读 n+1 字节——长度与内容精确对应，不依赖扫描窗口假设。
// 引擎保证错误串不含内嵌 NUL（CString::new 失败即返回 null），因此 NUL
// 必然落在第 n 字节，读取范围恰为 CString 分配大小（len+1）。
func (d *DLL) CompileErrorsExact(h uintptr) string {
	r, _, _ := d.CompileErrorsLength.Call(h)
	n := int(int32(r))
	if n <= 0 {
		return ""
	}
	p, _, _ := d.CompileErrors.Call(h)
	if p == 0 {
		Fatal("引擎契约破坏：vitro_get_compile_errors_length=%d 但 vitro_get_compile_errors 返回 NULL", n)
	}
	buf := unsafe.Slice((*byte)(unsafe.Pointer(p)), n+1)
	return string(buf[:n])
}

// RunProgramStdout 纯程序 stdout 通道（E-P1-5；调用方自行 TrimSpace / Normalize）。
func (d *DLL) RunProgramStdout(h uintptr) string {
	return ReadChannel(h, d.ProgOutLen, d.ProgOut)
}

// RuntimeErr 读运行时错误（短而有界的 trap 串，走 PtrToGoString，见其假设声明）。
func (d *DLL) RuntimeErr(h uintptr) string {
	p, _, _ := d.RuntimeError.Call(h)
	return PtrToGoString(p)
}

// 时间辅助：驱动侧耗时统计共用。
func MsSince(start time.Time) float64 {
	return float64(time.Since(start).Milliseconds())
}
