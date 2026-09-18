package main

// `vitro_cli serve`（JSON-lines 会话模式）端到端冒烟 + 协议断言。
//
// 防线定位：出口 3 的协议契约验证 —— id 关联 / 错误帧同构 / session.reset /
// 与 capi 共用入口语义（StepPayload 字段、隔离预算默认值等）。
//
// D5 后续批次第一站（2026-09-18）：serve_smoke.py → Go。与 Python 版双轨
// 对账（断言逐条同名同序、计数一致、判定一致）后接管 CI，Python 版退役。
// 迁移中锚定的口径：
//   - `断言数: N  (PASS x / FAIL y)` 自报行是 scripts/facts 的采集锚点
//     （serve_smoke_assertions 真值），格式不得改动；
//   - 产物选择取 mtime 较新者（固定 debug 优先曾在陈旧 debug 上拿假绿——
//     J9 埋雷实测踩中：注入 release 后 smoke 仍跑旧 debug 全绿）；
//   - RSS 护栏证红通道：`VITRO_RSS_BUDGET_MB=5`（预算压到 5MB 必红）；
//   - 响应解析走 UseNumber：Python 的 isinstance(int) 整性检查在 Go 侧
//     以 json.Number.Int64 可解析性等价复刻（3 与 3.0 可区分）。
//
// 运行：`go run ./scripts/serve_smoke`（需先构建 vitro_cli：
// `cd native && cargo build --bin vitro_cli`）。
// 或经环境变量指定可执行文件：`VITRO_CLI=/path/to/vitro_cli go run ./scripts/serve_smoke`。

import (
	"bufio"
	"bytes"
	"context"
	"encoding/json"
	"fmt"
	"os"
	"os/exec"
	"path/filepath"
	"runtime"
	"strconv"
	"strings"
	"time"

	"vitro/scripts/internal/capi"
	"vitro/scripts/internal/probeutil"
)

const program = "#include <stdio.h>\n" +
	"int main(){ int a = 1; int b = 2; printf(\"%d\", a + b); return 0; }\n"

// request 按固定字段序序列化（id/method/params），与 Python json.dumps 一致。
type request struct {
	ID     int    `json:"id"`
	Method string `json:"method"`
	Params any    `json:"params,omitempty"`
}

var requests = []request{
	{ID: 1, Method: "ping"},
	{ID: 2, Method: "compile", Params: map[string]any{"source": program}},
	{ID: 3, Method: "run"},
	{ID: 4, Method: "output.delta", Params: map[string]any{"cursor": 0}},
	{ID: 5, Method: "step.begin"},
	{ID: 6, Method: "step.next"},
	{ID: 7, Method: "step.next"},
	{ID: 8, Method: "payload.get", Params: map[string]any{"start": 0, "end": 50}},
	{ID: 9, Method: "breakpoints.set", Params: map[string]any{"lines": []int{3}}},
	{ID: 10, Method: "seek", Params: map[string]any{"step": 1}},
	{ID: 11, Method: "memory.regions"},
	{ID: 12, Method: "config.set", Params: map[string]any{"quarantine_budget": 0}},
	{ID: 13, Method: "compile", Params: map[string]any{"source": "int main(){ int x = ; }}"}},
	{ID: 14, Method: "no.such.method"},
	{ID: 15, Method: "session.reset"},
	{ID: 16, Method: "capabilities"},
	{ID: 17, Method: "semantic_labels"},
	{ID: 18, Method: "contracts"},
	{ID: 19, Method: "session.create"},
	{ID: 20, Method: "shutdown"},
}

// defaultQuarantineBudget 1MB 堆上限的 1/4（堆决议 §1）。
const defaultQuarantineBudget = 256 * 1024

var (
	failures   []string
	assertions int
)

// tally 与 Python 版同构：断言总数自计数，"断言数" 是机器采集的真值口径。
func tally(okFlag bool) { assertions++ }

func check(cond bool, label, detail string) {
	tally(cond)
	if cond {
		fmt.Printf("  PASS  %s\n", label)
	} else {
		fmt.Printf("  FAIL  %s  %s\n", label, detail)
		failures = append(failures, label)
	}
}

// ─── JSON 响应解析（UseNumber：保留整性信息，复刻 Python int/float 区分）───

func parseFrame(line string) (map[string]any, error) {
	dec := json.NewDecoder(strings.NewReader(line))
	dec.UseNumber()
	var m map[string]any
	if err := dec.Decode(&m); err != nil {
		return nil, err
	}
	return m, nil
}

func asObj(v any) map[string]any {
	m, _ := v.(map[string]any)
	return m
}

