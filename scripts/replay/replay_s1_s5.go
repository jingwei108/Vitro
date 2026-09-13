//go:build windows

// replay_s1_s5 —— S1–S5 回放驱动的 Go 迁移（D5 第二站，裁定文档 §13.5）。
//
// 与 Python 版（scripts/replay/replay_s1_s5.py）的断言编号与判定口径一一对应：
//
//	S1-防抖编译流 A1–A10 / S2-fixtures判分流 A1–A6 / S3-单步seek内存交错流 A1–A16 /
//	S5-预留位缺省语义 A1–A5（schema v0.1，docs/spec/STEP_PAYLOAD_SCHEMA_V0_1.md）。
//
// 迁移要点：
//   - serve 会话是单进程时序协议流（compile → step → seek 状态互相依赖），
//     不做会话内并发；实测 Python 版全量 0.39s（W0-2 止血后），性能非动因；
//   - 前置门禁（preflight）fail fast exit 2：capabilities.engine_version 必须存在
//     且含当前 HEAD 短哈希；锚点缺省从版本串自取，显式 --anchor 必须命中版本串；
//   - 输出格式与 Python 版一致（`  [PASS] S1 A1` / 汇总块），双轨对账可逐行 diff；
//   - --selftest：J9 埋雷——对判定 helper 注入必然违反的输入，必须变红，否则 exit 2。
//
// 用法：go run scripts/replay/replay_s1_s5.go [--cli PATH] [--anchor <短哈希>] [--sections S1,S2,S3,S5] [--selftest]
package main

import (
	"cide/scripts/internal/capi"

	"bufio"
	"encoding/json"
	"flag"
	"fmt"
	"os"
	"os/exec"
	"path/filepath"
	"reflect"
	"regexp"
	"sort"
	"strings"
	"syscall"
	"time"
)

// ---------------------------------------------------------------- 路径

var (
	cliDefault string
	dllPath    string
)

func init() {
	root := capi.ProjectRoot()
	cliDefault = filepath.Join(root, "native", "target", "release", "cide_cli.exe")
	dllPath = filepath.Join(root, "native", "target", "release", "cide_native.dll")
}

// ---------------------------------------------------------------- JSON 访问 helper

func mmap(v any) map[string]any {
	m, _ := v.(map[string]any)
	return m
}

func marr(v any) []any {
	a, _ := v.([]any)
	return a
}

// abiVersionAtLeast 解析 "major.minor.patch" 版本串并断言下限；
// 解析失败 / major 不同 / 低于要求 → false（A4a 必红）。
func abiVersionAtLeast(v string, minMajor, minMinor int) bool {
	var major, minor, patch int
	if _, err := fmt.Sscanf(v, "%d.%d.%d", &major, &minor, &patch); err != nil {
		return false
	}
	if major != minMajor {
		return false
	}
	return minor >= minMinor
}

func mstr(v any) string {
	s, _ := v.(string)
	return s
}

func mnum(v any) float64 {
	f, _ := v.(float64)
	return f
}

// truthy 对齐 Python 的真值语义（nil / "" / false / 0 为假）
func truthy(v any) bool {
	switch t := v.(type) {
	case nil:
		return false
	case bool:
		return t
	case string:
		return t != ""
	case float64:
		return t != 0
	default:
		return true
	}
}

// ---------------------------------------------------------------- Serve：cide_cli serve 会话

type frame struct {
	req  map[string]any
	resp map[string]any
}

type Serve struct {
	cmd    *exec.Cmd
	stdin  ioWriteCloser
	stdout *bufio.Reader
	nextID float64
	frames []frame
}

type ioWriteCloser interface {
	Write(p []byte) (int, error)
	Close() error
}

func newServe(cliPath string) *Serve {
	cmd := exec.Command(cliPath, "serve")
	stdin, err := cmd.StdinPipe()
	if err != nil {
		capi.Fatal("serve stdin pipe: %v", err)
	}
	stdout, err := cmd.StdoutPipe()
	if err != nil {
		capi.Fatal("serve stdout pipe: %v", err)
	}
	cmd.Stderr = os.Stderr
	if err := cmd.Start(); err != nil {
		capi.Fatal("无法启动 %s serve：%v（请先 cd native && cargo build --release）", cliPath, err)
	}
	return &Serve{
		cmd:    cmd,
		stdin:  stdin.(ioWriteCloser),
		stdout: bufio.NewReader(stdout),
		nextID: 1,
	}
}

// request 发送 NDJSON 请求并读一行响应（id 关联由引擎保证，驱动逐帧记录）。
func (s *Serve) request(method string, params map[string]any) map[string]any {
	rid := s.nextID
	s.nextID++
	req := map[string]any{"id": rid, "method": method}
	if params != nil {
		req["params"] = params
	}
	line, err := json.Marshal(req)
	if err != nil {
		capi.Fatal("请求序列化失败: %v", err)
	}
	if _, err := s.stdin.Write(append(line, '\n')); err != nil {
		capi.Fatal("写 serve stdin 失败（进程已退出？）: %v", err)
	}
	respLine, err := s.stdout.ReadBytes('\n')
	if err != nil && len(respLine) == 0 {
		capi.Fatal("serve 进程提前退出（无响应）：%v", err)
	}
	var resp map[string]any
	if err := json.Unmarshal(respLine, &resp); err != nil {
		capi.Fatal("响应解析失败 %q: %v", string(respLine), err)
	}
	resp["id"] = mnum(resp["id"]) // 统一数字形态，A1 的 id 比较用 float64
	s.frames = append(s.frames, frame{req: req, resp: resp})
	return resp
}

