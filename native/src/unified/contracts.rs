//! schema 版本轨道与**行为契约**（B2：预留位 → 激活的既定轨道）。
//!
//! 本模块回答三个问题，且答案都在代码里（而非只在文档里）：
//!
//! 1. **v0.1 冻结了什么**：`SCHEMA_VERSION` + `RESERVED_FIELDS_V0_2`（预留位字段名
//!    集合，v0.1 阶段不得出现在任何 payload 中，见 SharpTutor S5 §1 A2）；
//! 2. **v0.2 要装什么**：`V0_2_FIELD_LEDGER`（字段台账）+ `V0_2_ACTIVATION_CHECKLIST`
//!    （激活清单）——激活提交必须走完清单，冻结测试会**主动失败**提醒（tripwire）；
//! 3. **哪些行为不许被性能优化毁掉**：`BEHAVIOR_CONTRACTS`（行为契约表）+
//!    `check_unwinding_granularity()`（"UNWINDING 不得合并单步"的可执行判据）。
//!
//! 文档锚点：`docs/spec/STEP_PAYLOAD_SCHEMA_V0_1.md` §7.x（预留位）/ §9（v0.2 轨道）/
//! 附录 B（词汇表）；契约出处：`CSharp前端引入计划.md` §6-B、SharpTutor S4 §5。

/// 当前已冻结的 schema 版本。
pub const SCHEMA_VERSION: &str = "v0.1";

/// v0.1 冻结时间与签字依据（S1–S5 签字回放 61/61）。
pub const SCHEMA_V0_1_FROZEN_AT: &str = "2026-09-12";

/// v0.2 预留位字段（异常域，随 v0.1 冻结以"预留位"写入，**激活前不得出现在任何 payload 中**）。
///
/// 顺序与 schema §7.x 表格一致；冻结测试按此断言"预留位缺省"。
pub const RESERVED_FIELDS_V0_2: [&str; 4] = [
    "handler_depth",
    "unwinding",
    "unwind_frames_left",
    "current_exception",
];

/// v0.2 激活清单（**激活提交必须逐条走完**）。
///
/// 这不是"建议流程"——`step_payload_schema_v0_1_test::test_v0_1_reserved_fields_absent`
/// 一旦发现 payload 里出现预留位字段就会失败并把本清单打印出来，迫使激活者显式改测试；
/// 改测试的同时必须完成清单第 2–4 条（文档 §7 校验表 + 重跑 C1–C3/S1–S5）。
pub const V0_2_ACTIVATION_CHECKLIST: [&str; 5] = [
    "① 只增事件：不得改动 v0.1 既有 14 字段的名称/类型/语义；预留位激活一律以新增字段形态落地（消费方须忽略未知字段）",
    "② 同步更新 docs/spec/STEP_PAYLOAD_SCHEMA_V0_1.md：§9 台账状态 active + §7 校验表追加 v0.2 历史行",
    "③ 重跑引擎侧 C1–C3：cargo test --test step_payload_schema_v0_1_test / --test unified_engine_window_test + go run ./scripts/serve_smoke",
    "④ 重跑签字回放 S1–S5：go run ./scripts/replay/replay_s1_s5.go --anchor <新短哈希>（异常域还需 S4 §5 激活契约 A1–A8）",
    "⑤ 解除冻结测试中的预留位断言（v0.1 断言 → v0.2 断言），并知会下游按容忍矩阵回归 T4 投影",
];

/// v0.2 字段台账一条。
#[derive(Debug, Clone, serde::Serialize)]
pub struct SchemaFieldPlan {
    pub field: &'static str,
    /// `reserve`（v0.1 已预留，激活即刻可用）/ `add`（v0.2 新增字段）
    pub kind: &'static str,
    /// `pending`（未激活）/ `active`（已落地）
    pub status: &'static str,
    /// 激活批次
    pub batch: &'static str,
    pub note: &'static str,
}

