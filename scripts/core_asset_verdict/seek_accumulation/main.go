//go:build windows

// seek_accumulation —— seek 内存累积判定的 Go 迁移（D5 探针集：判定型）。
//
// 回答的问题：同一会话内反复 seek 之后，内存是否回落？
//   - 每次 seek 后 commit 单调抬升 → 累积泄漏（事故 INCIDENT-2026-09-SEEK-REPLAY-LEAK 形态）；
//   - seek 完成后回落 → 只是瞬时窗口放大（性质不同）。
//
// 同时测 malloc/free 耗时增长指数（50k/100k/200k/400k），独立复核 "O(N²)" 说法。
//
// 与 Python 版（seek_accumulation.py）的差异（记录在案）：
//   - 看门狗击杀用进程句柄直接 Terminate（Python 经 taskkill 子进程）；
//   - 采样口径同（驱动侧 psapi commit_mb，30ms 间隔）；数值是测量值，双轨不要求相等。
//
// 用法：go run ./scripts/core_asset_verdict/seek_accumulation [--cap-mb 1200]
package main

import (
	"cide/scripts/internal/probeutil"

	"bufio"
	"bytes"
	"encoding/json"
	"fmt"
	"math"
	"os"
	"os/exec"
	"path/filepath"
	"strings"
	"sync/atomic"
	"time"
)

var (
	here = probeutil.VerdictDir()
	cli  = probeutil.MustFindCLI()
	work = filepath.Join(here, ".longrun")
)

// ── psapi 采样（口径沿自原 winmem.py，已随 Python 版退役；同款实现在 scripts/internal/probeutil） ──

// ── Session：serve 会话 + 采样看门狗 ──

type session struct {
	cmd     *exec.Cmd
	stdin   ioWriteCloser
	stdout  *bufio.Reader
	stderr  *bytes.Buffer
	pid     int
	rid     int
	killed  bool
	samples []float64
	exited  atomic.Bool
	stop    chan struct{}
}

type ioWriteCloser interface {
	Write(p []byte) (int, error)
	Close() error
}

func newSession(capMB float64) *session {
	s := &session{stop: make(chan struct{})}
	cmd := exec.Command(cli, "serve")
	stdin, _ := cmd.StdinPipe()
	stdout, _ := cmd.StdoutPipe()
	s.stderr = &bytes.Buffer{}
	cmd.Stderr = s.stderr
	if err := cmd.Start(); err != nil {
		fmt.Fprintln(os.Stderr, "FATAL: 无法启动 serve:", err)
		os.Exit(2)
	}
	s.cmd = cmd
	s.pid = cmd.Process.Pid
	s.stdin = stdin.(ioWriteCloser)
	s.stdout = bufio.NewReader(stdout)
	go func() {
		cmd.Wait()
		s.exited.Store(true)
	}()
	// 采样 + 看门狗（30ms，对齐 Python _watch）
	go func() {
		t := time.NewTicker(30 * time.Millisecond)
		defer t.Stop()
		for {
			select {
			case <-s.stop:
				return
			case <-t.C:
				if s.exited.Load() {
					return
				}
				c := probeutil.CommitMBRaw(s.pid)
				if c > 0 {
					s.samples = append(s.samples, c)
					if c > capMB && !s.killed {
						s.killed = true
						probeutil.KillPID(s.pid)
						return
					}
				}
			}
		}
	}()
	return s
}

func (s *session) req(method string, params map[string]any) map[string]any {
	if s.exited.Load() {
		return nil
	}
	s.rid++
	r := map[string]any{"id": s.rid, "method": method}
	if params != nil {
		r["params"] = params
	}
	line, _ := json.Marshal(r)
	if _, err := s.stdin.Write(append(line, '\n')); err != nil {
		s.exited.Store(true)
		return nil
	}
	respLine, err := s.stdout.ReadBytes('\n')
	if err != nil && len(respLine) == 0 {
		s.exited.Store(true)
		return nil
	}
	var resp map[string]any
	if json.Unmarshal(respLine, &resp) != nil {
		return nil
	}
	return resp
}

func (s *session) commit() float64 { return probeutil.CommitMBRaw(s.pid) }

func (s *session) close() string {
	s.stop <- struct{}{}
	s.stdin.Close()
	done := make(chan struct{})
	go func() { s.cmd.Wait(); close(done) }()
	select {
	case <-done:
	case <-time.After(5 * time.Second):
		s.cmd.Process.Kill()
		<-done
	}
	errOut := s.stderr.String()
	if len(errOut) > 400 {
		errOut = errOut[len(errOut)-400:]
	}
	return errOut
}

// ── A：同一会话反复 seek ──