// numEq 数值等价（Python 的 3 == 3.0 语义）：两侧都是数且值相等。
func numEq(v any, want float64) bool {
	n, ok := v.(json.Number)
	if !ok {
		return false
	}
	f, err := n.Float64()
	return err == nil && f == want
}

// isInt 复刻 Python isinstance(x, int)：必须是整数形态（3.0 / 3e0 不算）。
func isInt(v any) bool {
	n, ok := v.(json.Number)
	if !ok {
		return false
	}
	_, err := n.Int64()
	return err == nil
}

func isBool(v any, want bool) bool {
	b, ok := v.(bool)
	return ok && b == want
}

func strOf(v any) string {
	s, _ := v.(string)
	return s
}

// runServeBatch 一次性喂全部请求、收全部响应（主批 / 边界批 / 串帧批共用）。
// 超时语义对齐 Python subprocess.run(timeout=)：超时视为失败，verdict 不变绿。
func runServeBatch(exe, payload string, timeout time.Duration) (stdout, stderr string, exitCode int, timedOut bool) {
	ctx, cancel := context.WithTimeout(context.Background(), timeout)
	defer cancel()
	cmd := exec.CommandContext(ctx, exe, "serve")
	cmd.Stdin = strings.NewReader(payload)
	var out, errb bytes.Buffer
	cmd.Stdout, cmd.Stderr = &out, &errb
	if err := cmd.Start(); err != nil {
		return "", "", -1, false
	}
	err := cmd.Wait()
	if ctx.Err() == context.DeadlineExceeded {
		return out.String(), errb.String(), -1, true
	}
	code := 0
	if err != nil {
		code = -1
		if ee, ok := err.(*exec.ExitError); ok {
			code = ee.ExitCode()
		}
	}
	return out.String(), errb.String(), code, false
}

func nonEmptyLines(stdout string) []string {
	var out []string
	for _, l := range strings.Split(stdout, "\n") {
		if strings.TrimSpace(l) != "" {
			out = append(out, l)
		}
	}
	return out
}

func main() {
	os.Exit(run())
}