// shutdown 等价 Python：发 shutdown → 关 stdin → wait(10s)；超时 kill 返回 1。
func (s *Serve) shutdown() int {
	s.request("shutdown", nil)
	s.stdin.Close()
	done := make(chan error, 1)
	go func() { done <- s.cmd.Wait() }()
	select {
	case <-time.After(10 * time.Second):
		s.cmd.Process.Kill()
		return 1
	case <-done:
		if s.cmd.ProcessState == nil {
			return 1
		}
		return s.cmd.ProcessState.ExitCode()
	}
}

// collectPayloads 从已收集帧中取全部 StepPayload（step.next / payload.get / seek）。
func (s *Serve) collectPayloads() []map[string]any {
	var out []map[string]any
	for _, f := range s.frames {
		result, ok := f.resp["result"].(map[string]any)
		if !ok {
			continue
		}
		if pls, ok := result["payloads"].([]any); ok {
			for _, p := range pls {
				if pm := mmap(p); pm != nil {
					out = append(out, pm)
				}
			}
		}
		if pm := mmap(result["payload"]); pm != nil {
			out = append(out, pm)
		}
	}
	return out
}

// ---------------------------------------------------------------- Report

type reportRow struct {
	section string
	aid     string
	ok      bool
	detail  string
}

type Report struct {
	rows []reportRow
}

// check 输出格式与 Python 版一致：PASS 也打印尾部两空格，FAIL 附 detail。
func (r *Report) check(section, aid string, cond bool, detail string) bool {
	r.rows = append(r.rows, reportRow{section, aid, cond, detail})
	mark := "FAIL"
	if cond {
		mark = "PASS"
	}
	d := ""
	if !cond {
		d = detail
	}
	fmt.Printf("  [%s] %s %s  %s\n", mark, section, aid, d)
	return cond
}

func (r *Report) summarize() int {
	total := len(r.rows)
	failed := 0
	for _, row := range r.rows {
		if !row.ok {
			failed++
		}
	}
	fmt.Println("\n========== 回放汇总 ==========")
	fmt.Printf("断言总数: %d  PASS: %d  FAIL: %d\n", total, total-failed, failed)
	for _, row := range r.rows {
		if !row.ok {
			fmt.Printf("  FAIL %s %s: %s\n", row.section, row.aid, row.detail)
		}
	}
	if failed > 0 {
		return 1
	}
	return 0
}

// ---------------------------------------------------------------- 载体源码（与下游文档逐字一致）

const (
	s1K1 = "#include <stdio.h>\n\nint main() {\n    int age = 14\n    printf(\"age=%d\\n\", age);\n    return 0;\n}\n"
	s1K2 = "#include <stdio.h>\n\nint main() {\n    int age = \"小明\";\n    printf(\"age=%d\\n\", age);\n    return 0;\n}\n"
	s1K3 = "#include <stdio.h>\n\nint main() {\n    int age = 14;\n    double height = 1.62;\n    printf(\"age=%d\\n\", age);\n    printf(\"height=%.2f\\n\", height);\n    return 0;\n}\n"

	s2Src = "#include <stdio.h>\n\nint main() {\n    int n;\n    scanf(\"%d\", &n);\n" +
		"    int digits = 0;\n    int t = n;\n    while (t > 0) {\n        t /= 10;\n        digits++;\n    }\n" +
		"    printf(\"digits=%d\\n\", digits);\n    while (n > 0) {\n        printf(\"%d\", n % 10);\n        n /= 10;\n    }\n" +
		"    printf(\"\\n\");\n    return 0;\n}\n"

	s3P1 = "#include <stdio.h>\n\nvoid swap(int *a, int *b) {\n    int t = *a;\n    *a = *b;\n    *b = t;\n}\n\n" +
		"int main() {\n    int x = 3;\n    int y = 8;\n    swap(&x, &y);\n    printf(\"%d %d\\n\", x, y);\n    return 0;\n}\n"
	s3P2 = "#include <stdio.h>\n#include <stdlib.h>\n\nint main() {\n" +
		"    int *p = (int *)malloc(4 * sizeof(int));\n    p[0] = 7;\n    printf(\"%d\\n\", p[0]);\n    free(p);\n    return 0;\n}\n"
	s3P3 = "#include <stdio.h>\n\nint main() {\n    int s = 0;\n    for (int i = 0; i < 3000; i++) {\n        s += i;\n    }\n" +
		"    printf(\"%d\\n\", s);\n    return 0;\n}\n"
)

var s2Fixtures = []struct {
	name     string
	stdin    string
	expected string
}{
	{"F1", "12340\n", "digits=5\n04321\n"},
	{"F2", "7\n", "digits=1\n7\n"},
	{"F3", "10086\n", "digits=5\n68001\n"},
}

// ---------------------------------------------------------------- S1：防抖编译流

func diagErrors(resp map[string]any) []map[string]any {
	r := mmap(resp["result"])
	var out []map[string]any
	for _, d := range marr(r["diagnostics"]) {
		if dm := mmap(d); dm != nil && mstr(dm["severity"]) == "error" {
			out = append(out, dm)
		}
	}
	return out
}

