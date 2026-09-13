use crate::session::Session;
use crate::unified::collector::{self, StepCollector};
use crate::unified::trace_analyzer::TraceAnalyzer;
use crate::unified::types::{AutoStepResult, SeekResult, StepMeta, StepPayload};
use crate::vm::core::{CideVM, StepResult};
use crate::vm::snapshot::VMSnapshot;
use cide_vm::snapshot::CheckpointManager;

/// 统一模式引擎：整合检查点管理、数据收集和批量执行。
///
/// 不直接持有 VM 或 Session，而是在方法调用时通过参数传入，
/// 以便与 `flutter_bridge.rs` 的 Session 管理模式兼容。
pub struct UnifiedEngine {
    pub checkpoints: CheckpointManager,
    pub frame_cache: Vec<StepPayload>,
    pub max_steps: i32,
    pub is_paused: bool,
    pub is_cancelled: bool,
    /// 复用的 pre-step 快照容器，避免 `run_batch` 每步分配 1MB Vec。
    pre_step_snap: Option<VMSnapshot>,
    /// FrameCache 滑动窗口大小。超过此值时丢弃最早的帧。
    frame_cache_window_size: usize,
    /// 每次超出窗口时丢弃的比例（如 0.2 表示丢弃最早的 20%）。
    frame_cache_trim_ratio: f64,
    /// 当前 frame_cache[0] 对应的实际步号。
    pub frame_cache_start_step: i32,
}

impl Default for UnifiedEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl UnifiedEngine {
    pub fn new() -> Self {
        Self::with_max_steps(100_000)
    }

    /// 使用自定义最大步数限制创建引擎。
    ///
    /// `max_steps` 用于防止无限循环耗尽资源；教学场景中的长程序可通过此接口
    /// 获得更大的执行预算。
    pub fn with_max_steps(max_steps: i32) -> Self {
        Self {
            checkpoints: CheckpointManager::new(20),
            frame_cache: Vec::new(),
            max_steps,
            is_paused: false,
            is_cancelled: false,
            pre_step_snap: None,
            frame_cache_window_size: 2_000,
            frame_cache_trim_ratio: 0.2,
            frame_cache_start_step: 0,
        }
    }

    /// 轻量级语义标签推断（仅基于源码行，不访问变量值）。
    /// 用于智能检查点判断，避免每步都计算完整语义标签。
    pub fn reset(&mut self) {
        self.checkpoints.clear();
        self.frame_cache.clear();
        self.is_paused = false;
        self.is_cancelled = false;
        self.pre_step_snap = None;
        self.frame_cache_start_step = 0;
    }

    /// 返回当前 frame_cache 窗口起始步号。
    pub fn frame_cache_start_step(&self) -> i32 {
        self.frame_cache_start_step
    }

    /// 对 frame_cache 执行滑动窗口截断。
    /// 超过窗口上限时丢弃最早的 `trim_ratio` 比例帧，并更新窗口起点。
    pub(crate) fn trim_frame_cache(&mut self) {
        if self.frame_cache.len() <= self.frame_cache_window_size {
            return;
        }
        let discard = ((self.frame_cache.len() as f64) * self.frame_cache_trim_ratio).ceil() as usize;
        let discard = discard.max(1).min(self.frame_cache.len());
        self.frame_cache_start_step += discard as i32;
        self.frame_cache = self.frame_cache.split_off(discard);
    }

    pub fn pause(&mut self) {
        self.is_paused = true;
    }

    pub fn resume(&mut self) {
        self.is_paused = false;
    }

    pub fn cancel(&mut self) {
        self.is_cancelled = true;
    }