func seekAccumulation(capMB float64) map[string]any {
	iters := 50000
	src := fmt.Sprintf("#include <stdio.h>\nint main(){int s=0;\nfor(int i=0;i<%d;i++){s+=i%%7;}\nprintf(\"%%d\\n\",s);return 0;}\n", iters)
	s := newSession(capMB)
	var timeline []map[string]any
	s.req("compile", map[string]any{"source": src})
	s.req("step.begin", nil)
	for _, t := range []int{100000, 300000, 450000, 200000, 100000, 450000} {
		before := s.commit()
		t0 := time.Now()
		r := s.req("seek", map[string]any{"step": t})
		after := s.commit()
		time.Sleep(300 * time.Millisecond)
		settled := s.commit()
		wallS := probeutil.Sec(time.Since(t0))
		var success any
		if r != nil {
			if m, ok := r["result"].(map[string]any); ok {
				success = m["success"]
			}
		}
		timeline = append(timeline, map[string]any{
			"seek": t, "before_mb": probeutil.Round1(before), "after_mb": probeutil.Round1(after),
			"settled_mb": probeutil.Round1(settled), "wall_s": wallS, "success": success,
		})
		fmt.Printf("  seek(%7d) before=%8.1fMB after=%8.1fMB settled=%8.1fMB (%vs)\n",
			t, before, after, settled, wallS)
	}
	out := map[string]any{
		"iters": iters, "steps_est": iters * 10, "timeline": timeline,
		"killed_by_watchdog": s.killed,
	}
	peak := -1.0
	for _, v := range s.samples {
		if v > peak {
			peak = v
		}
	}
	out["peak_commit_mb"] = probeutil.Round1(peak)
	out["stderr_tail"] = s.close()
	return out
}

// ── B：malloc/free 耗时增长 ──

func mallocTiming() []map[string]any {
	var out []map[string]any
	for _, n := range []int{50000, 100000, 200000, 400000} {
		src := fmt.Sprintf("#include <stdio.h>\n#include <stdlib.h>\nint main(){\nfor(int i=0;i<%d;i++){int* p=(int*)malloc(16); *p=i; free(p);}\nprintf(\"done\\n\");return 0;}\n", n)
		f := filepath.Join(work, fmt.Sprintf("mt_%d.c", n))
		os.WriteFile(f, []byte(src), 0o644)
		t0 := time.Now()
		cmd := exec.Command(cli, "run", f)
		var stdout strings.Builder
		cmd.Stdout = &stdout
		cmd.Stderr = os.Stderr
		runErr := cmd.Run()
		wall := probeutil.Sec(time.Since(t0))
		exit := 0
		if cmd.ProcessState != nil {
			exit = cmd.ProcessState.ExitCode()
		}
		if runErr != nil && exit == 0 {
			exit = -1
		}
		outTail := stdout.String()
		if len(outTail) > 40 {
			outTail = outTail[len(outTail)-40:]
		}
		out = append(out, map[string]any{
			"n": n, "wall_s": wall, "exit": exit, "stdout_tail": outTail,
		})
		fmt.Printf("  malloc n=%7d wall=%6.2fs exit=%d\n", n, wall, exit)
	}
	if len(out) >= 2 {
		a, b := out[0], out[len(out)-1]
		na, nb := float64(a["n"].(int)), float64(b["n"].(int))
		wa, wb := a["wall_s"].(float64), b["wall_s"].(float64)
		out = append(out, map[string]any{
			"exponent_estimate": probeutil.Round3(math.Log(wb/wa) / math.Log(nb/na)),
			"basis":             fmt.Sprintf("%d->%d", a["n"], b["n"]),
		})
	}
	return out
}

func main() {
	capMB := 1200.0
	args := os.Args[1:]
	for i := 0; i+1 < len(args); i++ {
		if args[i] == "--cap-mb" {
			fmt.Sscan(args[i+1], &capMB)
		}
	}
	start := time.Now()
	if err := os.MkdirAll(work, 0o755); err != nil {
		fmt.Fprintln(os.Stderr, "FATAL:", err)
		os.Exit(2)
	}
	fmt.Println("== A. 同一会话反复 seek：峰值 vs 常驻 ==")
	a := seekAccumulation(capMB)
	fmt.Printf("  看门狗触发=%v  峰值提交=%vMB\n", a["killed_by_watchdog"], a["peak_commit_mb"])
	fmt.Println("== B. malloc/free 耗时增长 ==")
	b := mallocTiming()
	data, _ := json.MarshalIndent(map[string]any{"seek": a, "malloc_timing": b}, "", " ")
	p := filepath.Join(here, "seek_accumulation.json")
	if err := os.WriteFile(p, data, 0o644); err != nil {
		fmt.Fprintln(os.Stderr, "FATAL:", err)
		os.Exit(2)
	}
	fmt.Printf("\nJSON 已写出: %s\n耗时: %.1fs\n", p, probeutil.Sec(time.Since(start)))
}