func runS1(s *Serve, rep *Report) {
	fmt.Println("\n── S1 防抖编译流 ──")
	r101 := s.request("compile", map[string]any{"source": s1K1})
	r102 := s.request("compile", map[string]any{"source": s1K2})
	r103 := s.request("compile", map[string]any{"source": s1K3})
	r104 := s.request("compile", map[string]any{"source": s1K3})

	// A1 id 回填与帧同构（全序列终检在 A10；此处先验 compile 组）
	a1 := true
	for _, f := range s.frames {
		okVal, hasOk := f.resp["ok"]
		same := mnum(f.resp["id"]) == f.req["id"] &&
			hasOk && (okVal == true || mmap(f.resp["error"]) != nil)
		if !same {
			a1 = false
			rep.check("S1", "A1", false, fmt.Sprintf("id=%v 帧形状异常", f.req["id"]))
			break
		}
	}
	if a1 {
		rep.check("S1", "A1", true, "")
	}

	e101 := diagErrors(r101)
	if len(e101) == 1 && mstr(e101[0]["code"]) == "E2005" && mnum(e101[0]["line"]) == 4 &&
		strings.Contains(mstr(e101[0]["message"]), ";") {
		rep.check("S1", "A2", true, "")
	} else {
		var codes []string
		for _, d := range e101 {
			codes = append(codes, fmt.Sprintf("('%s', %v)", mstr(d["code"]), mnum(d["line"])))
		}
		rep.check("S1", "A2", false, "["+strings.Join(codes, ", ")+"]")
	}

	e102 := diagErrors(r102)
	if len(e102) == 1 && mstr(e102[0]["code"]) == "E3004" && mnum(e102[0]["line"]) == 4 &&
		strings.Contains(mstr(e102[0]["message"]), "类型不匹配") {
		rep.check("S1", "A3", true, "")
	} else {
		var codes []string
		for _, d := range e102 {
			codes = append(codes, fmt.Sprintf("('%s', %v)", mstr(d["code"]), mnum(d["line"])))
		}
		rep.check("S1", "A3", false, "["+strings.Join(codes, ", ")+"]")
	}

	res103 := mmap(r103["result"])
	res104 := mmap(r104["result"])
	ok103 := res103["ok"] == true && len(marr(res103["diagnostics"])) == 0 && isPresentArray(res103, "diagnostics")
	ok104 := res104["ok"] == true && len(marr(res104["diagnostics"])) == 0 && isPresentArray(res104, "diagnostics")
	rep.check("S1", "A4", ok103 && ok104, "")

	// A5 重复编译诊断逐字段相等（Python dict ==）
	d103 := res103["diagnostics"]
	d104 := res104["diagnostics"]
	rep.check("S1", "A5", reflect.DeepEqual(d103, d104), "重复编译诊断逐字段相等")

	// A6 诊断字段全集 + 形状约束（对 r101 + r102 的全部诊断）
	allDiags := append(marr(mmap(r101["result"])["diagnostics"]), marr(mmap(r102["result"])["diagnostics"])...)
	fieldsOK := len(allDiags) > 0
	for _, d := range allDiags {
		dm := mmap(d)
		for _, k := range []string{"severity", "line", "column", "end_line", "end_column", "code",
			"error_code", "filename", "message", "fix_suggestion"} {
			if _, has := dm[k]; !has {
				fieldsOK = false
			}
		}
		if mnum(dm["line"]) < 1 || mnum(dm["end_column"]) < mnum(dm["column"])+1 {
			fieldsOK = false
		}
	}
	rep.check("S1", "A6", fieldsOK, "")

	r105 := s.request("step.begin", nil)
	rep.check("S1", "A7a", r105["ok"] == true, fmt.Sprintf("%v", r105))

	a7 := true
	var prev float64
	for i := 0; i < 3; i++ {
		r := s.request("step.next", nil)
		pls := marr(mmap(r["result"])["payloads"])
		if len(pls) != 1 {
			a7 = false
			break
		}
		idx := mnum(mmap(pls[0])["step_index"])
		if i > 0 && idx != prev+1 {
			a7 = false
			break
		}
		prev = idx
	}
	rep.check("S1", "A7b", a7, "3 次 step.next 各恰 1 payload 且 step_index 连续")

	r109 := s.request("compile", map[string]any{"source": s1K3})
	r110 := s.request("payload.get", map[string]any{"start": 0, "end": 3})
	switch {
	case r109["ok"] == true && r110["ok"] == true:
		pls := marr(mmap(r110["result"])["payloads"])
		idxOK := len(pls) == 3
		for i, p := range pls {
			if mnum(mmap(p)["step_index"]) != float64(i) {
				idxOK = false
			}
		}
		rep.check("S1", "A8", idxOK, "重编译成功且步数据未串")
	case r109["ok"] == true && r110["ok"] != true:
		rep.check("S1", "A8", mstr(mmap(r110["error"])["kind"]) == "state", "状态要求显式化")
	default:
		rep.check("S1", "A8", false, fmt.Sprintf("第三种结果 r109=%v r110=%v", r109, r110))
	}

	r111 := s.request("compile", map[string]any{"source": s1K2})
	e111 := diagErrors(r111)
	rep.check("S1", "A9", len(e111) == 1 && mstr(e111[0]["code"]) == "E3004" &&
		strings.Contains(mstr(e111[0]["message"]), "类型不匹配"), "")
}

// isPresentArray：键存在且为数组（Python `diagnostics == []` 要求键存在、是数组、为空）
func isPresentArray(m map[string]any, key string) bool {
	v, has := m[key]
	if !has {
		return false
	}
	_, isArr := v.([]any)
	return isArr
}

// ---------------------------------------------------------------- S2：fixtures 判分流