    /// 批量自动执行，返回收集到的 StepPayload 列表。
    ///
    /// 调用者应确保 VM 已初始化（`setup_vm` 已调用）。
    pub fn run_batch(
        &mut self,
        vm: &mut CideVM,
        session: &mut Session,
        batch_size: i32,
    ) -> Result<AutoStepResult, String> {
        let mut payloads = Vec::new();
        let mut finished = false;
        let mut trapped = false;
        let mut waiting_input = false;

        let mut trap_message: Option<String> = None;

        for _ in 0..batch_size {
            if self.is_paused || self.is_cancelled {
                break;
            }

            let step = vm.get_executed_steps();

            // 检查点保存（固定间隔 + 智能边界）
            // R3：语义标签单源——与 StepPayload 共用 collector 的唯一分类器（降级形态）
            let func_name = vm
                .get_call_stack()
                .last()
                .map(|f| f.func_name.clone())
                .unwrap_or_default();
            let at_callee_entry = vm
                .get_call_stack()
                .last()
                .map(|f| f.caller_line == vm.get_current_line())
                .unwrap_or(false);
            let semantic_label = collector::infer_semantic_label(
                vm.get_current_line(),
                None,
                &func_name,
                session,
                at_callee_entry,
            );
            let meta = StepMeta {
                code_line: vm.get_current_line(),
                func_name: func_name.clone(),
                loop_depth: 0,
                semantic_label,
            };
            if self.checkpoints.should_checkpoint(step, &meta.semantic_label) {
                self.checkpoints.save(step, vm, &mut session.as_vm_context());
            }

            // 执行前快照：用于 Trap 时自动回退。
            // 复用已有 VMSnapshot 的 1MB buffer，避免每步分配新 Vec。
            let pre_step_snap = self.pre_step_snap.get_or_insert_with(|| vm.snapshot(&session.as_vm_context()));
            vm.snapshot_into(&session.as_vm_context(), pre_step_snap);

            // 执行一步
            match vm.step(&mut session.as_vm_context()) {
                StepResult::Ok => {
                    let payload = StepCollector::collect(vm, session, step);
                    payloads.push(payload);
                }
                StepResult::Paused => {
                    let payload = StepCollector::collect(vm, session, step);
                    payloads.push(payload);
                    self.is_paused = true;
                }
                StepResult::WaitingInput => {
                    let payload = StepCollector::collect(vm, session, step);
                    payloads.push(payload);
                    waiting_input = true;
                    break;
                }
                StepResult::Finished => {
                    let payload = StepCollector::collect(vm, session, step);
                    payloads.push(payload);
                    finished = true;
                    break;
                }
                StepResult::Trap => {
                    // 自动回退到上一步状态
                    vm.restore(pre_step_snap, &mut session.as_vm_context());
                    let mut payload = StepCollector::collect(vm, session, step);

                    // Build full history for trace analysis.
                    let mut history = Vec::with_capacity(self.frame_cache.len() + payloads.len() + 1);
                    history.extend_from_slice(&self.frame_cache);
                    history.extend_from_slice(&payloads);
                    history.push(payload.clone());
                    let trap_step = history.len().saturating_sub(1);

                    if let Some(hint) = TraceAnalyzer::analyze_trap(&history, trap_step, vm.get_error(), session) {
                        payload.root_cause_hint = Some(hint);
                    }

                    payloads.push(payload);
                    trapped = true;
                    trap_message = Some(vm.get_error().to_string());
                    break;
                }
            }

            if step >= self.max_steps {
                return Err(format!("执行步数超过限制（{} 步），可能存在无限循环。", self.max_steps));
            }
        }

        // U1#1 P0-1：同一源码行的多帧中只有**行末帧**（语句完成帧）携带
        // algorithm_step——首帧在赋值发生前，数值是旧值/未初始化哨兵
        // （用户实测三例：binary 首帧"计算中点 mid=0"实际 mid=2、
        // shellSort 首帧"取增量 gap=0"、dijkstra 首帧"顶点 -1"）。
        // 去重必须作用于**返回数组本身**（step.next 客户端看到的是它），
        // 并同步缓存（含与上批尾帧的衔接——批边界切在语句中间时上批
        // 尾帧不再是行末）。
        for i in 1..payloads.len() {
            if payloads[i].code_line == payloads[i - 1].code_line {
                payloads[i - 1].algorithm_step = None;
            }
        }
        if let (Some(tail), Some(first)) = (self.frame_cache.last_mut(), payloads.first()) {
            if tail.code_line == first.code_line {
                tail.algorithm_step = None;
            }
        }
        self.frame_cache.extend(payloads.clone());
        self.trim_frame_cache();

        Ok(AutoStepResult {
            payloads,
            finished,
            trapped,
            waiting_input,
            paused: self.is_paused,
            current_line: vm.get_current_line(),
            trap_message,
            cache_start_step: self.frame_cache_start_step,
        })
    }

