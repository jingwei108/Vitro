use crate::session::Session;
use crate::unified::types::{
    AccessedVar, ApiFrameInfo, ApiVariableSnapshot, PointerSnapshot, PointerStatus, StepPayload,
};
use crate::vm::core::CideVM;
use cide_algorithm_steps::infer_algorithm_step;

/// 每步数据收集器：从 VM 和 Session 中提取轻量 `StepPayload`。
pub struct StepCollector;

impl StepCollector {
    pub fn collect(vm: &mut CideVM, session: &mut Session, step_index: i32) -> StepPayload {
        let code_line = vm.get_current_line();
        let func_name = vm.get_call_stack().last().map(|f| f.func_name.clone()).unwrap_or_default();

        // 数组元素快照（可视化条形图 + 下面 local_vars 的元素摘要共用一次采集）
        let array_snapshots: Vec<crate::unified::types::ArraySnapshot> =
            vm.get_array_snapshots().into_iter().map(Into::into).collect();
        // 数组的"值"用元素摘要表示：此前 local_vars 里的数组条目显示的是首元素（如 `0`），
        // 与 array_snapshots 重复，且会被消费方误读成"数组的值"（P2-7b）。
        let array_summaries: std::collections::HashMap<&str, String> = array_snapshots
            .iter()
            .map(|a| {
                const MAX_SHOWN: usize = 16;
                let shown: Vec<String> = a.elements.iter().take(MAX_SHOWN).cloned().collect();
                let mut s = format!("{{{}}}", shown.join(", "));
                if a.elements.len() > shown.len() {
                    s.push_str(", …");
                }
                (a.name.as_str(), s)
            })
            .collect();

        let local_vars: Vec<ApiVariableSnapshot> = vm
            .get_variable_snapshot()
            .into_iter()
            .map(|v| {
                let is_array = matches!(v.ty.kind(), crate::compiler::ast::TypeKind::Array);
                let value_str = if is_array {
                    array_summaries
                        .get(v.name.as_str())
                        .cloned()
                        .unwrap_or_else(|| format_value(&v))
                } else {
                    format_value(&v)
                };
                ApiVariableSnapshot {
                    name: v.name,
                    addr: v.addr,
                    is_local: v.is_local,
                    // P2-7c：C 风格可读类型名（此前是 `format!("{:?}", ty)` 的内部枚举结构）
                    ty_name: cide_runtime::type_display_name(&v.ty),
                    value: value_str,
                }
            })
            .collect();

        let call_stack = vm
            .get_call_stack()
            .iter()
            .map(|f| ApiFrameInfo {
                func_name: f.func_name.clone(),
                return_line: 0, // MVP 阶段简化
            })
            .collect();

        // 取出该步产生的可视化事件
        let vis_events: Vec<crate::session::VisEvent> = vm.take_vis_events().into_iter().map(Into::into).collect();

        let heatmap_line = code_line;
        let heatmap_count = session.runtime.heatmap.line_counts.get(&code_line).copied().unwrap_or(0);

        let at_callee_entry = vm
            .get_call_stack()
            .last()
            .map(|f| f.caller_line == code_line)
            .unwrap_or(false);
        let semantic_label =
            infer_semantic_label(code_line, Some(&local_vars), &func_name, session, at_callee_entry);
        // U1#1 管道批（P0-4/P1-3/P1-4/P1-1 边界）：行入口变量快照维护——
        // 行变化时把上一帧行（上一行末帧）的变量固化为 row_entry_vars，
        // 供"展示运算过程"的 phase 取语句执行前操作数。
        if code_line != session.unified_last_line {
            session.unified_row_entry_vars = session.unified_last_frame_vars.clone();
            session.unified_last_line = code_line;
        }
        let algorithm_step = {
            let ctx: &dyn cide_algorithm_steps::AlgorithmContext = session;
            let algo_vars: Vec<cide_algorithm_steps::VariableSnapshot> = local_vars
                .iter()
                .map(|v| cide_algorithm_steps::VariableSnapshot {
                    name: v.name.clone(),
                    value: v.value.clone(),
                })
                .collect();
            // env：caller 栈层的函数名（callee entry 且 caller 是 main =
            // 顶层启动调用）+ 下一行源码（多行 for 的体）。
            let stack = vm.get_call_stack();
            let caller_is_main =
                stack.len() >= 2 && stack[stack.len() - 2].func_name.eq_ignore_ascii_case("main");
            // lookahead：下 3 行拼接（嵌套 for 头穿透——dpLCS 的
            // L10 for i / L11 for j / L12 体，+1 行不够）。
            let mut lookahead = String::new();
            for dl in 1..=3 {
                if let Some(l) = ctx.source_line(code_line + dl) {
                    lookahead.push_str(&l);
                    lookahead.push(0x0A as char);
                }
            }
            let env = cide_algorithm_steps::InferEnv {
                prev_vars: &session.unified_row_entry_vars,
                at_callee_entry,
                caller_is_main,
                lookahead: &lookahead,
            };
            let step = infer_algorithm_step(code_line, &algo_vars, &func_name, ctx, &env);
            session.unified_last_frame_vars = algo_vars;
            step.map(|s| {
                crate::unified::types::AlgorithmStepSnapshot {
                    algorithm_name: s.algorithm_name,
                    display_name: s.display_name,
                    phase: s.phase,
                    description: s.description,
                }
            })
        };

        let accessed_vars = vm
            .get_last_accessed_vars()
            .iter()
            .map(|a| AccessedVar {
                name: a.name.clone(),
                access_type: match a.access_type {
                    crate::vm::core::AccessType::Read => "Read".to_string(),
                    crate::vm::core::AccessType::Write => "Write".to_string(),
                },
            })
            .collect();

        let pointer_snapshots = collect_pointer_snapshots(vm, session, &local_vars);

        StepPayload {
            step_index,
            code_line,
            func_name,
            semantic_label,
            local_vars,
            call_stack,
            vis_events,
            heatmap_line,
            heatmap_count,
            accessed_vars,
            array_snapshots,
            pointer_snapshots,
            algorithm_step,
            root_cause_hint: None,
        }
    }
}