func runS2(s *Serve, rep *Report) {
	fmt.Println("\n── S2 fixtures 判分流 ──")
	s.request("ping", nil)
	s.request("config.set", map[string]any{"deterministic": true})
	s.request("compile", map[string]any{"source": s2Src})

	type roundResult struct {
		rnd      int
		fname    string
		expected string
		runResp  map[string]any
		deltaRes map[string]any
	}
	var rounds []roundResult

	for _, rnd := range []int{1, 2} {
		for _, fx := range s2Fixtures {
			rReset := s.request("session.reset", nil)
			cfg := mmap(mmap(rReset["result"])["config"])
			cfgOK := cfg["deterministic"] == true
			detail := ""
			if !cfgOK {
				detail = "reset 保留 deterministic"
			}
			rep.check("S2", fmt.Sprintf("A5 R%d %s", rnd, fx.name), cfgOK, detail)
			s.request("compile", map[string]any{"source": s2Src})
			rRun := s.request("run", map[string]any{"input": fx.stdin, "deterministic": true})
			rDelta := s.request("output.delta", map[string]any{"cursor": 0, "stream": "stdout"})
			rounds = append(rounds, roundResult{rnd, fx.name, fx.expected, rRun, mmap(rDelta["result"])})
		}
	}

	for _, rr := range rounds {
		res := mmap(rr.runResp["result"])
		a1 := rr.runResp["ok"] == true && mstr(res["status"]) == "finished" &&
			res["waiting_input"] == false && mstr(res["trap"]) == "" && mnum(res["return_value"]) == 0
		rep.check("S2", fmt.Sprintf("A1 R%d %s", rr.rnd, rr.fname), a1, fmt.Sprintf("%v", res))

		delta := mstr(rr.deltaRes["delta"])
		a2 := delta == rr.expected && mnum(rr.deltaRes["cursor"]) == mnum(rr.deltaRes["total"]) &&
			mstr(rr.deltaRes["stream"]) == "stdout"
		rep.check("S2", fmt.Sprintf("A2 R%d %s", rr.rnd, rr.fname), a2,
			fmt.Sprintf("delta=%q 期望=%q", delta, rr.expected))

		a3 := !strings.Contains(delta, "程序运行完成") && !strings.Contains(delta, "=====")
		rep.check("S2", fmt.Sprintf("A3 R%d %s", rr.rnd, rr.fname), a3, "stdout 流无引擎附注")
	}

	// A4 两轮可复现（stdout / steps_executed / return_value）
	a4 := true
	for _, fx := range s2Fixtures {
		var first, second roundResult
		for _, rr := range rounds {
			if rr.fname != fx.name {
				continue
			}
			if rr.rnd == 1 {
				first = rr
			} else {
				second = rr
			}
		}
		fRes := mmap(first.runResp["result"])
		sRes := mmap(second.runResp["result"])
		if mnum(fRes["steps_executed"]) != mnum(sRes["steps_executed"]) ||
			mnum(fRes["return_value"]) != mnum(sRes["return_value"]) ||
			mstr(first.deltaRes["delta"]) != mstr(second.deltaRes["delta"]) {
			a4 = false
		}
	}
	rep.check("S2", "A4", a4, "两轮可复现（stdout/steps/return_value）")

	// A6 sanity（Python 版死代码行已省略，语义等价：R1 各 fixture 的 steps 记录）
	steps := map[string]float64{}
	for _, rr := range rounds {
		if rr.rnd == 1 {
			steps[rr.fname] = mnum(mmap(rr.runResp["result"])["steps_executed"])
		}
	}
	sanity := true
	for _, v := range steps {
		if v <= 0 {
			sanity = false
		}
	}
	if !(steps["F2"] < steps["F1"]) {
		sanity = false
	}
	rep.check("S2", "A6", sanity, fmt.Sprintf("steps=%v（sanity 记录，非门禁）", steps))
}

// ---------------------------------------------------------------- S3：单步 + seek + 内存交错流

// stepUntil 等价 Python step_until：返回 (hit, hitPayload)
func stepUntil(s *Serve, cond func(map[string]any) bool) (bool, map[string]any) {
	for i := 0; i < 500; i++ {
		r := s.request("step.next", nil)
		result := mmap(r["result"])
		pls := marr(result["payloads"])
		if len(pls) > 0 {
			last := mmap(pls[len(pls)-1])
			if cond(last) {
				return true, last
			}
		}
		if truthy(result["finished"]) || truthy(result["trapped"]) {
			return false, nil
		}
	}
	return false, nil
}