    /// Seek 到指定步。
    ///
    /// 如果目标步已在当前 `frame_cache` 窗口中，直接返回；
    /// 否则从最近检查点恢复 VM 并正向重放，然后只保留目标步附近窗口内的帧。
    pub fn seek_to(&mut self, target: i32, vm: &mut CideVM, session: &mut Session) -> SeekResult {
        // 目标已在当前窗口中
        if let Some(idx) = self.frame_cache_index(target) {
            return SeekResult {
                success: true,
                payload: Some(self.frame_cache[idx].clone()),
                error: None,
            };
        }

        // 找到最近检查点（增量快照会在此重建为全量）
        let (checkpoint_step, snap) = match self.checkpoints.nearest(target) {
            Some(v) => v,
            None => {
                return SeekResult {
                    success: false,
                    payload: None,
                    error: Some("没有可用的检查点".to_string()),
                };
            }
        };

        // 恢复 VM 状态
        vm.restore(&snap, &mut session.as_vm_context());

        // 越窗重放前重置帧窗口到检查点步。否则 target < 旧窗口起点时
        // `push_or_replace_in_replay` 的 `step - start_step` 为负，
        // `as usize` 变成天文数字 → 占位填充循环无限 push（实测吃满 63.6GB 内存，
        // schema 签字回放 S3 A15 暴露；检查点锚点固化使该路径首次可达）。
        self.frame_cache.clear();
        self.frame_cache_start_step = checkpoint_step;

        if std::env::var("CIDE_SEEK_DEBUG").is_ok() {
            eprintln!(
                "[seek_debug] cp={checkpoint_step} target={target} cache_start={} len={}",
                self.frame_cache_start_step,
                self.frame_cache.len()
            );
        }

        // 正向重放到目标步（含 target——重放区间排他会把目标步本身留在
        // 窗口之外，随后 frame_cache_index(target) 落空（S3 A15 实测
        // "无法获取目标步的 payload"；checkpoint=0 重放 len=5 只含 0..4））
        for step in checkpoint_step..=target {
            if self.is_cancelled {
                return SeekResult {
                    success: false,
                    payload: None,
                    error: Some("执行已取消".to_string()),
                };
            }

            // 重放过程中动态保存检查点，加速后续 seek
            if step > checkpoint_step && step % self.checkpoints.interval == 0 {
                self.checkpoints.save(step, vm, &mut session.as_vm_context());
            }

            match vm.step(&mut session.as_vm_context()) {
                StepResult::Ok | StepResult::Paused => {
                    let payload = StepCollector::collect(vm, session, step);
                    self.push_or_replace_in_replay(step, payload);
                }
                StepResult::WaitingInput => {
                    let payload = StepCollector::collect(vm, session, step);
                    self.push_or_replace_in_replay(step, payload.clone());
                    self.finish_replay_window(target);
                    return SeekResult {
                        success: true,
                        payload: Some(payload),
                        error: None,
                    };
                }
                StepResult::Finished => {
                    let payload = StepCollector::collect(vm, session, step);
                    self.push_or_replace_in_replay(step, payload.clone());
                    self.finish_replay_window(target);
                    return SeekResult {
                        success: true,
                        payload: Some(payload),
                        error: None,
                    };
                }
                StepResult::Trap => {
                    return SeekResult {
                        success: false,
                        payload: None,
                        error: Some(format!("运行时错误：{}", vm.get_error())),
                    };
                }
            }
        }

        self.finish_replay_window(target);

        if std::env::var("CIDE_SEEK_DEBUG").is_ok() {
            eprintln!(
                "[seek_debug] 重放完成 cache_start={} len={}",
                self.frame_cache_start_step,
                self.frame_cache.len()
            );
        }

        // 返回目标步的 payload
        match self
            .frame_cache_index(target)
            .and_then(|idx| self.frame_cache.get(idx))
            .cloned()
        {
            Some(payload) => SeekResult {
                success: true,
                payload: Some(payload),
                error: None,
            },
            None => SeekResult {
                success: false,
                payload: None,
                error: Some("无法获取目标步的 payload".to_string()),
            },
        }
    }

    /// 将实际步号转换为当前 frame_cache 中的索引。
    pub(crate) fn frame_cache_index(&self, step: i32) -> Option<usize> {
        if step < self.frame_cache_start_step {
            return None;
        }
        let idx = (step - self.frame_cache_start_step) as usize;
        if idx < self.frame_cache.len() {
            Some(idx)
        } else {
            None
        }
    }