func run() int {
	exe := resolveExe()
	if _, err := os.Stat(exe); err != nil {
		fmt.Printf("错误: 找不到 %s，请先 `cd native && cargo build --bin vitro_cli`\n", exe)
		return 2
	}
	fmt.Printf("vitro_cli: %s\n", exe)

	// ── 主批：20 请求协议契约 ──
	var payloadBuilder strings.Builder
	for _, r := range requests {
		b, _ := json.Marshal(r)
		payloadBuilder.Write(b)
		payloadBuilder.WriteByte('\n')
	}
	stdout, stderr, code, timedOut := runServeBatch(exe, payloadBuilder.String(), 120*time.Second)
	if timedOut {
		fmt.Println("  FAIL  主批超时（疑似挂起）")
		return 1
	}
	fmt.Printf("exit=%d\n", code)
	lines := nonEmptyLines(stdout)
	fmt.Printf("responses=%d (requests=%d)\n", len(lines), len(requests))
	check(code == 0, "进程正常退出", capi.TruncateRunes(stderr, 300))
	check(len(lines) == len(requests), "每个请求一行响应", "")

	var responses []map[string]any
	for i, line := range lines {
		m, err := parseFrame(line)
		if err != nil {
			check(false, fmt.Sprintf("响应 %d 是合法 JSON", i),
				fmt.Sprintf("%v: %s", err, capi.TruncateRunes(line, 120)))
			return 1
		}
		responses = append(responses, m)
	}

	// id 关联 + 帧同构（每帧都有 id/ok，二选一携带 result/error）
	idsOK := len(responses) == len(requests)
	if idsOK {
		for i, r := range responses {
			if !numEq(r["id"], float64(requests[i].ID)) {
				idsOK = false
				break
			}
		}
	}
	check(idsOK, "响应 id 与请求一一对应", fmt.Sprintf("%v", idListOf(responses)))

	xorOK := true
	for _, r := range responses {
		_, hasR := r["result"]
		_, hasE := r["error"]
		if hasR == hasE {
			xorOK = false
			break
		}
	}
	check(xorOK, "帧同构：result / error 二选一", "")

	okConsistent := true
	for _, r := range responses {
		_, hasR := r["result"]
		if isBool(r["ok"], true) != hasR {
			okConsistent = false
			break
		}
	}
	check(okConsistent, "ok 与 result/error 一致", "")

	byID := map[int]map[string]any{}
	for _, r := range responses {
		if n, ok := r["id"].(json.Number); ok {
			if id, err := n.Int64(); err == nil {
				byID[int(id)] = r
			}
		}
	}

	r1 := asObj(byID[1]["result"])
	check(isBool(r1["pong"], true), "ping 回应 pong", "")
	check(strOf(r1["abi"]) != "", "ping 携带 ABI 版本", "")

	r2 := asObj(byID[2]["result"])
	check(isBool(byID[2]["ok"], true) && isBool(r2["ok"], true), "compile 成功", "")
	_, diagIsList := r2["diagnostics"].([]any)
	check(diagIsList, "compile 返回 diagnostics 数组", "")

	r3 := asObj(byID[3]["result"])
	check(strOf(r3["status"]) == "finished", "run 正常结束", tailMap(byID[3]))
	r4 := asObj(byID[4]["result"])
	delta := strOf(r4["delta"])
	check(strings.Contains(delta, "3"), "output.delta 含程序输出", strconv.Quote(capi.TruncateRunes(delta, 80)))
	check(numEq(r4["cursor"], numFloat(r4["total"])), "游标推进到末尾", "")

	// R2（2026-09-14）：一帧发布缓冲（U1#1 P0-1）语义下 step.next 首调返回空
	// payloads 数组（缓冲建立、滞后一帧），此后每次恰 1 帧。合并序列断言：
	// 非空 + step_index 严格递增（spec 附录 A 冻结不变量）。
	var frames []map[string]any
	for _, rid := range []int{6, 7} {
		r := asObj(byID[rid]["result"])
		pl, isList := r["payloads"].([]any)
		check(isList, fmt.Sprintf("step.next(id=%d) 返回 payloads 数组", rid), "")
		for _, p := range pl {
			if m := asObj(p); m != nil {
				frames = append(frames, m)
			}
		}
	}
	check(len(frames) > 0, "step.next 帧序列非空（首调空 + 后续逐帧的合并序列）", "")
	idxSeq := make([]string, 0, len(frames))
	for _, p := range frames {
		if n, ok := p["step_index"].(json.Number); ok {
			idxSeq = append(idxSeq, n.String())
		} else {
			idxSeq = append(idxSeq, "<非数>")
		}
	}
	strictIncr := len(idxSeq) > 0
	for _, s := range idxSeq {
		if !isIntStr(s) {
			strictIncr = false
			break
		}
	}
	if strictIncr {
		for i := 1; i < len(idxSeq); i++ {
			a, _ := strconv.ParseInt(idxSeq[i-1], 10, 64)
			b, _ := strconv.ParseInt(idxSeq[i], 10, 64)
			if b != a+1 {
				strictIncr = false
				break
			}
		}
	}
	check(strictIncr, "step_index 严格递增且无重复投递（spec 附录 A）", fmt.Sprintf("%v", idxSeq))

	schemaFields := []string{"step_index", "code_line", "func_name", "local_vars", "pointer_snapshots"}
	hasAll := len(frames) > 0
	if hasAll {
		for _, f := range schemaFields {
			if _, ok := frames[0][f]; !ok {
				hasAll = false
				break
			}
		}
	}
	check(hasAll, "StepPayload 含 schema 字段", "")

	pg := asObj(byID[8]["result"])
	_, hasCS := pg["cache_start_step"]
	_, hasMC := pg["max_collected_step"]
	check(hasCS && hasMC, "payload.get 携带窗口字段", "")

	r9 := asObj(byID[9]["result"])
	lines3, _ := r9["lines"].([]any)
	check(len(lines3) == 1 && numEq(lines3[0], 3), "breakpoints.set 回显行号", "")

	r10 := asObj(byID[10]["result"])
	check(isBool(r10["success"], true), "seek 成功", tailMap(byID[10]))

	regions := asObj(byID[11]["result"])
	_, hasRegions := regions["regions"]
	_, hasQuarantine := regions["quarantine"]
	check(hasRegions && hasQuarantine, "memory.regions 结构完整", "")
	quarantine := asObj(regions["quarantine"])
	check(numEq(quarantine["budget"], defaultQuarantineBudget), "默认隔离预算 256KB（与 capi 一致）",
		tailMap(quarantine))

	// C2：三段式内存地图（kind + region_counts + 栈/全局的 name/alloc_line）
	counts := asObj(regions["region_counts"])
	countOK := true
	for _, k := range []string{"global", "stack", "heap"} {
		if _, ok := counts[k]; !ok {
			countOK = false
		}
	}
	check(countOK, "memory.regions 三段式计数（C2）", "")

	regionList, _ := regions["regions"].([]any)
	allKinded := true
	for _, rr := range regionList {
		if _, ok := asObj(rr)["kind"].(string); !ok {
			allKinded = false
			break
		}
	}
	check(allKinded, "每个 region 都带 kind 段标识（C2）", "")

	var stackRegions []map[string]any
	for _, rr := range regionList {
		m := asObj(rr)
		if strOf(m["kind"]) == "stack" {
			stackRegions = append(stackRegions, m)
		}
	}
	namedMain := false
	for _, m := range stackRegions {
		if strOf(m["name"]) == "main" {
			namedMain = true
		}
	}
	check(numGe1(counts["stack"]) && namedMain,
		"栈帧区域带函数名（C2）", tailMaps(stackRegions[:min(2, len(stackRegions))]))

	stackMetaOK := true
	for _, m := range stackRegions {
		if strOf(m["alloc_by"]) != "call" || m["alloc_line"] == nil {
			stackMetaOK = false
			break
		}
	}
	check(stackMetaOK, "栈帧区域带 alloc_by=call / alloc_line（C2）", tailMaps(stackRegions[:min(2, len(stackRegions))]))

	r12 := asObj(byID[12]["result"])
	check(numEq(r12["quarantine_budget"], 0), "config.set 生效（预算可调）", "")

	diagBad := asObj(byID[13]["result"])
	diags, _ := diagBad["diagnostics"].([]any)
	check(isBool(diagBad["ok"], false) && len(diags) > 0, "非法程序编译失败并给诊断", "")

	err14 := asObj(byID[14]["error"])
	check(strOf(err14["kind"]) == "protocol" && strings.Contains(strOf(err14["message"]), "未知方法"),
		"未知方法 → protocol 错误帧", "")

	r15 := asObj(byID[15]["result"])
	check(isBool(r15["reset"], true), "session.reset 成功", "")
	cfg15 := asObj(r15["config"])
	check(isBool(cfg15["compiled"], false), "reset 后 compiled=false", "")
	check(numEq(cfg15["quarantine_budget"], 0), "reset 保留会话级配置（隔离预算）", "")

	check(isBool(byID[16]["ok"], true), "capabilities 可用", "")
	caps := asObj(byID[16]["result"])
	langs := asObj(caps["languages"])
	langC := asObj(langs["c"])
	check(strOf(langC["stdc_version_macro_nominal"]) == "202311L", "capabilities 版本宏名义锚点", "")
	memModel := asObj(caps["memory_model"])
	check(numEq(memModel["global_region_limit"], 65536), "capabilities 内存模型常量（单源 vitro_runtime）", "")

	// B2：schema 轨道与行为契约进能力清单（消费方可直读版本协商信息）
	schema := asObj(caps["schema"])
	reserved, _ := schema["reserved_fields_v0_2"].([]any)
	reservedWant := []string{"handler_depth", "unwinding", "unwind_frames_left", "current_exception"}
	reservedEq := strOf(schema["version"]) == "v0.1" && len(reserved) == len(reservedWant)
	if reservedEq {
		for i, s := range reservedWant {
			if strOf(reserved[i]) != s {
				reservedEq = false
				break
			}
		}
	}
	check(reservedEq, "capabilities 携带 schema 轨道与预留位（B2）", tailMap(schema))

	contractsList, _ := caps["behavior_contracts"].([]any)
	hasUnwindContract := false
	for _, c := range contractsList {
		if strOf(asObj(c)["id"]) == "unwinding_step_granularity" {
			hasUnwindContract = true
		}
	}
	check(hasUnwindContract, "capabilities 携带行为契约（B2：UNWINDING 不合并单步）", "")

	ev := strOf(caps["engine_version"])
	check(ev != "" && strings.Count(ev, "(") == 1, "capabilities 携带 engine_version（产物自检用）", ev)

	// B2：词汇表导出（词汇只增不改；异常域条目以 reserved 预登记）
	labels := asObj(byID[17]["result"])
	labelList, _ := labels["labels"].([]any)
	labelIDs := make([]string, 0, len(labelList))
	for _, l := range labelList {
		labelIDs = append(labelIDs, strOf(asObj(l)["id"]))
	}
	check(containsStr(labelIDs, "swap") && containsStr(labelIDs, "loop"),
		"semantic_labels 导出 C 域词汇（B2）", fmt.Sprintf("%v", labelIDs))
	reservedIDs := map[string]bool{}
	for _, l := range labelList {
		m := asObj(l)
		if strOf(m["status"]) == "reserved" {
			reservedIDs[strOf(m["id"])] = true
		}
	}
	reservedAll := true
	for _, id := range []string{"throw", "unwind", "catch_enter", "finally"} {
		if !reservedIDs[id] {
			reservedAll = false
		}
	}
	check(reservedAll, "semantic_labels 预登记异常域词汇（reserved，B2）", "")

	// B2：契约导出（预留位 + v0.2 台账 + 激活清单）
	contracts := asObj(byID[18]["result"])
	actList, _ := contracts["v0_2_activation_checklist"].([]any)
	ledger, _ := contracts["v0_2_field_ledger"].([]any)
	hasCodeFile := false
	for _, f := range ledger {
		if strOf(asObj(f)["field"]) == "code_file" {
			hasCodeFile = true
		}
	}
	check(len(actList) >= 4 && hasCodeFile, "contracts 导出激活清单与 v0.2 台账（B2）", "")

	// D2：单 serve 进程 = 单活跃会话，响应显式回带拓扑字段
	created := asObj(byID[19]["result"])
	sess := asObj(created["session"])
	check(isBool(created["created"], true) && strOf(sess["model"]) == "single-active-session" &&
		numEq(sess["active_sessions"], 1) && isBool(sess["concurrent_sessions"], false),
		"session.create 显式回带单会话语义（D2）", tailMap(sess))

	r20 := asObj(byID[20]["result"])
	check(isBool(r20["shutdown"], true), "shutdown 回应", "")

	// ── 三个追加批 ──
	failures = append(failures, runEdgeBatch(exe)...)
	failures = append(failures, runPendingLeakBatch(exe)...)
	failures = append(failures, runRSSGuardBatch(exe)...)

	fmt.Println()
	// 自报口径（供 facts 台账采集；格式稳定，勿随意改动）
	fmt.Printf("断言数: %d  (PASS %d / FAIL %d)\n", assertions, assertions-len(failures), len(failures))
	if len(failures) > 0 {
		fmt.Printf("FAILED: %d 项 -> %v\n", len(failures), failures)
		return 1
	}
	fmt.Println("serve 冒烟全部通过")
	return 0
}

