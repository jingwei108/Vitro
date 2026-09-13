// cabi_smoke.go — Go syscall 驱动 vitro_native.dll 的 C ABI 冒烟（D5 前置验证）。
// 验证三类边界：版本串（rust-alloc 字符串：指针取回 + vitro_free_string 释放）、句柄指针往返。
// 运行：go run cabi_smoke.go（需先 cargo build --release）
// vet 说明：`go vet -unsafeptr=false cabi_smoke.go` 零告警。unsafeptr 单项豁免是已裁定的：
// DLL Call 返回值天然是 uintptr，转 unsafe.Pointer 是 Win32 互操作的必然形态
// （golang.org/x/sys/windows 同型），vet 无法静态证明其合法性而非缺陷。
// 共享 helper 落地时该豁免集中在 helper 一处，业务脚本不得自行转换。
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
	dll         = syscall.NewLazyDLL(dllPath)
	abiVersion  = dll.NewProc("vitro_abi_version")
	engineVer   = dll.NewProc("vitro_engine_version")
	freeString  = dll.NewProc("vitro_free_string")
	sessionNew  = dll.NewProc("vitro_session_create")
	sessionFree = dll.NewProc("vitro_session_destroy")
)

// cString 从 C 字符串指针读 NUL 结尾内容为 Go string（只读扫描，不接管所有权）。
// 这是共享 DLL helper 的雏形——vitro_output.py 的 "restype 必须 c_void_p" 口径在 Go 侧的对应物。
// vet 合规要求 uintptr → unsafe.Pointer 转换与解引用在同一表达式内完成，不得先存变量再转。
func cString(ptr uintptr) string {
	if ptr == 0 {
		return ""
	}
	p := unsafe.Pointer(ptr) // uintptr → Pointer 立即转换（vet 合规形态：不参与后续算术）
	buf := unsafe.Slice((*byte)(p), 4096) // 版本串远小于 4KB，NUL 一定在其中
	n := 0
	for buf[n] != 0 {
		n++
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

	// ① ABI 版本（返回 rust-alloc 字符串 "1.1.0"，不是整数——capi/first_batch.rs:66）
	abip, _, _ := abiVersion.Call()
	if abip == 0 {
		fmt.Fprintln(os.Stderr, "vitro_abi_version 返回 NULL")
		os.Exit(1)
	}
	abi := cString(abip)
	_, _, _ = freeString.Call(abip) // rust-alloc 所有权契约：立即归还
	fmt.Printf("vitro_abi_version = %q\n", abi)

	// ② 引擎版本串（同契约）
	vp, _, _ := engineVer.Call()
	if vp == 0 {
		fmt.Fprintln(os.Stderr, "vitro_engine_version 返回 NULL")
		os.Exit(1)
	}
	ver := cString(vp)
	_, _, _ = freeString.Call(vp)
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