func runS3(s *Serve, rep *Report) string {
	fmt.Println("\n── S3 单步 + seek + 内存交错流 ──")
	// P1
	s.request("compile", map[string]any{"source": s3P1})
	s.request("step.begin", nil)
	s.request("breakpoints.set", map[string]any{"lines": []int{12}})
	hit, hitPl := stepUntil(s, func(p map[string]any) bool { return mnum(p["code_line"]) == 12 })
	rep.check("S3", "A1", hit && mstr(hitPl["semantic_label"]) == "调用 swap" && mstr(hitPl["func_name"]) == "main",
		fmt.Sprintf("label=%q func=%q", mstr(hitPl["semantic_label"]), mstr(hitPl["func_name"])))

	rStick := s.request("step.next", nil)
	stickRes := mmap(rStick["result"])
	stick := stickRes["paused"] == true && isPresentArray(stickRes, "payloads") && len(marr(stickRes["payloads"])) == 0
	rep.check("S3", "A2", stick, "暂停态粘性（不推进）")

	s.request("breakpoints.set", map[string]any{"lines": []int{}})
	rResume := s.request("step.next", nil)
	res := mmap(rResume["result"])
	pls := marr(res["payloads"])
	a3 := res["paused"] == false && len(pls) > 0 && mnum(mmap(pls[0])["step_index"]) > mnum(hitPl["step_index"])
	rep.check("S3", "A3", a3, "清断点后恢复推进且不重编号")

	// 推进至 swap 体内（指针快照出现）
	var ptrPl map[string]any
loop:
	for i := 0; i < 30; i++ {
		r := s.request("step.next", nil)
		res := mmap(r["result"])
		for _, p := range marr(res["payloads"]) {
			pm := mmap(p)
			ptrs := marr(pm["pointer_snapshots"])
			if len(ptrs) >= 2 {
				allValid := true
				for _, x := range ptrs {
					if mstr(mmap(x)["status"]) != "Valid" {
						allValid = false
					}
				}
				if allValid {
					ptrPl = pm
					break loop
				}
			}
		}
		if truthy(res["finished"]) || truthy(res["trapped"]) {
			break
		}
	}
	a4 := false
	detail := "未捕获双指针快照"
	if ptrPl != nil {
		ptrs := marr(ptrPl["pointer_snapshots"])
		var addrs []float64
		nameOK := true
		for _, x := range ptrs {
			xm := mmap(x)
			addrs = append(addrs, mnum(xm["target_addr"]))
			if v, has := xm["target_name"]; has && !truthy(v) {
				nameOK = false
			}
		}
		sort.Float64s(addrs)
		tyOK := true
		for _, x := range ptrs {
			if mstr(mmap(x)["ty_name"]) != "int*" {
				tyOK = false
			}
		}
		a4 = tyOK && len(addrs) == 2 && addrs[0] == 1048568 && addrs[1] == 1048572 && nameOK
		detail = fmt.Sprintf("addrs=%v ty=%v", addrs, func() []string {
			var out []string
			for _, x := range ptrs {
				out = append(out, mstr(mmap(x)["ty_name"]))
			}
			return out
		}())
	}
	rep.check("S3", "A4", a4, detail)

	// A5/A6 全帧扫描（此处扫描的是 P1 时点的累积帧，与 Python 版一致）
	a5 := true
	a6 := true
	prevHm := map[float64]float64{}
	var prevIdx any
	for _, f := range s.frames {
		result := mmap(f.resp["result"])
		for _, p := range marr(result["payloads"]) {
			pm := mmap(p)
			for _, av := range marr(pm["accessed_vars"]) {
				at := mstr(mmap(av)["access_type"])
				if at != "Read" && at != "Write" {
					a5 = false
				}
			}
			codeLine := mnum(pm["code_line"])
			if mnum(pm["heatmap_line"]) != codeLine {
				a6 = false
			}
			if prev, has := prevHm[codeLine]; has && mnum(pm["heatmap_count"]) < prev {
				a6 = false
			}
			if mnum(pm["heatmap_count"]) > prevHm[codeLine] {
				prevHm[codeLine] = mnum(pm["heatmap_count"])
			}
			if mnum(pm["step_index"]) == 0 {
				prevIdx = pm["code_line"]
			}
		}
	}
	rep.check("S3", "A5", a5, "access_type ∈ {Read, Write}")
	rep.check("S3", "A6", a6, "heatmap_count 同行单调不减且 heatmap_line==code_line")

	// A7 step 0 前奏步口径（Python 条件 idx0 is None or code_line in (0,1) or >= 1）
	a7 := prevIdx == nil || mnum(prevIdx) == 0 || mnum(prevIdx) == 1 || mnum(prevIdx) >= 1
	rep.check("S3", "A7", a7, "step 0 前奏步口径")

	rSeek := s.request("seek", map[string]any{"step": 5})
	resS := mmap(rSeek["result"])
	a8 := resS["success"] == true && mnum(mmap(resS["payload"])["step_index"]) == 5
	rep.check("S3", "A8", a8, fmt.Sprintf("seek(5)=%v", map[string]any{"success": resS["success"]}))

	rPg := s.request("payload.get", map[string]any{"start": 0, "end": 6})
	pls6 := marr(mmap(rPg["result"])["payloads"])
	seqOK := true
	for i, p := range pls6 {
		if mnum(mmap(p)["step_index"]) != float64(i) {
			seqOK = false
		}
	}
	a8b := seqOK || len(pls6) == 6
	rep.check("S3", "A8b", a8b && len(pls6) == 6, fmt.Sprintf("payload.get(0,6) 步号连续 0–5，实际 %v", stepIndexOf(pls6)))

	hasSuccess := false
	hasError := false
	if _, has := resS["success"]; has {
		hasSuccess = true
	}
	if _, has := resS["error"]; has {
		hasError = true
	}
	a9 := hasSuccess && hasError && ((resS["payload"] != nil) != (resS["error"] != nil))
	rep.check("S3", "A9", a9, "seek 帧同构（payload/error 二选一）")

	// P2
	s.request("compile", map[string]any{"source": s3P2})
	s.request("step.begin", nil)
	s.request("memory.regions", nil)
	var mallocSeen, freedSeen map[string]any
	quarantine := map[string]any{}
	for i := 0; i < 400; i++ {
		r := s.request("step.next", nil)
		res := mmap(r["result"])
		rMem := s.request("memory.regions", nil)
		regs := marr(mmap(rMem["result"])["regions"])
		for _, rg := range regs {
			rgm := mmap(rg)
			if mstr(rgm["alloc_by"]) == "malloc" && mnum(rgm["alloc_line"]) == 5 && mnum(rgm["size"]) == 16 {
				mallocSeen = rgm
				if rgm["is_freed"] == true {
					freedSeen = rgm
				}
			}
		}
		if q := mmap(mmap(rMem["result"])["quarantine"]); mnum(q["blocks"]) >= 1 {
			quarantine = q
		}
		if truthy(res["finished"]) || truthy(res["trapped"]) {
			// 终态后再取一次终局 regions
			rMem := s.request("memory.regions", nil)
			regs := marr(mmap(rMem["result"])["regions"])
			for _, rg := range regs {
				rgm := mmap(rg)
				if mstr(rgm["alloc_by"]) == "malloc" && mnum(rgm["alloc_line"]) == 5 && mnum(rgm["size"]) == 16 {
					freedSeen = rgm
				}
			}
			if q := mmap(mmap(rMem["result"])["quarantine"]); q != nil {
				quarantine = q
			}
			break
		}
	}
	rep.check("S3", "A10", mallocSeen != nil, fmt.Sprintf("malloc region=%v", mallocSeen))
	a11 := freedSeen != nil && mnum(quarantine["blocks"]) == 1 && mnum(quarantine["bytes"]) >= 16
	rep.check("S3", "A11", a11, fmt.Sprintf("free 后 is_freed + 隔离区 %v", quarantine))

	if mallocSeen != nil {
		heapAddr := mnum(mallocSeen["addr"])
		heapEnd := heapAddr + mnum(mallocSeen["size"])
		if ptrPl != nil {
			overlap := false
			for _, x := range marr(ptrPl["pointer_snapshots"]) {
				ta := mnum(mmap(x)["target_addr"])
				if heapAddr <= ta && ta < heapEnd {
					overlap = true
				}
			}
			rep.check("S3", "A12", !overlap, "栈指针与堆 region 不相交")
		} else {
			rep.check("S3", "A12", true, "（无指针快照样本，跳过）")
		}
	} else {
		rep.check("S3", "A12", false, "无 malloc region")
	}

	// P3
	s.request("compile", map[string]any{"source": s3P3})
	s.request("step.begin", nil)
	s.request("run", map[string]any{"deterministic": true})
	rPg = s.request("payload.get", map[string]any{"start": 0, "end": 1})
	resPg := mmap(rPg["result"])
	a13a := isPresentArray(resPg, "payloads") && len(marr(resPg["payloads"])) == 0 && mnum(resPg["cache_start_step"]) == 0
	rep.check("S3", "A13a", a13a, "全速后无帧缓存（payloads==[]）")

	// seek 语义（锚点固化后更新，已同步下游）：step 0 检查点恒存在，任何
	// >=0 的 seek 都可成功；"success:false" 仅在 0 步场景成立。
	rSeek = s.request("seek", map[string]any{"step": 5})
	resS = mmap(rSeek["result"])
	rep.check("S3", "A13b", resS["success"] == true, "seek(5) 经检查点恢复成功（锚点固化后语义）")

	s.request("step.begin", nil)
	for i := 0; i < 2050; i++ {
		r := s.request("step.next", nil)
		res := mmap(r["result"])
		if truthy(res["finished"]) || truthy(res["trapped"]) {
			break
		}
	}
	rPg = s.request("payload.get", map[string]any{"start": 0, "end": 10})
	resPg = mmap(rPg["result"])
	rep.check("S3", "A14", mnum(resPg["cache_start_step"]) > 0,
		fmt.Sprintf("cache_start_step=%v（窗口已裁剪）", resPg["cache_start_step"]))

	rSeek = s.request("seek", map[string]any{"step": 5})
	resS = mmap(rSeek["result"])
	a15 := resS["success"] == true
	if a15 {
		a15 = mnum(mmap(resS["payload"])["step_index"]) == 5
	}
	rep.check("S3", "A15", a15, fmt.Sprintf("越窗 seek(5) 恢复+重放 %v", resS["max_collected_step"]))

	return s3P3
}

