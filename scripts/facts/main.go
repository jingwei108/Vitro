package main

// 测试数字自动同步 —— 可反复运行（不是一次性脚本）。
//
// 用法（**flag 必须写在子命令之前**——Go flag 在第一个位置参数处停止解析，
// `facts sync --yes` 里的 --yes 会被静默丢弃，脚本会显式报错拦截）：
//
//	go run ./scripts/facts                    # 采集(只读) → 对账 → 交互式逐条同步
//	go run ./scripts/facts check              # CI 门禁：有漂移 exit 1（不改文件）
//	go run ./scripts/facts report             # 只生成报告 reports/doc_fact_drift.md
//	go run ./scripts/facts facts              # 只打印事实台账 reports/facts.json
//
// 常用选项：
//
//	--run         额外执行 replay / serve_smoke（补全它们的真值）
//	--run-slow    再额外执行 cargo test（很慢）
//	--yes         自动应用无警告条目；带警告的（分解式/实测数字/同行多规则）仍跳过
//	--force       连带警告条目也自动应用（会改断分解式/伪造测量记录，慎用）
//	--verbose     报告含冻结（as-of）命中明细
//	--allow-stale check 容忍真值超龄（本地调试逃生门；CI 不得使用）
//	--max-age D   真值新鲜度预算，如 168h / 72h（默认 168h；超龄降级且 check 变红）
//
// 交互按键：y=改这一处  n=跳过  a=全部改  d=看完整行  q=退出

import (
	"bufio"
	"encoding/json"
	"flag"
	"fmt"
	"os"
	"path/filepath"
	"strings"
	"time"
)

func main() {
	fs := flag.NewFlagSet("facts", flag.ExitOnError)
	doRun := fs.Bool("run", false, "额外执行 replay / serve_smoke 以补全真值")
	doRunSlow := fs.Bool("run-slow", false, "再额外执行 cargo test（很慢）")
	verbose := fs.Bool("verbose", false, "报告含冻结（as-of）命中明细")
	autoYes := fs.Bool("yes", false, "自动应用无警告条目（带警告的跳过，--force 才覆盖）")
	forceYes := fs.Bool("force", false, "连同带警告条目也自动应用（慎用：会改断分解式/伪造测量）")
	allowStale := fs.Bool("allow-stale", false, "check 容忍真值超龄（本地调试逃生门，CI 不得使用）")
	maxAge := fs.Duration("max-age", 168*time.Hour, "真值新鲜度预算（超龄降级为待采集且 check 变红）")
	_ = fs.Parse(os.Args[1:])

	// Go flag 在第一个位置参数处停止解析：`facts sync --yes` 的 --yes 会被当作
	// 位置参数静默丢弃、自动降级为交互模式（曾实测：--yes 完全空转、已改 0）。
	// 残留 flag 形态的参数必须 fail loud，不许静默吞掉。
	for _, a := range fs.Args() {
		if strings.HasPrefix(a, "-") && a != "-" {
			fatal(fmt.Sprintf(
				"参数 %q 未被解析（flag 必须写在子命令之前，如 `go run ./scripts/facts --yes sync`）", a))
		}
	}

	cmd := "sync"
	if fs.NArg() > 0 {
		cmd = fs.Arg(0)
	}

	root, err := os.Getwd()
	if err != nil {
		fatal("无法获取当前目录: " + err.Error())
	}
	if _, err := os.Stat(filepath.Join(root, "go.mod")); err != nil {
		fatal("请在项目根目录运行（找不到 go.mod）")
	}

	factsPath := filepath.Join(root, "reports", "facts.json")
	reportPath := filepath.Join(root, "reports", "doc_fact_drift.md")

	// ── 采集（仅读产物；--run 才实际执行防线）──
	// 沿用上次采集值，避免每次运行都要重跑 replay / serve_smoke。
	var prev *FactsDoc
	if b, err := os.ReadFile(factsPath); err == nil {
		var d FactsDoc
		if json.Unmarshal(b, &d) == nil {
			prev = &d
		}
	}
	doc := collectAll(root, *doRun, *doRunSlow, prev)
	// 先写盘**原始采集值**，再做超龄降级（降级只作用于内存中本轮判定的 doc）。
	// 若把 value:null 写回 facts.json，replay/serve_smoke 这类靠 prev-fallback
	// 沿用的真值会不可逆丢失——超龄一次就必须 --run 重采（曾实测踩坑）。
	if err := writeFacts(doc, factsPath); err != nil {
		fatal("写 facts.json 失败: " + err.Error())
	}

	if cmd == "facts" {
		fmt.Println("事实台账已写入: reports/facts.json")
		printFacts(doc)
		return
	}

	// 真值新鲜度门禁：as_of 超过预算的真值降级为待采集（不参与漂移判定），
	// 防止用陈旧真值判新文档、乃至把错数字 sync 进文档。
	staleN := demoteStale(doc, *maxAge)

	// ── 对账 ──
	res := auditDocs(root, doc)
	if err := os.MkdirAll(filepath.Dir(reportPath), 0o755); err != nil {
		fatal(err.Error())
	}
	if err := os.WriteFile(reportPath, []byte(renderReport(root, doc, res, *verbose)), 0o644); err != nil {
		fatal("写报告失败: " + err.Error())
	}

	summarize(res, doc)
	if staleN > 0 {
		if *allowStale {
			fmt.Printf("  [真值超龄] %d 项 as_of 超过 %s，已降级为待采集（--allow-stale：本轮容忍；加 --run 刷新）\n",
				staleN, *maxAge)
		} else {
			fmt.Printf("  [真值超龄] %d 项 as_of 超过 %s，已降级为待采集——check 将 exit 1（加 --run 刷新）\n",
				staleN, *maxAge)
		}
	}

	switch cmd {
	case "check":
		// 超龄即红（曾实测假绿：--max-age 1ns → 真值全超龄 → 漂移 0 → exit 0，
		// 文档漂移一个没修 CI 照样绿——降级保护了 sync 却给 check 开了后门）。
		// --allow-stale 是本地调试逃生门，CI 不得使用。坏引用同理：文档指向
		// 不存在的脚本 = 文档已失效，必须红。
		if res.DriftN > 0 || len(res.Broken) > 0 || (staleN > 0 && !*allowStale) {
			os.Exit(1)
		}
	case "report":
		return
	case "sync":
		if res.DriftN == 0 {
			fmt.Println("所有 CURRENT 数字与真值一致，无需同步。")
			return
		}
		interactiveSync(root, res, *autoYes, *forceYes)
	default:
		fatal("未知子命令: " + cmd + "（可用: sync / check / report / facts）")
	}
}