// resolveExe 定位被测产物：VITRO_CLI 覆盖优先；否则 debug/release 取 mtime
// 较新者——固定 debug 优先曾在陈旧 debug 上拿假绿（J9 埋雷实测）。
func resolveExe() string {
	if override := os.Getenv("VITRO_CLI"); override != "" {
		return override
	}
	name := "vitro_cli"
	if runtime.GOOS == "windows" {
		name = "vitro_cli.exe"
	}
	root := capi.ProjectRoot()
	debug := filepath.Join(root, "native", "target", "debug", name)
	release := filepath.Join(root, "native", "target", "release", name)
	candidates := []string{}
	for _, p := range []string{debug, release} {
		if _, err := os.Stat(p); err == nil {
			candidates = append(candidates, p)
		}
	}
	if len(candidates) == 0 {
		return release
	}
	best := candidates[0]
	var bestMT int64 = -1
	for _, p := range candidates {
		if fi, err := os.Stat(p); err == nil && fi.ModTime().UnixMilli() > bestMT {
			best, bestMT = p, fi.ModTime().UnixMilli()
		}
	}
	return best
}

// ─── 小工具 ──────────────────────────────────────────────────────────────────

func idListOf(responses []map[string]any) []string {
	out := make([]string, 0, len(responses))
	for _, r := range responses {
		out = append(out, fmt.Sprintf("%v", r["id"]))
	}
	return out
}

