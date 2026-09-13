//go:build windows

// resource_longrun —— 资源长跑探针的 Go 迁移（D5 探针集：判定型，U2 验收对象）。
//
// 三条"宿主资源域"路径（规模 realism 参数）：
//  1. seek 重放放大：N 步程序 + 一次远距 seek → 提交内存随步数的斜率
//     （事故 INCIDENT-2026-09-SEEK-REPLAY-LEAK 的机制复现，见裁定文档）；
//  2. malloc/free 循环：regions 登记表只增不删 → 内存与耗时随 N 的变化；
//  3. putchar 1M：output_chunks 每字符一个 String。
//
// 所有子进程带硬内存看门狗（默认 1500MB 即击杀并记录），避免重演 63.6GB 事故。
//
// 与 Python 版（resource_longrun.py）的差异（记录在案）：
//   - 看门狗击杀用 Process.Kill()（Python 经 taskkill 子进程）；
//   - 采样口径同（驱动侧 psapi commit_mb，50ms 间隔）；
//   - 数值（wall_s / commit_mb）是测量值，双轨不要求相等，结构/机制对齐即可。
//
// 用法：go run ./scripts/core_asset_verdict/resource_longrun [--cap-mb 1500]
package main

import (
	"vitro/scripts/internal/probeutil"

	"context"
	"encoding/json"
	"fmt"
	"os"
	"os/exec"
	"path/filepath"
	"strings"
	"sync"
	"time"
)

var (
	here = probeutil.VerdictDir()
	cli  = probeutil.MustFindCLI()
	work = filepath.Join(here, ".longrun")
)

// ── Sampler：驱动侧采样 + 硬看门狗 ──

type sampler struct {
	peak    float64
	samples []float64
	killed  bool
	stop    chan struct{}
	done    chan struct{}
}

func newSampler(pid int, capMB float64) *sampler {
	s := &sampler{stop: make(chan struct{}), done: make(chan struct{})}
	go func() {
		defer close(s.done)
		t := time.NewTicker(50 * time.Millisecond)
		defer t.Stop()
		for {
			select {
			case <-s.stop:
				return
			case <-t.C:
				c := probeutil.CommitMBRaw(pid)
				if c > 0 {
					if c > s.peak {
						s.peak = c
					}
					s.samples = append(s.samples, c)
					if c > capMB && !s.killed {
						s.killed = true
						probeutil.KillPID(pid)
						return
					}
				}
			}
		}
	}()
	return s
}
func (s *sampler) finish() float64 {
	close(s.stop)
	<-s.done
	return s.peak
}

// ── sample_run：非交互子进程 + 采样 ──

type runRecord struct {
	WallS        float64 `json:"wall_s"`
	Exit         int     `json:"exit"`
	CommitPeakMB float64 `json:"commit_peak_mb"`
	PeakPsapiMB  float64 `json:"peak_psapi_mb"`
	KilledByWD   bool    `json:"killed_by_watchdog"`
	StdoutTail   string  `json:"stdout_tail"`
	StderrTail   string  `json:"stderr_tail"`
	Case         string  `json:"case,omitempty"`
	EstSteps     int     `json:"est_steps,omitempty"`
	MBPer1kSteps float64 `json:"mb_per_1k_steps,omitempty"`
	N            int     `json:"n,omitempty"`
}

func sampleRun(cmdArgs []string, timeout time.Duration, capMB float64, stdinLines []string) runRecord {
	start := time.Now()
	cmd := exec.Command(cmdArgs[0], cmdArgs[1:]...)
	var stdout, stderr syncBuffer
	if len(stdinLines) > 0 {
		cmd.Stdin = strings.NewReader(strings.Join(stdinLines, "\n") + "\n")
	} else {
		cmd.Stdin = nil // DEVNULL 语义（exec 默认 /dev/null）
	}
	cmd.Stdout = &stdout
	cmd.Stderr = &stderr
	if err := cmd.Start(); err != nil {
		return runRecord{WallS: probeutil.Sec(time.Since(start)), Exit: -1, CommitPeakMB: -1, PeakPsapiMB: -1,
			StderrTail: probeutil.Tail(err.Error(), 200)}
	}
	s := newSampler(cmd.Process.Pid, capMB)

	ctx, cancel := context.WithTimeout(context.Background(), timeout)
	go func() {
		<-ctx.Done()
		if ctx.Err() != nil {
			cmd.Process.Kill()
		}
	}()
	waitErr := cmd.Wait()
	cancel()
	peak := s.finish()
	exitCode := 0
	if cmd.ProcessState != nil {
		exitCode = cmd.ProcessState.ExitCode()
	}
	_ = waitErr
	return runRecord{
		WallS:        probeutil.Sec(time.Since(start)),
		Exit:         exitCode,
		CommitPeakMB: probeutil.Round2(peak),
		PeakPsapiMB:  probeutil.Round2(probeutil.PeakCommitMBRaw(cmd.Process.Pid)),
		KilledByWD:   s.killed,
		StdoutTail:   probeutil.Tail(stdout.String(), 200),
		StderrTail:   probeutil.Tail(stderr.String(), 200),
	}
}

