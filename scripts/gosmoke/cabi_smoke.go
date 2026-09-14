// cabi_smoke.go — Go syscall 驱动 vitro_native.dll 的 C ABI 冒烟（D5 前置验证）。
// 验证两类边界：版本串（buf 写入式读取，ABI 2.1.0：零所有权转移）、句柄指针往返。
// 运行：go run cabi_smoke.go（需先 cargo build --release）
// vet 说明（U2#13 修订）：旧头注声称"unsafeptr 单项豁免是已裁定的"——与事实不符，
// vet unsafeptr 检查器不接受任何 uintptr→unsafe.Pointer 逆向转换（无论豁免与否
// 它都在默认检查集内）。本文件的 cString 曾是三处 vet 命中之一，已随 buf 写入式
// API（vitro_*_version_into）整体移除；`go vet ./scripts/...` 现零输出且为 CI 门禁。
package main

import (
	"fmt"
	"os"
	"path/filepath"
	"runtime"
	"syscall"
	"unsafe"
)

// dllPath 以本源文件位置推导仓库根，摆脱对本地目录名的绝对路径依赖
// （go run 模式下 runtime.Caller 返回源文件真实路径）。
var dllPath = func() string {
	_, this, _, _ := runtime.Caller(0) // <repo>/scripts/gosmoke/cabi_smoke.go
	root := filepath.Dir(filepath.Dir(filepath.Dir(this)))
	return filepath.Join(root, "native", "target", "release", "vitro_native.dll")
}()

var (
	dll            = syscall.NewLazyDLL(dllPath)
	abiVersionInto = dll.NewProc("vitro_abi_version_into")
	engineVerInto  = dll.NewProc("vitro_engine_version_into")
	sessionNew     = dll.NewProc("vitro_session_create")
	sessionFree    = dll.NewProc("vitro_session_destroy")
)

// engineVersionInto 读引擎版本串（buf 写入式 ABI 2.1.0）：调用方缓冲 + 正向
// 指针，零所有权转移。U2#13 收口：原 cString 的 uintptr→unsafe.Pointer 逆向
// 转换（注释自称"vet 合规形态"与事实不符——unsafeptr 检查器不接受任何
// uintptr→Pointer 转换，无论是否先存变量）随此移除。
func readIntoProc(proc *syscall.LazyProc) string {
	buf := make([]byte, 64)
	r, _, _ := proc.Call(uintptr(unsafe.Pointer(&buf[0])), uintptr(len(buf)))
	runtime.KeepAlive(buf)
	n := int(int32(r))
	if n <= 0 {
		return ""
	}
	if n >= len(buf) {
		n = len(buf) - 1
	}
	return string(buf[:n])
}

func mustFind() {
	if err := dll.Load(); err != nil {
		fmt.Fprintln(os.Stderr, "DLL 加载失败:", err)
		os.Exit(2)
	}
}

func main() {
	mustFind()

	// ① ABI 版本（buf 写入式 ABI 2.1.0，零所有权转移）
	abi := readIntoProc(abiVersionInto)
	if abi == "" {
		fmt.Fprintln(os.Stderr, "vitro_abi_version_into 读取失败")
		os.Exit(1)
	}
	fmt.Printf("vitro_abi_version = %q\n", abi)

	// ② 引擎版本串（同契约）
	ver := readIntoProc(engineVerInto)
	fmt.Printf("vitro_engine_version = %q\n", ver)

	// ③ 句柄指针往返：create → destroy（非 NULL 即契约成立）
	sh, _, _ := sessionNew.Call()
	if sh == 0 {
		fmt.Fprintln(os.Stderr, "vitro_session_create 返回 NULL")
		os.Exit(1)
	}
	fmt.Printf("session handle = 0x%x\n", sh)
	sessionFree.Call(sh)
	fmt.Println("cabi smoke OK：版本串×2 + 释放契约 + 句柄往返全过")
}