func numFloat(v any) float64 {
	if n, ok := v.(json.Number); ok {
		f, err := n.Float64()
		if err == nil {
			return f
		}
	}
	return -1
}

func numGe1(v any) bool {
	n, ok := v.(json.Number)
	if !ok {
		return false
	}
	f, err := n.Float64()
	return err == nil && f >= 1
}

func isIntStr(s string) bool {
	_, err := strconv.ParseInt(s, 10, 64)
	return err == nil
}

func containsStr(list []string, want string) bool {
	for _, s := range list {
		if s == want {
			return true
		}
	}
	return false
}

func min(a, b int) int {
	if a < b {
		return a
	}
	return b
}

// tailMap 序列化响应子对象作 FAIL detail（对齐 Python str()[:N] 截断形态）。
func tailMap(m map[string]any) string {
	b, err := json.Marshal(m)
	if err != nil {
		return ""
	}
	return capi.TruncateRunes(string(b), 150)
}

func tailMaps(ms []map[string]any) string {
	b, err := json.Marshal(ms)
	if err != nil {
		return ""
	}
	return capi.TruncateRunes(string(b), 150)
}

// ─── U0#8 / W0-2 边界样例批：越界 seek / 负参 payload.get / 畸形行 ──────────
// 历史上这三个形状直接 panic 杀死会话（R-2026-09-01/02/03，exit 101），
// 修复后必须返回结果帧且进程存活。任何一处 panic 回归（子进程死亡/响应行
// 缺失）本批必红——这就是 serve_smoke 对已知 panic 的"埋雷可触发性"（J9）。