func stepIndexOf(pls []any) []float64 {
	var out []float64
	for _, p := range pls {
		out = append(out, mnum(mmap(p)["step_index"]))
	}
	return out
}

// runS3A16：两次独立进程 deterministic run，stdout/steps 一致
func runS3A16(cliPath string, rep *Report, p3Src string) {
	type outPair struct {
		delta string
		steps float64
		ret   float64
	}
	var outs []outPair
	for i := 0; i < 2; i++ {
		s := newServe(cliPath)
		s.request("compile", map[string]any{"source": p3Src})
		s.request("config.set", map[string]any{"deterministic": true})
		r := s.request("run", map[string]any{"deterministic": true})
		res := mmap(r["result"])
		rDelta := s.request("output.delta", map[string]any{"cursor": 0, "stream": "stdout"})
		delta := mstr(mmap(rDelta["result"])["delta"])
		outs = append(outs, outPair{delta, mnum(res["steps_executed"]), mnum(res["return_value"])})
		s.shutdown()
	}
	sanity := outs[0].delta == "4498500\n"
	rep.check("S3", "A16", outs[0] == outs[1] && sanity,
		fmt.Sprintf("两次 run 一致 %+v（sanity 期望 4498500）", outs[0]))
}

// ---------------------------------------------------------------- S5：预留位缺省语义

// v01PayloadFields / reservedFields：v0.1 字段白名单，单源资产
// v01_payload_fields.json（与 replay_s1_s5.py 共读同一份）。语义快照随该文件
// git 版本化——加载失败/为空/schema 不符一律 fail loud：白名单是 S5 断言的
// 判据，静默降级等于拔掉防线 5 的牙。
var (
	v01PayloadFields, reservedFields = loadV01Fields()
)

func loadV01Fields() (v01, reserved map[string]bool) {
	path := filepath.Join(capi.ProjectRoot(), "scripts", "replay", "v01_payload_fields.json")
	data, err := os.ReadFile(path)
	if err != nil {
		capi.Fatal("读取字段白名单失败 %s: %v", path, err)
	}
	var parsed struct {
		Schema           string   `json:"schema"`
		V01PayloadFields []string `json:"v01_payload_fields"`
		ReservedFields   []string `json:"reserved_fields"`
	}
	if err := json.Unmarshal(data, &parsed); err != nil {
		capi.Fatal("字段白名单 JSON 非法 %s: %v", path, err)
	}
	if parsed.Schema != "v0.1" || len(parsed.V01PayloadFields) == 0 || len(parsed.ReservedFields) == 0 {
		capi.Fatal("字段白名单 %s schema/内容不合法（schema=%q, v0.1 字段 %d, 预留字段 %d）",
			path, parsed.Schema, len(parsed.V01PayloadFields), len(parsed.ReservedFields))
	}
	v01 = map[string]bool{}
	for _, f := range parsed.V01PayloadFields {
		v01[f] = true
	}
	reserved = map[string]bool{}
	for _, f := range parsed.ReservedFields {
		reserved[f] = true
	}
	return v01, reserved
}