    /// U1#1 P0-1：对 frame_cache 的 [from, len) 段执行"行末帧标注保留"——
    /// 与前一帧同 code_line 的帧不是行末（语句尚未完成），其 algorithm_step
    /// 置 None；段的最后一帧（或行切换前的末帧）保留标注（数值为完成态）。
    /// from = 0 时包含与既有缓存尾部的衔接（run_batch 跨批边界场景）。
    fn demote_row_annotations(&mut self, from: usize) {
        for i in from..self.frame_cache.len() {
            if i >= 1 && self.frame_cache[i].code_line == self.frame_cache[i - 1].code_line {
                self.frame_cache[i - 1].algorithm_step = None;
            }
        }
    }

    /// 重放过程中将 payload 放入临时缓存。
    fn push_or_replace_in_replay(&mut self, step: i32, payload: StepPayload) {
        let Ok(idx) = usize::try_from(step - self.frame_cache_start_step) else {
            // 目标步早于窗口起点：越窗重放路径已重置窗口，此为防御分支
            return;
        };
        if idx == self.frame_cache.len() {
            self.frame_cache.push(payload);
            self.demote_row_annotations(self.frame_cache.len() - 1);
        } else if idx < self.frame_cache.len() {
            self.frame_cache[idx] = payload;
        } else {
            // 中间有缺口，用占位填充（正常重放不应出现）
            while self.frame_cache.len() < idx {
                self.frame_cache.push(StepPayload {
                    step_index: self.frame_cache_start_step + self.frame_cache.len() as i32,
                    code_line: 0,
                    func_name: String::new(),
                    semantic_label: String::new(),
                    algorithm_step: None,
                    local_vars: Vec::new(),
                    call_stack: Vec::new(),
                    vis_events: Vec::new(),
                    heatmap_line: 0,
                    heatmap_count: 0,
                    accessed_vars: Vec::new(),
                    array_snapshots: Vec::new(),
                    pointer_snapshots: Vec::new(),
                    root_cause_hint: None,
                });
            }
            self.frame_cache.push(payload);
            self.demote_row_annotations(self.frame_cache.len() - 1);
        }
    }

    /// 重放结束后仅保留目标步附近窗口内的帧。
    fn finish_replay_window(&mut self, target: i32) {
        let end_step = target;
        let want_start = (end_step - self.frame_cache_window_size as i32 + 1).max(0);
        if self.frame_cache_start_step < want_start {
            // R-2026-09-01/02：seek 目标远超已有步数时，理论 discard 大于缓存长度，
            // 直接 split_off 会 panic——钳到实际长度（即清空缓存），并把缓存起点
            // 按实际推进量记账（钳位后 want_start 可能超出真实数据范围）。
            let discard = ((want_start - self.frame_cache_start_step) as usize)
                .min(self.frame_cache.len());
            self.frame_cache_start_step += discard as i32;
            self.frame_cache = self.frame_cache.split_off(discard);
        }
        // 截断 target 之后的多余帧
        let expected_len = (end_step - self.frame_cache_start_step + 1).max(0) as usize;
        self.frame_cache.truncate(expected_len);
    }

    /// 获取指定范围的 StepPayload（按实际步号）。
    /// 只返回当前窗口内存在的部分。
    ///
    /// 参数先钳到 `[start_step, cache_end]` 再转 usize——负 `end` 直接 `as usize`
    /// 会绕回 `usize::MAX`（R-2026-09-03 复发根因：旧代码先 `min` 后 `as`，
    /// `(-1).min(cache_end) = -1` 仍为负），导致切片 panic。
    pub fn get_payloads(&self, start: i32, end: i32) -> Vec<StepPayload> {
        let start_step = self.frame_cache_start_step;
        let cache_end = start_step + self.frame_cache.len() as i32;
        let s = start.max(start_step).min(cache_end);
        let e = end.max(start_step).min(cache_end);
        if s < e {
            let lo = (s - start_step) as usize;
            let hi = (e - start_step) as usize;
            self.frame_cache[lo..hi].to_vec()
        } else {
            Vec::new()
        }
    }

    /// 获取当前缓存窗口中已收集的最大步数。
    /// 若缓存为空，返回 `frame_cache_start_step - 1`。
    pub fn max_collected_step(&self) -> i32 {
        if self.frame_cache.is_empty() {
            self.frame_cache_start_step - 1
        } else {
            self.frame_cache_start_step + self.frame_cache.len() as i32 - 1
        }
    }
}
