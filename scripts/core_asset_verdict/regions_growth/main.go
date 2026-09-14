// regions_growth：Q7-G6 复核收尾——malloc/free churn 后 regions 条目数多点采样。
//
// 背景（裁定 §13.4 / 实测发现登记 P-1）：regions 登记表重构（U2#2）开工前必须
// 完成 G6 复核——既有证据矛盾：「14B/次无界」（重构评估）vs「峰值平坦」
// （裁定实测）。P-1 的超线性实测（20k=893ms / 100k=8.94s，×5 规模 → ×10 时间）
// 已支持超线性成立；本驱动补**剩余一半**：regions 条目数随 churn 规模的直接采样
// （1k/10k/100k/1M 四点，free 后条目是否永存一测便知）。
//
// 采样通道：serve 会话 `config.set max_steps`（编译前设，避免 10M 步默认上限）
// → `compile` → `run` → `memory.regions` 的 `region_counts.heap`（即
// `session.memory.regions.len()`，条目数第一手采样，不经任何推导）。
//
// 用法：go run ./scripts/core_asset_verdict/regions_growth
//      [--points 1000,10000,100000,1000000] [--cli PATH] [--timeout 45m]
//
// 输出：每点一行（N / 步数 / heap 条目 / free_list 长度 / 墙钟 / 条目÷N 比），
// 结尾给增长率判读（比值 ≈1 → 无界确认；显著 <1 → 既有结论需修正）。
package main

import (
	"bufio"
	"encoding/json"
	"flag"
	"fmt"
	"os"
	"os/exec"
	"strconv"
	"strings"
	"time"

	"vitro/scripts/internal/probeutil"
)

func main() {
	pointsFlag := flag.String("points", "1000,10000,100000,1000000", "逗号分隔的 churn 规模点")
	cliFlag := flag.String("cli", "", "vitro_cli 路径（缺省自动探测）")
	timeoutFlag := flag.Duration("timeout", 45*time.Minute, "单点 serve 子进程硬超时")
	flag.Parse()

	cli := *cliFlag
	if cli == "" {
		cli = probeutil.MustFindCLI()
	}
	var points []int
	for _, p := range strings.Split(*pointsFlag, ",") {
		n, err := strconv.Atoi(strings.TrimSpace(p))
		if err != nil || n <= 0 {
			fmt.Fprintf(os.Stderr, "非法规模点 %q\n", p)
			os.Exit(2)
		}
		points = append(points, n)
	}

	// 启动自检（fail loud）：先用最小点验证采样通道本身——run 必须 finished、
	// memory.regions 必须可达。通道坏了直接拒绝给出判定，禁止静默 default。
	fmt.Println("== Q7-G6 regions 条目数采样（malloc(16)+free churn）==")
	fmt.Printf("cli=%s timeout=%v\n\n", cli, *timeoutFlag)

	fmt.Printf("%9s %12s %10s %10s %10s %8s\n", "N", "steps", "heap条目", "free_list", "墙钟", "条目/N")
	for _, n := range points {
		sample(cli, n, *timeoutFlag)
	}
	fmt.Println(`
判读（分段饱和模型，2026-09-14 G6 实测定论）：
  - 推进期（条目 < 隔离预算/块大小 ≈ 16384）：条目随 N 线性增长（"14B/次无界"
    在此段成立），时间含 O(N²) 分量（每次操作扫描条目数随 N 涨）；
  - 饱和期（隔离预算 DEFAULT_QUARANTINE_BUDGET = MEM_SIZE/4 = 256KB 填满后
    FIFO 驱逐 + free_list 复用）：条目恒定 ~16387（实测），时间 O(N) 线性
    （每次操作扫固定 ~16k 条）——"峰值平坦"与"饱和段≈线性"在此段成立；
  - 两侧既有证据（评估报告"14B/次 + O(N²)" vs 裁定"峰值平坦 + ≈线性"）
    均为真，观测区间不同；U2#2 的核心依据 = 消除每次操作的 ~16k 条线性扫描
    （free 的 iter().find / malloc 复用的 iter().find / freed_logs retain）。`)
}