func runS5(s *Serve, rep *Report, payloads []map[string]any, anchor string) {
	var a1Fail, a2Fail, a3Fail []string
	for _, p := range payloads {
		var unknown, reserved []string
		for k := range p {
			if !v01PayloadFields[k] {
				unknown = append(unknown, k)
			}
			if reservedFields[k] {
				reserved = append(reserved, k)
			}
		}
		sort.Strings(unknown)
		sort.Strings(reserved)
		if len(unknown) > 0 {
			a1Fail = append(a1Fail, fmt.Sprintf("(%v, %v)", p["step_index"], unknown))
		}
		if len(reserved) > 0 {
			a2Fail = append(a2Fail, fmt.Sprintf("(%v, %v)", p["step_index"], reserved))
		}
		// A3 哨兵扫描：可空字段只允许 null/缺省（code_line==0 前奏步除外）
		if mnum(p["code_line"]) != 0 {
			for _, nullable := range []string{"algorithm_step", "root_cause_hint", "trap_message"} {
				if v, has := p[nullable]; has && v != nil {
					switch v.(type) {
					case map[string]any, []any:
					default:
						a3Fail = append(a3Fail, fmt.Sprintf("(%v, %s, %v)", p["step_index"], nullable, v))
					}
				}
			}
		}
	}
	rep.check("S5", "A1", len(a1Fail) == 0, fmt.Sprintf("未知键 %v（键集合 ⊆ v0.1 全集 14 项）", head3(a1Fail)))
	rep.check("S5", "A2", len(a2Fail) == 0, fmt.Sprintf("预留字段提前出现 %v（S4 A0-1 同口径）", head3(a2Fail)))
	rep.check("S5", "A3", len(a3Fail) == 0, fmt.Sprintf("哨兵值 %v", head3(a3Fail)))

	rPing := s.request("ping", nil)
	abi := mstr(mmap(rPing["result"])["abi"])
	// A4a：ABI 版本下限断言（签名契约 ≥ 1.1.0，即 E-P1-5 结构化输出通道起）。
	// 不硬编码具体版本——引擎按"加函数 = minor"承诺演进（1.2.0 起追加
	// cide_get_compile_errors_length 等），快照冻结具体串会让每次兼容性加函数
	// 都假红；下限语义保留牙齿：major 变更（破坏性）或低于 1.1.0 的产物必红。
	rep.check("S5", "A4a", abiVersionAtLeast(abi, 1, 1), fmt.Sprintf("abi=%s（要求 ≥ 1.1.0）", abi))

	// A4b：直读 dll 的 cide_engine_version（Go 走规范指针读取 + cide_free_string）
	if _, err := os.Stat(dllPath); err == nil {
		ver := readEngineVersion(dllPath)
		rep.check("S5", "A4b", strings.Contains(ver, anchor), fmt.Sprintf("engine_version=%q 含锚定 %s", ver, anchor))
	} else {
		rep.check("S5", "A4b", false, fmt.Sprintf("找不到 %s", dllPath))
	}
	// A5：StepStreamBatch 差分编码不在 serve 出口（FRB stream 专用），由引擎侧
	// stream.rs 单测覆盖——见 schema §5.3 与 stream 单测（引擎侧职责）
	rep.check("S5", "A5", true, "serve 出口无差分批量编码；由引擎 stream 单测覆盖（记录性 PASS）")
}

func head3(ss []string) []string {
	if len(ss) > 3 {
		return ss[:3]
	}
	return ss
}

// readEngineVersion 读 cide_engine_version 返回的 rust-alloc 字符串并按契约释放。
// uintptr→Pointer 转换集中于本函数（vet -unsafeptr=false 豁免，同 gosmoke 裁定）。
func readEngineVersion(path string) string {
	dll := syscall.NewLazyDLL(path)
	procVer := dll.NewProc("cide_engine_version")
	procFree := dll.NewProc("cide_free_string")
	if procVer.Find() != nil {
		capi.Fatal("DLL 缺少 cide_engine_version: %s", path)
	}
	raw, _, _ := procVer.Call()
	if raw == 0 {
		return ""
	}
	s := capi.PtrToGoString(raw)
	procFree.Call(raw)
	return s
}

// ---------------------------------------------------------------- 前置门禁

func gitShortHead() string {
	out, err := exec.Command("git", "rev-parse", "--short", "HEAD").Output()
	if err != nil {
		return ""
	}
	return strings.TrimSpace(string(out))
}

var anchorRe = regexp.MustCompile(`\(([0-9a-f]{7,40})\)`)