/// v0.2 字段台账（文档 §9 的机器可读单源）。
pub const V0_2_FIELD_LEDGER: &[SchemaFieldPlan] = &[
    SchemaFieldPlan {
        field: "handler_depth",
        kind: "reserve",
        status: "pending",
        batch: "CS3b",
        note: "当前受几层 try 保护；与 unwinding 正交（finally 步 handler_depth==0 但仍在展开）",
    },
    SchemaFieldPlan {
        field: "unwinding",
        kind: "reserve",
        status: "pending",
        batch: "CS3b",
        note: "展开态显式标记；每帧一 step，不得合并（见 BEHAVIOR_CONTRACTS）",
    },
    SchemaFieldPlan {
        field: "unwind_frames_left",
        kind: "reserve",
        status: "pending",
        batch: "CS3b",
        note: "剩余待展开帧数，展开动画驱动字段",
    },
    SchemaFieldPlan {
        field: "current_exception",
        kind: "reserve",
        status: "pending",
        batch: "CS3b",
        note: "{type_name, message, addr, origin_line}|null；origin_line 为原始抛点（`throw;` 保留 / `throw e;` 改写）",
    },
    SchemaFieldPlan {
        field: "code_file",
        kind: "add",
        status: "pending",
        batch: "v0.2",
        note: "多文件行号归位（§8 #9）：code_line 保持全局行号语义，另附文件名，避免破坏断点/heatmap 口径",
    },
    SchemaFieldPlan {
        field: "call_stack[].return_line",
        kind: "add",
        status: "pending",
        batch: "v0.2",
        note: "补全既有字段值（§8 #1）：字段已在 v0.1 中恒为 0，激活 = 记录 VM 帧返回行，属**补值不改字段**",
    },
    SchemaFieldPlan {
        field: "func_display_name / func_mangled_name",
        kind: "add",
        status: "pending",
        batch: "v0.2",
        note: "§8 #2：C++/C# 内部名（`__ctor__Vec`）与源码可读名双字段，保留 func_name 作 display 语义",
    },
];

/// 行为契约：**不得被性能优化破坏**的可观测行为（`CSharp前端引入计划.md` §6-B）。
#[derive(Debug, Clone, serde::Serialize)]
pub struct BehaviorContract {
    pub id: &'static str,
    pub statement: &'static str,
    /// `active`（当前即生效并已有防线）/ `reserved`（随批次激活）
    pub status: &'static str,
    pub batch: &'static str,
    /// 防线载体（可执行的判据所在）
    pub enforced_by: &'static str,
}

/// 行为契约清单（与 `结构重构与C23锚定决议` / C# 计划的"行为契约"同源）。
pub const BEHAVIOR_CONTRACTS: &[BehaviorContract] = &[
    BehaviorContract {
        id: "step_granularity",
        statement: "一步 = 一条字节码指令（含透明的 StepEvent 调试指令）；同一步不被合并",
        status: "active",
        batch: "-",
        enforced_by: "step_payload_schema_v0_1_test::test_step_payload_top_level_fields_frozen（step_index 连续递增）",
    },
    BehaviorContract {
        id: "payload_additive_only",
        statement: "StepPayload 字段只增不改语义；新增字段必须可空/有默认值；v0.1 阶段不得携带未发布字段",
        status: "active",
        batch: "-",
        enforced_by: "step_payload_schema_v0_1_test（键集合冻结 + contracts::RESERVED_FIELDS_V0_2 缺省断言）",
    },
    BehaviorContract {
        id: "jit_breakpoint_integrity",
        statement: "含断点的循环排除出 JIT trace —— 断点语义不被 trace 优化吞掉",
        status: "active",
        batch: "-",
        enforced_by: "jit_unit_test / e2e 断点用例",
    },
    BehaviorContract {
        id: "unwinding_step_granularity",
        statement: "UNWINDING 每弹一帧 / 执行一个 finally 块 = 一个 VM step，**不得合并单步**（展开动画的根基）",
        status: "reserved",
        batch: "CS3b",
        enforced_by: "contracts::check_unwinding_granularity（单测合成序列）+ SharpTutor S4 §5 A2/A5 回放",
    },
    BehaviorContract {
        id: "try_excludes_jit",
        statement: "含 TryBegin 的函数排除出 JIT trace 编译（与断点排除同一模式）",
        status: "reserved",
        batch: "CS3a",
        enforced_by: "CS3a 批次 jit 用例（激活时落库）",
    },
];

/// 展开步样本（"UNWINDING 不得合并单步"判据的输入）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UnwindSample {
    pub step_index: i32,
    pub unwinding: bool,
    pub unwind_frames_left: i32,
}