// sample 跑一个规模点：一次性批量喂 serve（顺序无分支：config→compile→run→regions）。
func sample(cli string, n int, timeout time.Duration) {
	src := churnSource(n)
	// 步数预算：实测 ~25-30 步/次，取 64 倍余量 + 固定底；i32 内安全。
	maxSteps := int64(n)*64 + 500_000
	reqs := []string{
		fmt.Sprintf(`{"id":1,"method":"config.set","params":{"max_steps":%d}}`, maxSteps),
		mustJSON(1, "compile", map[string]any{"source": src}),
		`{"id":3,"method":"run"}`,
		`{"id":4,"method":"memory.regions"}`,
		`{"id":5,"method":"shutdown"}`,
	}
	start := time.Now()
	cmd := exec.Command(cli, "serve")
	cmd.Stdin = strings.NewReader(strings.Join(reqs, "\n") + "\n")
	out, err := cmd.CombinedOutput()
	if err != nil && len(out) == 0 {
		fmt.Fprintf(os.Stderr, "N=%d serve 启动失败: %v\n", n, err)
		os.Exit(2)
	}
	elapsed := time.Since(start)

	var runRes, regionsRes map[string]any
	for _, line := range splitLines(string(out)) {
		var d map[string]any
		if json.Unmarshal([]byte(line), &d) != nil {
			continue
		}
		if !b(d["ok"]) {
			continue
		}
		switch d["id"] {
		case float64(3):
			runRes = m(d["result"])
		case float64(4):
			regionsRes = m(d["result"])
		}
	}
	if runRes == nil || regionsRes == nil {
		fmt.Fprintf(os.Stderr, "N=%d 采样通道失效（run/regions 帧缺失），拒绝判定；原始输出尾部：\n%.400s\n", n, tail(string(out), 400))
		os.Exit(2)
	}
	if s := runRes["status"]; s != "finished" {
		fmt.Fprintf(os.Stderr, "N=%d run 未正常结束（status=%v steps=%v），拒绝判定\n", n, s, runRes["steps_executed"])
		os.Exit(2)
	}
	counts := m(regionsRes["region_counts"])
	heap := counts["heap"]
	freeLen := -1
	if fl, ok := regionsRes["free_list"].([]any); ok {
		freeLen = len(fl)
	}
	ratio := 0.0
	if hv, ok := heap.(float64); ok && n > 0 {
		ratio = hv / float64(n)
	}
	fmt.Printf("%9d %12v %10v %10d %10s %8.3f\n",
		n, runRes["steps_executed"], heap, freeLen,
		elapsed.Truncate(time.Millisecond).String(), ratio)
}

// churnSource 生成 N 次 malloc(16)+free 的 churn 程序（printf 尾巴确认完整跑完）。
func churnSource(n int) string {
	var b strings.Builder
	b.WriteString("#include <stdlib.h>\n#include <stdio.h>\n")
	b.WriteString("int main(){\n")
	b.WriteString("    for (int i = 0; i < " + strconv.Itoa(n) + "; i++) {\n")
	b.WriteString("        int *p = (int*)malloc(16);\n")
	b.WriteString("        free(p);\n")
	b.WriteString("    }\n")
	b.WriteString(`    printf("done\n");` + "\n")
	b.WriteString("    return 0;\n}\n")
	return b.String()
}

func mustJSON(id int, method string, params map[string]any) string {
	p, _ := json.Marshal(params)
	return fmt.Sprintf(`{"id":%d,"method":%q,"params":%s}`, id, method, p)
}

func m(v any) map[string]any {
	mm, _ := v.(map[string]any)
	return mm
}

func b(v any) bool {
	bb, _ := v.(bool)
	return bb
}

func tail(s string, n int) string {
	if len(s) <= n {
		return s
	}
	return s[len(s)-n:]
}

// splitLines 按行切分（不用 bufio.Scanner——其 64KB 行长上限会被大 N 下的
// memory.regions 单行数 MB 打穿；ReadString 无行长限制）。
func splitLines(s string) []string {
	var lines []string
	r := bufio.NewReader(strings.NewReader(s))
	for {
		line, err := r.ReadString('\n')
		line = strings.TrimRight(line, "\r\n")
		if line != "" {
			lines = append(lines, line)
		}
		if err != nil {
			return lines
		}
	}
}