type syncBuffer struct {
	mu  sync.Mutex
	buf strings.Builder
}

func (b *syncBuffer) Write(p []byte) (int, error) {
	b.mu.Lock()
	defer b.mu.Unlock()
	return b.buf.Write(p)
}

func (b *syncBuffer) String() string {
	b.mu.Lock()
	defer b.mu.Unlock()
	return b.buf.String()
}

// ── 三个 case ──

func seekScaling(capMB float64) []runRecord {
	var out []runRecord
	for _, iters := range []int{5000, 20000, 50000} {
		src := fmt.Sprintf("#include <stdio.h>\nint main(){int s=0;\nfor(int i=0;i<%d;i++){s+=i%%7;}\nprintf(\"%%d\\n\",s);return 0;}\n", iters)
		nEst := iters * 10
		os.WriteFile(filepath.Join(work, fmt.Sprintf("seek_%d.c", iters)), []byte(src), 0o644)
		reqs := []map[string]any{
			{"id": 1, "method": "compile", "params": map[string]any{"source": src}},
			{"id": 2, "method": "step.begin"},
			{"id": 3, "method": "seek", "params": map[string]any{"step": nEst - 100}},
		}
		var lines []string
		for _, r := range reqs {
			b, _ := json.Marshal(r)
			lines = append(lines, string(b))
		}
		r := sampleRun([]string{cli, "serve"}, 300*time.Second, capMB, lines)
		r.Case = fmt.Sprintf("seek_iters%d", iters)
		r.EstSteps = nEst
		r.MBPer1kSteps = probeutil.Round2(r.CommitPeakMB / float64(maxInt(1, nEst)) * 1000)
		out = append(out, r)
		fmt.Printf("  seek iters=%d (~%d 步) 峰值提交 %vMB  %vMB/千步  墙钟 %vs  watchdog=%v\n",
			iters, nEst, r.CommitPeakMB, r.MBPer1kSteps, r.WallS, r.KilledByWD)
	}
	return out
}

func mallocScaling(capMB float64) []runRecord {
	var out []runRecord
	for _, n := range []int{100_000, 300_000, 1_000_000} {
		src := fmt.Sprintf("#include <stdio.h>\n#include <stdlib.h>\nint main(){\nfor(int i=0;i<%d;i++){int* p=(int*)malloc(16); *p=i; free(p);}\nprintf(\"done\\n\");return 0;}\n", n)
		f := filepath.Join(work, fmt.Sprintf("malloc_%d.c", n))
		os.WriteFile(f, []byte(src), 0o644)
		r := sampleRun([]string{cli, "run", f}, 600*time.Second, capMB, nil)
		r.Case = fmt.Sprintf("malloc_%d", n)
		r.N = n
		out = append(out, r)
		fmt.Printf("  malloc n=%9d 峰值提交 %vMB  墙钟 %vs  exit=%d  watchdog=%v\n",
			n, r.CommitPeakMB, r.WallS, r.Exit, r.KilledByWD)
	}
	return out
}

func putcharRun(capMB float64) runRecord {
	src := "#include <stdio.h>\nint main(){for(int i=0;i<1000000;i++)putchar(97);\nprintf(\"\\n\");return 0;}\n"
	f := filepath.Join(work, "putchar_1m.c")
	os.WriteFile(f, []byte(src), 0o644)
	r := sampleRun([]string{cli, "run", f}, 600*time.Second, capMB, nil)
	r.Case = "putchar_1m"
	fmt.Printf("  putchar 1M 峰值提交 %vMB  墙钟 %vs  watchdog=%v\n", r.CommitPeakMB, r.WallS, r.KilledByWD)
	return r
}

func maxInt(a, b int) int {
	if a > b {
		return a
	}
	return b
}

func main() {
	capMB := 1500.0
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
	fmt.Printf("vitro_cli: %s\n看门狗上限 %vMB\n", cli, capMB)
	fmt.Println("== 1. seek 重放放大 ==")
	a := seekScaling(capMB)
	fmt.Println("== 2. malloc/free (regions 登记表) ==")
	b := mallocScaling(capMB)
	fmt.Println("== 3. putchar 1M (output_chunks) ==")
	c := putcharRun(capMB)
	out := map[string]any{"cap_mb": capMB, "seek": a, "malloc": b, "putchar": c}
	data, _ := json.MarshalIndent(out, "", " ")
	p := filepath.Join(here, "resource_longrun.json")
	if err := os.WriteFile(p, data, 0o644); err != nil {
		fmt.Fprintln(os.Stderr, "FATAL:", err)
		os.Exit(2)
	}
	fmt.Printf("\nJSON 已写出: %s\n耗时: %.1fs\n", p, probeutil.Sec(time.Since(start)))
}