/// 从变量快照中提取指针变量，并判断其状态。
///
/// `target_name`（D3）：先在**当前帧快照**里找，未命中再向 VM 请求**跨帧解析**
/// （全局 + 全部活跃帧）——`swap(&x, &y)` 体内 `a`/`b` 指向调用者的 `x`/`y`，
/// 当前帧快照里没有这两个符号，只看当前帧就永远是空串。
/// 两级顺序保留既有"当前帧优先"的可读性（同名变量时内层更贴近学生视角）。
fn collect_pointer_snapshots(
    vm: &CideVM,
    session: &Session,
    local_vars: &[ApiVariableSnapshot],
) -> Vec<PointerSnapshot> {
    let mut result = Vec::new();
    for v in local_vars {
        if !is_pointer_type(&v.ty_name) {
            continue;
        }
        let target_addr = match parse_addr(&v.value) {
            Some(a) => a,
            None => continue,
        };

        let status = if target_addr == 0 {
            PointerStatus::Null
        } else if !(cide_runtime::NULL_TRAP_SIZE..cide_runtime::MEM_SIZE).contains(&target_addr) {
            PointerStatus::Dangling
        } else if is_freed_heap(&session.memory.regions, target_addr) {
            PointerStatus::Freed
        } else {
            PointerStatus::Valid
        };

        let target_name = {
            let same_frame = find_var_name_at_addr(local_vars, target_addr);
            if same_frame.is_empty() {
                vm.find_variable_name_at_addr(target_addr).unwrap_or_default()
            } else {
                same_frame
            }
        };

        result.push(PointerSnapshot {
            name: v.name.clone(),
            addr: v.addr,
            ty_name: v.ty_name.clone(),
            target_addr,
            target_name,
            status,
        });
    }
    result
}

/// 判断某行是否是函数**定义**行（形如 `int helper(int x) {`）。
///
/// 用于把"函数定义行"从递归调用判定里排除。仅识别单行形态（`{` 与签名同行）；
/// 左花括号写在下一行时仍会误判 —— 已知限制，记录于 `ARCHIVE_代码审查复核20260911.md`。
fn is_function_definition_line(line: &str, func_name: &str) -> bool {
    line.ends_with('{') && line.contains(&format!("{}(", func_name))
}

/// 从源码行里提取数组下标的**标识符**（`temp = arr[j];` → `j`）。
///
/// 只认形如 `name[ident]` 的简单下标；表达式下标（`arr[i + 1]`）返回 `None`。
fn array_index_var(line: &str) -> Option<String> {
    let mut search_from = 0usize;
    while let Some(rel) = line[search_from..].find('[') {
        let open = search_from + rel;
        let name_start = line[..open]
            .rfind(|c: char| !(c.is_alphanumeric() || c == '_'))
            .map(|p| p + 1)
            .unwrap_or(0);
        if name_start < open {
            if let Some(close_rel) = line[open..].find(']') {
                let inner = line[open + 1..open + close_rel].trim();
                if !inner.is_empty() && inner.chars().all(|c| c.is_alphanumeric() || c == '_') {
                    return Some(inner.to_string());
                }
            }
        }
        search_from = open + 1;
        if search_from >= line.len() {
            break;
        }
    }
    None
}