var edgeLines = []string{
	`{"id": 101, "method": "compile", "params": {"source": "int main(){ int s=0; for(int i=0;i<3;i++) s+=i; return s; }"}}`,
	`{"id": 102, "method": "step.begin"}`,
	// 先执行若干步产生帧缓存与检查点——无检查点时 seek_to 在"没有可用的检查点"
	// 提前返回，走不到 finish_replay_window 的窗口路径（埋雷触发的必要前置）
	`{"id": 1021, "method": "step.next"}`,
	`{"id": 1022, "method": "step.next"}`,
	`{"id": 1023, "method": "step.next"}`,
	`{"id": 1024, "method": "step.next"}`,
	`{"id": 1025, "method": "step.next"}`,
	`{"id": 103, "method": "seek", "params": {"step": 50000}}`,                // R-2026-09-01 越程
	`{"id": 104, "method": "seek", "params": {"step": -7}}`,                   // 负值
	`{"id": 105, "method": "payload.get", "params": {"start": 0, "end": -1}}`, // R-2026-09-03 负 end
	`{"id": 106, "method": "payload.get", "params": {"start": -5, "end": 3}}`, // 负 start
	`{"id": 107, "method": "step.next"}`,                                      // 越界 seek 后会话仍可用
	`{"id": 108, "method": "no.such.method"}`,
	`{"id": 109, "method": "seek"}`, // 缺 params
	`{"id": 110, "method": "ping"}`,
	`{ this is not json`,            // 非法 JSON 行
	`{"id": 111, "method": "ping"}`, // 畸形行后进程仍活
}

var edgeExpectedIDs = []int{101, 102, 1021, 1022, 1023, 1024, 1025, 103, 104, 105, 106, 107, 108, 109, 110, 111}

func runEdgeBatch(exe string) []string {
	fmt.Println("\n== 边界/负值/极值批（panic 回归即红）==")
	var fails []string
	stdout, stderr, code, timedOut := runServeBatch(exe, strings.Join(edgeLines, "\n")+"\n", 120*time.Second)
	if timedOut {
		fmt.Println("  FAIL  边界批超时（疑似挂起）")
		return []string{"edge-batch-timeout"}
	}
	lines := nonEmptyLines(stdout)
	check(code == 0, "边界批进程正常退出",
		fmt.Sprintf("exit=%d stderr=%s", code, probeutil.Tail(stderr, 300)))
	check(len(lines) == len(edgeLines), "边界批逐行响应（panic 死亡即缺行）",
		fmt.Sprintf("responses=%d expected=%d", len(lines), len(edgeLines)))

	var parsed []map[string]any
	for i, line := range lines {
		m, err := parseFrame(line)
		if err != nil {
			check(false, fmt.Sprintf("边界批响应 %d 是合法 JSON", i),
				fmt.Sprintf("%v: %s", err, capi.TruncateRunes(line, 120)))
			return fails
		}
		parsed = append(parsed, m)
	}

	byID := map[int]map[string]any{}
	var nullIDFrames []map[string]any
	for _, r := range parsed {
		if n, ok := r["id"].(json.Number); ok {
			if id, err := n.Int64(); err == nil {
				byID[int(id)] = r
				continue
			}
		}
		nullIDFrames = append(nullIDFrames, r)
	}

	// 合法请求的 id 关联（101~111 全部在场 = 进程全程存活）
	var missing []int
	for _, id := range edgeExpectedIDs {
		if _, ok := byID[id]; !ok {
			missing = append(missing, id)
		}
	}
	check(len(missing) == 0, "合法边界请求 id 全部关联（进程存活）",
		fmt.Sprintf("missing=%v", missing))

	frameComplete := func(r map[string]any) bool {
		_, hasR := r["result"]
		_, hasE := r["error"]
		return hasR != hasE
	}
	for _, rid := range []int{103, 104} {
		check(frameComplete(byID[rid]), fmt.Sprintf("seek 边界(%d) 返回完整帧", rid), tailMap(byID[rid]))
	}
	for _, rid := range []int{105, 106} {
		check(frameComplete(byID[rid]), fmt.Sprintf("payload.get 负参(%d) 返回完整帧", rid), tailMap(byID[rid]))
	}

	// 越界 seek 后会话仍可用：107 有响应且帧完整
	check(frameComplete(byID[107]), "越界 seek 后 step.next 仍可用", tailMap(byID[107]))

	// 非法 JSON 行 → protocol 错误帧（id null 可接受）
	nullOK := len(nullIDFrames) >= 1
	for _, f := range nullIDFrames {
		if strOf(asObj(f["error"])["kind"]) != "protocol" {
			nullOK = false
			break
		}
	}
	check(nullOK, "非法 JSON 行 → protocol 错误帧", tailMap(nilSafeFirst(nullIDFrames)))
	// 存活探针：111 pong
	check(isBool(asObj(byID[111]["result"])["pong"], true), "畸形行之后 ping 仍 pong（进程存活）", "")
	return fails
}

