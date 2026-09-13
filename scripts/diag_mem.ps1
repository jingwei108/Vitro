# diag_mem.ps1 - 宿主进程内存峰值采样工具（常设版）
#
# 制度来源：docs/current/07-质量与裁定/INCIDENTS/（统一路线图 U0 #3，diag_mem.ps1 转正）。
# 第一起 seek 泄漏事故（63.6GB RSS / 33.9GB 页面文件）的现场诊断靠临时脚本完成且未入库；
# 本脚本是它的常设替代——任何长跑驱动 / serve 冒烟 / 压力用例都可以用它采样峰值，
# 事故现场第一时间有工具可用，不再临时造轮子。
#
# 用法：
#   # 采样一个已存在进程（PID）直到它退出：
#   powershell -NoProfile -File scripts\diag_mem.ps1 -ProcId 12345
#
#   # 启动并采样一条命令（命令 + 参数）：
#   powershell -NoProfile -File scripts\diag_mem.ps1 -FilePath native\target\release\vitro_cli.exe -Args "serve"
#
#   # 带阈值：超过 400MB commit 即告警并以 exit 1 结束（不杀进程，只观测）
#   powershell -NoProfile -File scripts\diag_mem.ps1 -ProcId 12345 -ThresholdMB 400
#
#   # 调整采样间隔（默认 200ms）
#   powershell -NoProfile -File scripts\diag_mem.ps1 -ProcId 12345 -IntervalMs 100
#
# 输出：结束（或 Ctrl+C）后打印峰值 RSS（WorkingSet）与峰值 commit（PagedMemorySize），
# 单位 MB，保留 1 位小数。阈值触发时打印 [ALERT] 行并以 exit 1 结束。
param(
    [int]$ProcId = 0,
    [string]$FilePath = "",
    [string[]]$Args = @(),
    [int]$IntervalMs = 200,
    [int]$ThresholdMB = 0
)

$ErrorActionPreference = "Stop"

$proc = $null
if ($ProcId -gt 0) {
    $proc = Get-Process -Id $ProcId -ErrorAction SilentlyContinue
    if (-not $proc) { Write-Host "[diag_mem] PID $ProcId 不存在或已退出"; exit 2 }
    Write-Host ("[diag_mem] 附加到 PID {0} ({1})" -f $proc.Id, $proc.ProcessName)
} elseif ($FilePath -ne "") {
    $proc = Start-Process -FilePath $FilePath -ArgumentList $Args -PassThru -NoNewWindow
    Write-Host ("[diag_mem] 启动 PID {0}: {1} {2}" -f $proc.Id, $FilePath, ($Args -join " "))
} else {
    Write-Host "[diag_mem] 需要 -ProcId <pid> 或 -FilePath <exe> [-Args ...]"
    exit 2
}

$peakWs = [long]0
$peakCommit = [long]0
$alerted = $false

try {
    while (-not $proc.HasExited) {
        $proc.Refresh()
        $ws = $proc.WorkingSet64
        $commit = $proc.PagedMemorySize64
        if ($ws -gt $peakWs) { $peakWs = $ws }
        if ($commit -gt $peakCommit) { $peakCommit = $commit }
        if ($ThresholdMB -gt 0 -and -not $alerted -and $commit -gt [long]$ThresholdMB * 1MB) {
            $mb = [math]::Round($commit / 1MB, 1)
            Write-Host ("[ALERT] commit {0} MB > 阈值 {1} MB（继续观测，不杀进程）" -f $mb, $ThresholdMB)
            $alerted = $true
        }
        Start-Sleep -Milliseconds $IntervalMs
    }
} finally {
    $proc.Refresh()
    if (-not $proc.HasExited) {
        # 结束采样时进程可能还在跑（Ctrl+C）——以当前值兜底计入峰值
        if ($proc.WorkingSet64 -gt $peakWs) { $peakWs = $proc.WorkingSet64 }
        if ($proc.PagedMemorySize64 -gt $peakCommit) { $peakCommit = $proc.PagedMemorySize64 }
    }
    $exitCode = $proc.ExitCode
    Write-Host ""
    Write-Host ("[diag_mem] 峰值 RSS    = {0,10:N1} MB" -f ($peakWs / 1MB))
    Write-Host ("[diag_mem] 峰值 commit = {0,10:N1} MB" -f ($peakCommit / 1MB))
    if ($alerted) { exit 1 }
    if ($exitCode -is [int] -and $exitCode -ne 0) { exit $exitCode }
}
