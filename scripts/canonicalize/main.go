// canonicalize：差分锚的 JSON 归一器（P7，2026-09-19）。
//
// 用途：E1/E4 等 B 级锚的对拍——Rust 侧（serde_json）与 MoonBit 侧（显式
// emitter）的 JSON 输出**同经本工具归一**后逐字节比对。归一规则：
//
//   1. 对象键按字典序排序（数组顺序保持——数组是有序结构，排序会销毁信息）；
//   2. 数字形态保持（json.Number 直通——1、1.0、1e2 不互相改写；两侧若对
//      同一数值写出不同形态，属于 emitter 侧要修的差异，不是本工具的归一职责）；
//   3. 字符串转义统一（解码后按 Go 标准编码重转义，HTML 转义关闭）；
//   4. 缩进固定 2 空格 + 尾随换行。
//
// 形态约定（scripts 通行纪律）：零第三方依赖 / fail loud（非法 JSON 拒绝
// 输出，exit 1，禁止静默 default）/ J9 埋雷见 canonicalize_test.go。
//
// 用法：
//
//	go run ./scripts/canonicalize < in.json              # 输出规范形
//	go run ./scripts/canonicalize --check < in.json      # 已规范 exit 0，否则 exit 1（锚定模式）
//	echo '{"b":1,"a":2}' | go run ./scripts/canonicalize # 管道
package main

import (
	"bytes"
	"encoding/json"
	"flag"
	"fmt"
	"io"
	"os"
)

func fatal(format string, args ...interface{}) {
	fmt.Fprintf(os.Stderr, "canonicalize: "+format+"\n", args...)
	os.Exit(1)
}

func main() {
	check := flag.Bool("check", false, "锚定模式：输入已是规范形则 exit 0，否则 exit 1")
	flag.Parse()

	raw, err := io.ReadAll(os.Stdin)
	if err != nil {
		fatal("读 stdin 失败: %v", err)
	}

	canonical, err := canonicalize(raw)
	if err != nil {
		fatal("JSON 归一失败（拒绝输出，不猜不兜底）: %v", err)
	}

	if *check {
		if !bytes.Equal(bytes.TrimSpace(raw), bytes.TrimSpace(canonical)) {
			fmt.Fprintln(os.Stderr, "输入不是规范形（键序/缩进/转义/尾随换行之一不匹配）")
			os.Exit(1)
		}
		return
	}
	os.Stdout.Write(canonical)
}

// canonicalize：原始 JSON 字节 → 规范形字节（尾随换行含）。
func canonicalize(raw []byte) ([]byte, error) {
	dec := json.NewDecoder(bytes.NewReader(raw))
	dec.UseNumber() // 数字保形：1 / 1.0 / 1e2 直通，不改写
	var v interface{}
	if err := dec.Decode(&v); err != nil {
		return nil, fmt.Errorf("非法 JSON: %w", err)
	}
	// 尾随内容拒绝（两份 JSON 拼接是锚数据错误，不是可猜的输入）
	if err := dec.Decode(new(json.RawMessage)); err != io.EOF {
		return nil, fmt.Errorf("输入含多个 JSON 值（锚数据必须是单值）")
	}
	var buf bytes.Buffer
	enc := json.NewEncoder(&buf)
	enc.SetEscapeHTML(false) // HTML 转义差异（&<>）属 emitter 差异，归一层关闭
	enc.SetIndent("", "  ")
	if err := enc.Encode(v); err != nil {
		return nil, fmt.Errorf("重编码失败: %w", err)
	}
	return buf.Bytes(), nil
}