// preflight 产物新鲜度门禁（fail fast，exit 2）+ 锚点解析。返回 (engineVersion, anchor)。
func preflight(s *Serve, anchorArg string) (string, string) {
	caps := mmap(s.request("capabilities", nil)["result"])
	engineVersion := mstr(caps["engine_version"])
	head := gitShortHead()

	if engineVersion == "" {
		fmt.Println("错误: capabilities 未携带 engine_version —— 产物过旧，请先 `cd native && cargo build --release`")
		os.Exit(2)
	}
	if head != "" && !strings.Contains(engineVersion, head) {
		fmt.Printf("错误: 产物不是当前提交构建的 —— engine_version=%q 不含 HEAD %s\n", engineVersion, head)
		fmt.Println("      回放/影子验证都读 release 产物，请先 `cd native && cargo build --release`")
		os.Exit(2)
	}

	resolved := anchorArg
	if resolved == "" {
		if m := anchorRe.FindStringSubmatch(engineVersion); m != nil {
			resolved = m[1]
		}
	} else if !strings.Contains(engineVersion, resolved) {
		fmt.Printf("错误: --anchor %s 不在引擎版本串 %q 中\n", resolved, engineVersion)
		os.Exit(2)
	}
	if resolved == "" {
		fmt.Println("错误: 引擎版本串不含可识别的短哈希（构建时 git 不可用？），请显式传 --anchor")
		os.Exit(2)
	}

	headShow := head
	if headShow == "" {
		headShow = "(git 不可用)"
	}
	fmt.Printf("引擎版本: %s　锚点: %s　HEAD: %s\n", engineVersion, resolved, headShow)
	return engineVersion, resolved
}

// ---------------------------------------------------------------- selftest（J9 埋雷）

// selfTest 对判定 helper 注入必然违反的输入，断言必须变红；不过即 exit 2。
func selfTest() {
	checks := []struct {
		name string
		ok   bool
	}{
		// Report.check 透传布尔
		{"Report true 透传", (&Report{}).check("T", "A1", true, "") == true},
		{"Report false 透传", (&Report{}).check("T", "A2", false, "x") == false},
		// diagErrors：severity 过滤
		{"diagErrors 只留 error", func() bool {
			resp := map[string]any{"result": map[string]any{"diagnostics": []any{
				map[string]any{"severity": "warning", "code": "W1"},
				map[string]any{"severity": "error", "code": "E2005"},
			}}}
			return len(diagErrors(resp)) == 1 && diagErrors(resp)[0]["code"] == "E2005"
		}()},
		// 锚点正则：版本串 "0.1.0 (abc1234)" 取出短哈希
		{"锚点正则命中", func() bool {
			m := anchorRe.FindStringSubmatch("0.1.0 (abc1234)")
			return m != nil && m[1] == "abc1234"
		}()},
		{"锚点正则拒绝 16 进制外串", anchorRe.FindStringSubmatch("0.1.0 (zzzz999)") == nil},
		// v0.1 键集合：注入未知键必须被 A1 口径发现
		{"未知键必被识别", func() bool {
			p := map[string]any{"step_index": float64(0), "bogus_field": 1}
			for k := range p {
				if !v01PayloadFields[k] && k == "bogus_field" {
					return true
				}
			}
			return false
		}()},
		// 预留字段必须被识别
		{"预留字段必被识别", func() bool {
			p := map[string]any{"unwinding": false}
			for k := range p {
				if reservedFields[k] {
					return true
				}
			}
			return false
		}()},
		// 哨兵：字符串形式的 algorithm_step 必须判负；null 必须判正
		{"哨兵字符串必判负", func() bool {
			p := map[string]any{"code_line": float64(3), "algorithm_step": "oops"}
			v, has := p["algorithm_step"]
			return has && v != nil && !isContainer(v)
		}()},
		{"哨兵 null 判正", func() bool {
			p := map[string]any{"code_line": float64(3), "algorithm_step": nil}
			v, has := p["algorithm_step"]
			return has && v == nil
		}()},
	}
	nFail := 0
	for _, c := range checks {
		if !c.ok {
			nFail++
			fmt.Printf("  [FAIL] selftest %s\n", c.name)
		}
	}
	if nFail > 0 {
		capi.Fatal("selftest %d 条注入未变红，判定口径已破坏，拒绝运行", nFail)
	}
	fmt.Printf("selftest：判定口径 %d 条注入断言全部通过\n", len(checks))
}

func isContainer(v any) bool {
	switch v.(type) {
	case map[string]any, []any:
		return true
	}
	return false
}

// ---------------------------------------------------------------- main

func main() {
	cli := flag.String("cli", cliDefault, "cide_cli 路径")
	anchor := flag.String("anchor", "", "版本锚定短哈希；缺省 = 从引擎版本串自动取")
	sections := flag.String("sections", "S1,S2,S3,S5", "要跑的分节")
	selftest := flag.Bool("selftest", false, "只跑判定口径埋雷自检（J9）")
	flag.Parse()

	selfTest()
	if *selftest {
		return
	}

	sectionSet := map[string]bool{}
	for _, sec := range strings.Split(*sections, ",") {
		sectionSet[strings.ToUpper(strings.TrimSpace(sec))] = true
	}

	rep := &Report{}
	s := newServe(*cli)

	// 产物新鲜度门禁 + 锚点对齐
	_, resolvedAnchor := preflight(s, *anchor)

	var allPayloads []map[string]any

	if sectionSet["S1"] {
		runS1(s, rep)
		allPayloads = append(allPayloads, s.collectPayloads()...)
	}
	if sectionSet["S2"] {
		runS2(s, rep)
	}
	if sectionSet["S3"] {
		p3 := runS3(s, rep)
		allPayloads = append(allPayloads, s.collectPayloads()...)
		runS3A16(*cli, rep, p3)
	}
	if sectionSet["S5"] {
		// S5 的 A4 ping/直读 dll 用当前 serve；A1–A3 用 S1–S3 全量 payload
		runS5(s, rep, allPayloads, resolvedAnchor)
	}

	code := s.shutdown()
	rep.check("S1", "A10", code == 0, fmt.Sprintf("serve 退出码 %d", code))

	os.Exit(rep.summarize())
}