/// 校验一批步数据是否违反 `unwinding_step_granularity` 契约。
///
/// 判据（对**相邻**两步）：
/// 1. `step_index` 严格递增；
/// 2. `unwind_frames_left >= 0`（负数是"展开已结束"的错误编码，空值语义用 `null`/不动字段表达）；
/// 3. 展开区间内，`unwind_frames_left` 的下降幅度 **≤ 1**——一次弹多帧 = 合并单步 = 违反契约。
///    允许为 0：`finally` 块执行步不弹帧（SharpTutor S4 §5 A5：`handler_depth==0` 但
///    `unwinding==true`），允许回增则为**错误**（展开方向不可逆）；
/// 4. 进入展开（`false → true`）时剩余帧数 ≥ 1（S4 A2：从栈深 N 递减至 1）；
/// 5. 离开展开（`true → false`）时剩余帧数必须归零（S4 A6：异常被吞/捕获后 `unwind_frames_left==0`）。
///
/// 返回 `Err(说明)` 即违反契约；空输入与单样本（无相邻对）恒通过。
pub fn check_unwinding_granularity(steps: &[UnwindSample]) -> Result<(), String> {
    let mut prev: Option<UnwindSample> = None;
    for step in steps {
        if step.unwind_frames_left < 0 {
            return Err(format!(
                "step {}：unwind_frames_left={} 为负 —— 展开结束应以 0 + unwinding=false 表达",
                step.step_index, step.unwind_frames_left
            ));
        }
        if let Some(p) = prev {
            if step.step_index <= p.step_index {
                return Err(format!(
                    "step_index 未严格递增：{} → {}",
                    p.step_index, step.step_index
                ));
            }
            if !p.unwinding && step.unwinding && step.unwind_frames_left < 1 {
                return Err(format!(
                    "step {}：进入展开态时 unwind_frames_left={} 应 ≥ 1（栈深 N 起算）",
                    step.step_index, step.unwind_frames_left
                ));
            }
            if p.unwinding && step.unwinding {
                let delta = p.unwind_frames_left - step.unwind_frames_left;
                if delta > 1 {
                    return Err(format!(
                        "step {} → {}：一次展开 {} 帧（{} → {}）—— UNWINDING 不得合并单步，每帧必须独立成步",
                        p.step_index, step.step_index, delta, p.unwind_frames_left, step.unwind_frames_left
                    ));
                }
                if delta < 0 {
                    return Err(format!(
                        "step {} → {}：unwind_frames_left 回增（{} → {}）—— 展开方向不可逆",
                        p.step_index, step.step_index, p.unwind_frames_left, step.unwind_frames_left
                    ));
                }
            }
            if p.unwinding && !step.unwinding && step.unwind_frames_left != 0 {
                return Err(format!(
                    "step {}：离开展开态但 unwind_frames_left={} ≠ 0",
                    step.step_index, step.unwind_frames_left
                ));
            }
        }
        prev = Some(*step);
    }
    Ok(())
}

/// 契约表 JSON（`capabilities.behavior_contracts` / 文档自检共用）。
pub fn contracts_json() -> serde_json::Value {
    serde_json::json!({
        "schema": SCHEMA_VERSION,
        "frozen_at": SCHEMA_V0_1_FROZEN_AT,
        "reserved_fields_v0_2": RESERVED_FIELDS_V0_2,
        "v0_2_activation_checklist": V0_2_ACTIVATION_CHECKLIST,
        "v0_2_field_ledger": V0_2_FIELD_LEDGER,
        "behavior_contracts": BEHAVIOR_CONTRACTS,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(step_index: i32, unwinding: bool, left: i32) -> UnwindSample {
        UnwindSample {
            step_index,
            unwinding,
            unwind_frames_left: left,
        }
    }

    #[test]
    fn unwinding_one_frame_per_step_passes() {
        // 栈深 3：进入展开 → 逐帧 3 → 2 → 1 → catch
        let seq = [s(1, false, 0), s(2, true, 3), s(3, true, 2), s(4, true, 1), s(5, false, 0)];
        assert!(check_unwinding_granularity(&seq).is_ok());
    }

    #[test]
    fn unwinding_with_finally_step_passes() {
        // finally 执行步不弹帧：剩余帧数持平是合法的（S4 A5）
        let seq = [s(1, true, 3), s(2, true, 3), s(3, true, 2), s(4, true, 2), s(5, true, 1)];
        assert!(check_unwinding_granularity(&seq).is_ok());
    }

    #[test]
    fn merged_unwinding_fails() {
        // 一次弹 2 帧 = 合并单步（契约核心违规）
        let seq = [s(1, false, 0), s(2, true, 3), s(3, true, 1)];
        let err = check_unwinding_granularity(&seq).expect_err("合并单步必须被判据拒绝");
        assert!(err.contains("不得合并单步"), "错误信息应指明契约，实际: {}", err);
    }

    #[test]
    fn unwinding_direction_and_bounds_fail() {
        // 回增
        assert!(check_unwinding_granularity(&[s(1, true, 2), s(2, true, 3)]).is_err());
        // 进入展开时剩余 0 帧
        assert!(check_unwinding_granularity(&[s(1, false, 0), s(2, true, 0)]).is_err());
        // 离开展开但未归零
        assert!(check_unwinding_granularity(&[s(1, true, 2), s(2, false, 1)]).is_err());
        // 负值
        assert!(check_unwinding_granularity(&[s(1, true, -1)]).is_err());
        // 步号不递增
        assert!(check_unwinding_granularity(&[s(2, true, 2), s(2, true, 1)]).is_err());
    }

    #[test]
    fn empty_and_single_samples_pass() {
        assert!(check_unwinding_granularity(&[]).is_ok());
        assert!(check_unwinding_granularity(&[s(7, true, 1)]).is_ok());
    }
}