func nilSafeFirst(ms []map[string]any) map[string]any {
	if len(ms) == 0 {
		return nil
	}
	return ms[0]
}

// ─── U1#1 二审 P0-A 批（2026-09-13）：同会话二次运行不串帧 ─────────────────
// unified_pending（step.next 一帧发布缓冲）挂在 session 上跨 step_begin 存活，
// 曾致同会话二次运行（不调 session.reset）时新程序首个 step.next 下发上一
// 程序的滞留帧。修复：step_begin 主清 + run 兜底清。本批锚定协议契约：B 程序
// 首帧 step_index 必须为 0（串帧时是 A 的递增步号）。

const (
	p0aProgramA = "#include <stdio.h>\n" +
		"int f(int x){ return x * 2; }\n" +
		"int main(){ int a = 3; printf(\"%d\", f(a)); return 0; }\n"
	p0aProgramB = "#include <stdio.h>\n" +
		"int main(){ int b = 7; printf(\"%d\", b); return 0; }\n"
)

func runPendingLeakBatch(exe string) []string {
	fmt.Println("\n== 二次运行不串帧批（P0-A：pending 泄漏即红）==")
	var fails []string
	reqs := []request{
		{ID: 1, Method: "compile", Params: map[string]any{"source": p0aProgramA}},
		{ID: 2, Method: "step.begin"},
		{ID: 3, Method: "step.next"}, {ID: 3, Method: "step.next"},
		{ID: 3, Method: "step.next"}, {ID: 3, Method: "step.next"},
		{ID: 3, Method: "step.next"},
		// 不调 session.reset——直接换程序（泄漏触发条件）
		{ID: 4, Method: "compile", Params: map[string]any{"source": p0aProgramB}},
		{ID: 5, Method: "step.begin"},
		{ID: 6, Method: "step.next"},
		{ID: 7, Method: "step.next"},
	}
	var pb strings.Builder
	for _, r := range reqs {
		b, _ := json.Marshal(r)
		pb.Write(b)
		pb.WriteByte('\n')
	}
	stdout, _, _, timedOut := runServeBatch(exe, pb.String(), 60*time.Second)
	if timedOut {
		fmt.Println("  FAIL  二次运行批超时")
		return []string{"pending-batch-timeout"}
	}

	// 只收 id=6/7 且 ok=true 的帧（B 程序的 step.next）
	bFrames := []map[string]any{}
	for _, line := range nonEmptyLines(stdout) {
		d, err := parseFrame(line)
		if err != nil {
			continue
		}
		n, ok := d["id"].(json.Number)
		if !ok {
			continue
		}
		id, _ := n.Int64()
		if (id == 6 || id == 7) && isBool(d["ok"], true) {
			payloads, _ := asObj(d["result"])["payloads"].([]any)
			for _, p := range payloads {
				if m := asObj(p); m != nil {
					bFrames = append(bFrames, m)
				}
			}
		}
	}

	check(len(bFrames) > 0, "二次运行有帧返回", fmt.Sprintf("responses=%d", len(bFrames)))
	if len(bFrames) > 0 {
		first := bFrames[0]
		check(numEq(first["step_index"], 0), "B 程序首帧 step_index=0（无 A 程序滞留帧）",
			fmt.Sprintf("step_index=%v（串帧时为 A 的递增步号）", first["step_index"]))
		noALeak := true
		varLV, _ := first["local_vars"].([]any)
		nameList := make([]string, 0, len(varLV))
		for _, v := range varLV {
			name := strOf(asObj(v)["name"])
			nameList = append(nameList, name)
			if name == "a" {
				noALeak = false
			}
		}
		check(noALeak, "B 程序首帧不携带 A 的局部变量",
			fmt.Sprintf("local_vars=%v", nameList))
	}
	return fails
}

// ─── U0#2 RSS 护栏批（2026-09-13）───────────────────────────────────────────
// 防线对宿主内存零观测是两次 GB 级泄漏事故的制度性根因。本批在已知压力形状
// （远距 seek 重放）下监控 serve 子进程的提交峰值，超预算即红并回显峰值。
//
// 预算语义（诚实分层）：默认预算是当前基线的宽松护栏（远低于事故量级、高于
// 正常尖峰），不是 J5 的 64B/步——那要等 U2 生命周期重构后才收紧；
// 证红方式：`VITRO_RSS_BUDGET_MB=5` 必红（护栏有牙，U0#2"先证会红"义务）。
// 采样：probeutil psapi 直调（GetProcessMemoryInfo 的提交峰值，不信被测
// 代码自报）。与 Python 版的差异：Python 在进程退出后凭 Popen 持有的句柄
// 再采样一次；Go 的 exec 在 Wait 后句柄即关，改用**最后一次 seek 后的采样**
// 作断言值——峰值单调，shutdown 只释放不增长，两口径数值一致。