func fatal(msg string) {
	fmt.Fprintln(os.Stderr, "错误: "+msg)
	os.Exit(2)
}

func summarize(res AuditResult, doc FactsDoc) {
	fmt.Printf("\n扫描 %d 份文档：漂移 %d 处 / 冻结 %d 处 / 人工维护 %d 处 / 待采集 %d 处 / 坏引用 %d 处\n",
		res.ScanN, res.DriftN, res.FrozenN, res.ManualN, res.PendingN, len(res.Broken))
	for _, a := range res.Audits {
		if len(a.Drift) > 0 {
			first := a.Drift[0]
			fmt.Printf("  [漂移] %s: 真值 %d，%d 处不一致（如 %s:%d = %d）\n",
				a.Rule.Label, *a.Truth, len(a.Drift), first.File, first.LineNo, first.Value())
		}
	}
	for _, a := range res.Audits {
		if len(a.Manual) > 0 {
			first := a.Manual[0]
			fmt.Printf("  [人工维护] %s: %d 处（分解式/实测行，子项与总数机判不区分）如 %s:%d\n",
				a.Rule.Label, len(a.Manual), first.File, first.LineNo)
		}
	}
	for _, r := range res.Broken {
		fmt.Printf("  [坏引用] %s:%d → %s（文件不存在）\n", r.File, r.LineNo, r.Path)
	}
	for _, a := range res.Audits {
		if a.Truth == nil && len(a.Pending) > 0 {
			fmt.Printf("  [待采集] %s: %d 处引用无真值 —— 用 --run 补（见 facts.json 的 how_to_get）\n",
				a.Rule.Label, len(a.Pending))
		}
	}
	if ck := cachedKeys(doc); len(ck) > 0 {
		fmt.Printf("  [沿用上次采集] %s（加 --run 可刷新）\n", strings.Join(ck, "、"))
	}
	fmt.Printf("报告: reports/doc_fact_drift.md\n")
}

func truncate(s string, n int) string {
	r := []rune(s)
	if len(r) <= n {
		return s
	}
	return string(r[:n]) + "…"
}