/// 取"当前最内层迭代变量"的值（P0-3）。
///
/// 优先用源码行里的数组下标标识符；取不到时按内层循环的常见命名回退（`j` → `i` → `k` …），
/// 最后才退回候选列表的末位。**不使用 `first()`** —— 白名单首位可能是 `n`/`len` 这类
/// 规模量，与"当前迭代到哪"无关，曾导致 `交换 arr[5]↔arr[6]` 这类与执行事实相反的描述。
fn pick_inner_index(loop_vars: &[(String, i32)], source_line: &str) -> i32 {
    if let Some(name) = array_index_var(source_line) {
        if let Some((_, v)) = loop_vars.iter().find(|(n, _)| *n == name) {
            return *v;
        }
    }
    for cand in ["j", "i", "k", "idx", "index"] {
        if let Some((_, v)) = loop_vars.iter().find(|(n, _)| n == cand) {
            return *v;
        }
    }
    loop_vars.last().map(|(_, v)| *v).unwrap_or(0)
}

fn is_pointer_type(ty_name: &str) -> bool {
    ty_name.contains('*') || ty_name.contains("Pointer")
}

fn parse_addr(value: &str) -> Option<u32> {
    // 支持 "0x1234" 和十进制 "4660"
    if value.starts_with("0x") || value.starts_with("0X") {
        u32::from_str_radix(&value[2..], 16).ok()
    } else {
        value
            .parse::<i64>()
            .ok()
            .and_then(|n| if n >= 0 { Some(n as u32) } else { None })
    }
}

fn is_freed_heap(regions: &[cide_runtime::MemoryRegionData], addr: u32) -> bool {
    regions.iter().any(|r| r.is_heap && r.addr == addr && r.is_freed)
}

fn find_var_name_at_addr(local_vars: &[ApiVariableSnapshot], addr: u32) -> String {
    for v in local_vars {
        if v.addr == addr {
            return v.name.clone();
        }
    }
    String::new()
}

fn format_value(v: &cide_runtime::VariableSnapshotData) -> String {
    use crate::compiler::ast::TypeKind;
    match v.ty.kind() {
        TypeKind::Double => {
            let bits = v.value as u64;
            let f = f64::from_bits(bits);
            format!("{:.15}", f).trim_end_matches('0').trim_end_matches('.').to_string()
        }
        TypeKind::Float => {
            let bits = v.value as u32;
            let f = f32::from_bits(bits);
            format!("{:.7}", f).trim_end_matches('0').trim_end_matches('.').to_string()
        }
        _ => v.value.to_string(),
    }
}