const rssProgram = "int main(){ int s=0; for(int i=0;i<60;i++) for(int j=0;j<60;j++) " +
	"for(int k=0;k<60;k++) s+=1; return s; }" // ~65 万逻辑步，远距 seek 的重放源

func runRSSGuardBatch(exe string) []string {
	fmt.Println("\n== RSS 护栏批（远距 seek 压力形状，超预算即红）==")
	// J5 收紧（U2#1 后，2026-09-14）：512 → 64——seek 重放循环内滚动截断落地后
	// 压力形状实测峰值 23MB（修复前 75MB 基线，-69%）。2.8× 余量覆盖 U2 剩余项
	// 完成后的进一步收紧空间；证红通道不变：VITRO_RSS_BUDGET_MB=5。
	budgetMB := 64
	if v := os.Getenv("VITRO_RSS_BUDGET_MB"); v != "" {
		if n, err := strconv.Atoi(v); err == nil {
			budgetMB = n
		}
	}
	if runtime.GOOS != "windows" {
		fmt.Println("  SKIP  非 Windows 平台（CI runner 为 windows-latest；采样走 psapi）")
		return nil
	}

	cmd := exec.Command(exe, "serve")
	stdin, err := cmd.StdinPipe()
	if err != nil {
		return []string{"rss-batch-spawn"}
	}
	stdoutPipe, err := cmd.StdoutPipe()
	if err != nil {
		return []string{"rss-batch-spawn"}
	}
	devnull, _ := os.OpenFile(os.DevNull, os.O_WRONLY, 0)
	cmd.Stderr = devnull
	if err := cmd.Start(); err != nil {
		return []string{"rss-batch-spawn"}
	}
	w := bufio.NewWriter(stdin)
	r := bufio.NewReader(stdoutPipe)

	sendAndRead := func(obj any) map[string]any {
		b, _ := json.Marshal(obj)
		w.Write(b)
		w.WriteByte('\n')
		w.Flush()
		line, err := r.ReadString('\n')
		if err != nil || strings.TrimSpace(line) == "" {
			return nil
		}
		m, err := parseFrame(line)
		if err != nil {
			return nil
		}
		return m
	}

	pid := cmd.Process.Pid
	peakMB := -1
	compileFailed := false
	func() {
		defer func() {
			w.Flush()
			stdin.Close()
		}()
		resp := sendAndRead(map[string]any{"id": 1, "method": "compile",
			"params": map[string]any{"source": rssProgram}})
		// Python 口径：compile 失败直接 append 失败标签并结束本批（不进断言计数）
		if !isBool(resp["ok"], true) {
			compileFailed = true
			return
		}
		sendAndRead(map[string]any{"id": 2, "method": "step.begin"})
		// 前进到程序中段（产生跨检查点分布的帧），随后多轮远距 seek 制造重放尖峰
		for i := 0; i < 30; i++ {
			sendAndRead(map[string]any{"id": 10 + i, "method": "step.next"})
		}
		for roundNo, target := range []int{8000, 20000, 5000, 20000} {
			sendAndRead(map[string]any{"id": 100 + roundNo, "method": "seek",
				"params": map[string]any{"step": target}})
			peak := int(probeutil.PeakCommitMBRaw(pid))
			if peak >= 0 {
				peakMB = peak
				fmt.Printf("  seek(%5d) 后提交峰值 ≈ %d MB（预算 %d MB）\n", target, peak, budgetMB)
			}
		}
		sendAndRead(map[string]any{"id": 999, "method": "shutdown"})
	}()

	done := make(chan error, 1)
	go func() { done <- cmd.Wait() }()
	select {
	case <-done:
	case <-time.After(30 * time.Second):
		_ = cmd.Process.Kill()
		<-done
	}

	var fails []string
	if compileFailed {
		return append(fails, "rss-batch-compile")
	}
	if peakMB >= 0 {
		check(peakMB <= budgetMB, "RSS 护栏：提交峰值在预算内",
			fmt.Sprintf("peak=%dMB budget=%dMB（J5 已收紧至 64MB：U2#1 后实测 23MB；证红：VITRO_RSS_BUDGET_MB=5）",
				peakMB, budgetMB))
	} else {
		check(false, "RSS 护栏：采样可用", "psapi 采样失败")
	}
	return fails
}