// needsHumanReview 判定一条漂移是否必须人工确认后才能改写。
// 警告来源（audit.go 生成）：
//   - 数字绑定了实测结果（耗时/加速比…）——直接替换会伪造测量记录；
//   - 数字分解式（a + b + c）——只换总数会让算式不成立；
//   - 同一行被多条规则命中——逐条替换会交叉污染（曾实测：E2E 的 78 被
//     shadow 的真值 97 覆盖）。
//
// --yes 模式跳过这些条目（计 skipped），--force 才会覆盖。
func needsHumanReview(h Hit) bool { return h.Warn != "" || len(h.Cooccur) > 0 }

// interactiveSync 逐条弹出，输入字母确认后改写文件。
// autoYes：无警告条目自动应用；forceYes：连同警告条目也自动应用。
func interactiveSync(root string, res AuditResult, autoYes, forceYes bool) {
	hits := res.allDrift()
	reader := bufio.NewReader(os.Stdin)
	// 启动即自动（--yes/--force）；交互中按 a 只把"无警告条目"转为自动，
	// 带警告的仍逐条询问（保留人工否决权）。
	autoMode := autoYes || forceYes
	overrideWarn := forceYes
	applyAll := autoMode
	applied, skipped, failed := 0, 0, 0

	fmt.Printf("\n逐条确认（共 %d 处）。按键: [y] 改  [n] 跳过  [a] 全部改  [d] 详情  [q] 退出\n", len(hits))
	if forceYes {
		fmt.Println("（--force：连同警告条目全部自动应用）")
	} else if autoYes {
		fmt.Println("（--yes：无警告条目自动应用，带警告的仍会逐条询问）")
	}

	quit := false
	for i, h := range hits {
		if quit {
			break
		}
		truth := res.TruthOf[h.Key]
		fmt.Printf("\n[%d/%d] %s\n", i+1, len(hits), labelOf(res, h.Key))
		fmt.Printf("  %s:%d\n", h.File, h.LineNo)
		fmt.Printf("  %d → %d %s\n", h.Value(), truth, unitOf(res, h.Key))
		fmt.Printf("  > %s\n", truncate(h.Text, 150))
		if h.Warn != "" {
			fmt.Printf("  ⚠ %s\n", h.Warn)
		}
		if len(h.Cooccur) > 0 {
			fmt.Printf("  ⚠ 同一行还含: %s —— 改完请人工核对整行\n", strings.Join(h.Cooccur, " / "))
		}

		// 自动应用仅覆盖无警告条目；--force 覆盖一切。
		// 交互中按 a 触发的 applyAll 不 overrideWarn——警告条目回落到询问。
		if applyAll && (overrideWarn || !needsHumanReview(h)) {
			if err := applyHit(root, h, truth); err != nil {
				fmt.Printf("  ✗ 失败: %v\n", err)
				failed++
				continue
			}
			applied++
			fmt.Println("  ✓ 已改")
			continue
		}
		if applyAll && autoMode && needsHumanReview(h) {
			// --yes 遇到警告条目：不静默跳过，明示留给人审
			skipped++
			fmt.Println("  – 带警告，--yes 不自动改（--force 强制 / 交互运行逐条确认）")
			continue
		}

		for {
			fmt.Print("  [y] 改  [n] 跳过  [a] 全部改  [d] 详情  [q] 退出 > ")
			line, err := reader.ReadString('\n')
			if err != nil && strings.TrimSpace(line) == "" {
				// stdin 关闭（非交互）：保守退出，不动文件
				fmt.Println("\n（标准输入已结束，停止同步）")
				quit = true
				break
			}
			switch strings.ToLower(strings.TrimSpace(line)) {
			case "y", "Y":
				if err := applyHit(root, h, truth); err != nil {
					fmt.Printf("  ✗ 失败: %v\n", err)
					failed++
				} else {
					applied++
					fmt.Println("  ✓ 已改")
				}
			case "n", "":
				skipped++
				fmt.Println("  – 跳过")
			case "a":
				applyAll = true
				if err := applyHit(root, h, truth); err != nil {
					fmt.Printf("  ✗ 失败: %v\n", err)
					failed++
				} else {
					applied++
					fmt.Println("  ✓ 已改（后续无警告条目自动应用，带警告的仍会询问）")
				}
			case "d":
				fmt.Printf("\n  完整行:\n  %s\n", h.Text)
				continue
			case "q":
				quit = true
				fmt.Println("  – 退出")
			default:
				continue
			}
			break
		}
	}

	fmt.Printf("\n同步结束：已改 %d / 跳过 %d / 失败 %d\n", applied, skipped, failed)
	if applied > 0 {
		fmt.Println("建议复查: git diff --stat  （数字改动应只落在被确认的行）")
	}
}