/// 推断语义标签——**全库唯一分类器**（R3 教学标注单源化）。
///
/// `local_vars`：
/// - `Some(vars)`：全量形态（交换下标、循环变量取值等值敏感分支可用），
///   供 StepPayload 语义标注使用；
/// - `None`：降级形态（局部变量不可用时，如检查点保存路径），**同一词汇表、
///   同一判定顺序**，仅值敏感分支退化为行首形态判断。
///
/// 此前 `engine.rs::quick_semantic_label` 是第二套简化启发（"循环边界" vs
/// 本函数的"循环"、无交换下标），同一行源码两处标注词汇不一致——R3 收口后
/// 检查点判定与 StepPayload 标注出自同一函数。
pub(crate) fn infer_semantic_label(
    code_line: i32,
    local_vars: Option<&[ApiVariableSnapshot]>,
    func_name: &str,
    session: &Session,
    at_callee_entry: bool,
) -> String {
    if code_line <= 0 {
        return String::new();
    }

    // 获取当前源码行（P0-4：按全局行号 → 文件映射定位，多文件会话下不再固定查第一个单元）
    let source_line = session
        .source_line_at(code_line)
        .map(|s| s.trim().to_string())
        .unwrap_or_default();

    // 提取循环变量（i, j, k, idx, index, m, n, left, right, mid, low, high, pivot）
    let loop_vars: Vec<(String, i32)> = local_vars
        .unwrap_or(&[])
        .iter()
        .filter_map(|v| {
            if matches!(
                v.name.as_str(),
                "i" | "j"
                    | "k"
                    | "idx"
                    | "index"
                    | "m"
                    | "n"
                    | "left"
                    | "right"
                    | "mid"
                    | "low"
                    | "high"
                    | "pivot"
                    | "gap"
            ) {
                v.value.parse::<i32>().ok().map(|val| (v.name.clone(), val))
            } else {
                None
            }
        })
        .collect();

    let loop_depth = loop_vars.len() as i32;

    // 降级形态（无局部变量值）：循环行首形态直接判"循环"（同一词汇表）
    let is_loop_headline = local_vars.is_none()
        && (source_line.starts_with("for ") || source_line.starts_with("while "));

    // 检测交换模式：包含 temp + arr[ / a[ + 赋值
    let is_swap = source_line.contains("temp")
        && (source_line.contains("arr[") || source_line.contains("a["))
        && source_line.contains("=");

    // S3 观测 #3（schema §8 #10）修复：进入被调函数的第一步，code_line 归因于
    // 调用点行（`swap(&x, &y);`）——func_name 已是 callee、行内含 `swap(`，
    // 旧逻辑误判"递归调用 swap"。`at_callee_entry` 由调用方经
    // `caller_line == code_line` 判定；调用点行只可能是普通调用（真递归的
    // 调用行在函数体内部，彼时 caller_line != code_line）。
    // 检测递归调用（排除函数定义行 —— `int helper(int x) {` 含 `helper(` 但不是递归调用，
    // 此前会把定义行标成"递归调用 helper"，是直接呈现给学生的错误描述）
    let is_recursive = !at_callee_entry
        && !func_name.is_empty()
        && func_name != "main"
        && source_line.contains(&format!("{}(", func_name))
        && !is_function_definition_line(&source_line, func_name);

    // 检测普通函数调用（排除控制流关键字）
    let is_func_call = source_line.contains('(')
        && !source_line.starts_with("if ")
        && !source_line.starts_with("while ")
        && !source_line.starts_with("for ")
        && !source_line.starts_with("switch ")
        && !source_line.starts_with("return ")
        && !source_line.starts_with("//")
        && !source_line.starts_with("/*");

    // 生成语义标签
    //
    // 判定顺序（2026-09-12 修订，B2-3 词汇闭合防线首日抓到的缺陷）：
    // **具体语句模式优先于"循环上下文"**。旧顺序把 `loop_depth >= 1` 放在
    // 递归/printf/return/内存分配 之前，而 `loop_depth` 只要求"有循环变量在白名单里
    // 且已进入作用域"——循环变量在循环结束后仍在作用域内，于是 `printf(...)`、
    // `free(p);`、`return 0;` 这些**位于循环之后**的行全部被标成 `循环 i=3`
    // （实测：`释放内存` 与 `调用 printf` 在真实程序里几乎不可达，词汇表条目形同虚设）。
    // 修订后：具体模式先判；都判不出来时**才**回落到循环上下文（保留"循环体内
    // 普通语句显示当前迭代变量"这一教学价值最高的用法），最后才是行号兜底。
    if is_loop_headline {
        return "循环".to_string();
    }
    if is_swap && loop_depth >= 1 {
        // P0-3：交换语句形如 `temp = arr[j];` —— 下标变量从**源码行**里取。
        // 此前用 `loop_vars.first()`，而白名单里含 `n`（规模量），取到的常是 n 而非 j，
        // 于是同一个 payload 里 semantic_label 说 `交换 arr[5]↔arr[6]`、
        // algorithm_step 说 `交换 arr[0]↔arr[1]`，消费方同时收到一对一错的描述。
        let idx_val = pick_inner_index(&loop_vars, &source_line);
        format!("交换 arr[{}]↔arr[{}]", idx_val, idx_val + 1)
    } else if is_recursive {
        format!("递归调用 {}", func_name)
    } else if source_line.starts_with("printf") || source_line.starts_with("scanf") {
        let func = if source_line.starts_with("printf") {
            "printf"
        } else {
            "scanf"
        };
        format!("调用 {}", func)
    } else if source_line.starts_with("return") {
        "返回".to_string()
    } else if source_line.contains("malloc") || source_line.contains("calloc") {
        "内存分配".to_string()
    } else if source_line.contains("free(") {
        "释放内存".to_string()
    } else if source_line.contains("getchar") || source_line.contains("putchar") {
        "IO 操作".to_string()
    } else if source_line.contains("qsort(") {
        "调用 qsort".to_string()
    } else if is_func_call {
        // 尝试提取函数名
        if let Some(func) = extract_called_func(&source_line) {
            format!("调用 {}", func)
        } else {
            "函数调用".to_string()
        }
    } else if loop_depth >= 1 {
        let iter_str = loop_vars
            .iter()
            .map(|(name, val)| format!("{}={}", name, val))
            .collect::<Vec<_>>()
            .join(", ");
        format!("循环 {}", iter_str)
    } else {
        format!("第 {} 行", code_line)
    }
}

/// 从函数调用语句中提取函数名。
fn extract_called_func(line: &str) -> Option<String> {
    let trimmed = line.trim();
    // 跳过赋值部分："x = foo(...)" → "foo(...)"
    let after_assign = if let Some(pos) = trimmed.find("=") {
        trimmed[pos + 1..].trim()
    } else {
        trimmed
    };
    // 提取函数名："foo(bar, baz)" → "foo"
    if let Some(paren_pos) = after_assign.find('(') {
        let name = after_assign[..paren_pos].trim();
        if !name.is_empty() && name.chars().all(|c| c.is_alphanumeric() || c == '_') {
            return Some(name.to_string());
        }
    }
    None
}
