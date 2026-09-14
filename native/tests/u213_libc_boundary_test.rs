#![allow(clippy::unwrap_used, clippy::expect_used)]

//! U2#13（2026-09-14）：libc/边界修复批红→绿锚。
//!
//! 六点：fseek 负偏移 clamp / bsearch key 边界 / host_strerror region 登记 /
//! freed_logs 部分重叠精确裁剪 / register_function resize 上限 /
//! call_user_function assert! 改教学诊断（vm/runtime 审查 P1-2/3/4/5/13/8）。

use vitro_native::engine::compile_pipeline::{run_compile_pipeline, setup_vm};
use vitro_native::session::Session;
use vitro_native::vm::core::{StepResult, VitroVM};
use vitro_runtime::InputMode;

fn make_session(source: &str) -> Session {
    let mut session = Session::default();
    session.compile.compile_units.push(vitro_native::session::CompileUnit {
        filename: "main.c".to_string(),
        source: source.to_string(),
    });
    let mut full_source = source.to_string();
    if !full_source.ends_with('\n') {
        full_source.push('\n');
    }
    run_compile_pipeline(&mut session, &full_source).expect("compile failed");
    session
}

/// 跑完程序，返回 (trapped, error, stdout)。
fn run_to_end(session: &mut Session) -> (bool, String, String) {
    session.runtime.input_mode = InputMode::Batch;
    let mut vm = VitroVM::new();
    setup_vm(&mut vm, session);
    let mut trapped = false;
    let mut err = String::new();
    loop {
        match vm.step(&mut session.as_vm_context()) {
            StepResult::Finished => break,
            StepResult::Trap => {
                trapped = true;
                err = vm.get_error().to_string();
                break;
            }
            _ => {}
        }
    }
    (trapped, err, session.runtime.stdout())
}

/// ① fseek 负偏移：负 SEEK_SET / 越过文件头的 SEEK_CUR 必须**返回 -1 且不移动
/// 游标**（glibc/EINVAL 语义）。修复前负数 `as usize` 绕回为天文数字游标
///（后续读写即越界）。本锚修复前红（返回 0 / ftell 错乱）。
#[test]
fn test_u213_fseek_negative_offset_rejected() {
    let source = r#"
#include <stdio.h>
int main() {
    FILE *fp = fopen("t.bin", "wb");
    if (!fp) { printf("open fail\n"); return 1; }
    fputs("abcdef", fp);
    int r1 = fseek(fp, -5, SEEK_SET);
    long t1 = ftell(fp);
    int r2 = fseek(fp, -1000, SEEK_CUR);
    long t2 = ftell(fp);
    printf("r1=%d t1=%ld r2=%d t2=%ld\n", r1, t1, r2, t2);
    fclose(fp);
    return 0;
}
"#;
    let mut session = make_session(source);
    let (trapped, err, out) = run_to_end(&mut session);
    assert!(!trapped, "不应 trap：{err}");
    // C 语义：fseek 失败返回 -1 且流位置**保持不变**——写完 6 字节后游标为 6，
    // 两次失败的 seek 后 ftell 仍应是 6（修复前：返回 0 假成功 + 游标绕回天文数字）。
    assert!(
        out.contains("r1=-1") && out.contains("t1=6"),
        "负 SEEK_SET 必须失败且游标留在原位：{out}"
    );
    assert!(
        out.contains("r2=-1") && out.contains("t2=6"),
        "越过文件头的 SEEK_CUR 必须失败且游标留在原位：{out}"
    );
}

/// ② bsearch key 边界：key 指向 [key, key+size) 越界时（学生常见野指针），
/// 修复前 compar==NULL 分支直接切片 panic（进程崩溃）；修复后按"未找到"
/// 返回 NULL。
#[test]
fn test_u213_bsearch_wild_key_no_panic() {
    let source = r#"
#include <stdio.h>
#include <stdlib.h>
int main() {
    int arr[4] = {10, 20, 30, 40};
    int *wild = (int *)0xFFFFF;
    int *r = (int *)bsearch(wild, arr, 4, sizeof(int), NULL);
    printf("r=%d\n", r == NULL);
    return 0;
}
"#;
    let mut session = make_session(source);
    let (trapped, err, out) = run_to_end(&mut session);
    assert!(!trapped, "越界 key 应按未找到处理而非崩溃：{err}");
    assert!(out.contains("r=1"), "返回值应为 NULL：{out}");
}

/// ③ host_strerror region 登记：strerror 的返回缓冲由 allocate_raw 分配但
/// 此前不进 regions——泄漏报告漏计该块、学生 free 它又被误报 E3027。
/// 修复后：不 free → 泄漏报告计入 1 处（红锚：修复前 0 处）。
#[test]
fn test_u213_strerror_region_leak_counted() {
    let source = r#"
#include <stdio.h>
#include <string.h>
int main() {
    char *msg = strerror(1);
    printf("%s\n", msg);
    return 0;
}
"#;
    let mut session = make_session(source);
    let (trapped, err, out) = run_to_end(&mut session);
    assert!(!trapped, "不应 trap：{err}");
    assert!(out.contains("Invalid argument"), "strerror 文本应正确：{out}");
    // 泄漏报告由出口层在运行结束时追加（直跑 VM 不会生成）——补调
    vitro_native::engine::session_ops::append_leak_report(&mut session);
    let notes = session.runtime.notes();
    assert!(
        notes.contains("1 处"),
        "未释放的 strerror 缓冲必须计入泄漏报告（修复前 allocate_raw 不登记 region 而漏计）：{notes}"
    );
}

/// ③-b strerror 缓冲 free 不误报：修复前 free 该地址因无 region 记录而
/// 误报 E3027（invalid free）。
#[test]
fn test_u213_strerror_free_no_false_alarm() {
    let source = r#"
#include <stdio.h>
#include <string.h>
int main() {
    char *msg = strerror(1);
    printf("%s\n", msg);
    free(msg);
    return 0;
}
"#;
    let mut session = make_session(source);
    let (trapped, err, _) = run_to_end(&mut session);
    assert!(
        !trapped,
        "free strerror 缓冲不应误报（修复前 E3027 invalid free）：{err}"
    );
    vitro_native::engine::session_ops::append_leak_report(&mut session);
    assert!(!session.runtime.notes().contains("处"), "已释放则泄漏报告应为 0 处：{}", session.runtime.notes());
}

/// ④ freed_logs 部分重叠精确裁剪：free X(size=100) 后 malloc Y(size=60)
/// 复用 X 的前 60 字节——旧记录 [x, x+100) 与新块 [x, x+60) **部分重叠**。
/// 修复前整条删除旧记录 → X 尾部 [x+60, x+100) 的 UAF 检测窗口丢失
///（假阴性）；修复后裁剪保留尾部区间，访问 X 尾部必须触发 E3060。
#[test]
fn test_u213_freed_logs_partial_overlap_trimmed() {
    use vitro_native::vm::host_funcs::{host_free, host_malloc};
    use vitro_vm::context::VmContext;

    let source = "int main() { return 0; }";
    let mut session = make_session(source);
    session.runtime.input_mode = InputMode::Batch;
    session.memory.quarantine_budget = 0; // free 立即进 free_list 可复用
    let mut vm = VitroVM::new();
    setup_vm(&mut vm, &session);

    let malloc = |vm: &mut VitroVM, ctx: &mut VmContext<'_>, n: u64| {
        vm.push(n);
        host_malloc(vm, ctx);
        vm.pop() as u32
    };

    let x = malloc(&mut vm, &mut session.as_vm_context(), 100);
    // 产生 freed log [x, x+100)
    vm.push(x as u64);
    host_free(&mut vm, &mut session.as_vm_context());
    // 部分重叠复用：取前 60 字节（first-fit 拆分）
    let y = malloc(&mut vm, &mut session.as_vm_context(), 60);
    assert_eq!(y, x, "free_list first-fit 应复用 x 的起点");

    // X 尾部 [x+60, x+100) 仍在 UAF 检测窗口——修复前整条删除导致假阴性
    let logs = vm.get_freed_logs();
    let has_tail = logs.values().any(|l| {
        let l_end = l.addr.saturating_add(l.size);
        l.addr < x + 100 && l_end > x + 60
    });
    assert!(
        has_tail,
        "部分重叠后旧记录的未复用尾部 [{:+}, {:+}) 必须保留（修复前整条删除 = UAF 假阴性），logs: {:?}",
        x + 60,
        x + 100,
        logs.values().collect::<Vec<_>>()
    );
    // 复用段 [x, x+60) 不应再命中（新分配合法覆盖）
    let has_head = logs.values().any(|l| l.addr == x && l.size == 100);
    assert!(!has_head, "原整条记录不应原样保留（复用段已合法）");
}

/// ⑤ register_function resize 上限：恶意/损坏的函数索引曾直接
/// `resize(idx+1)`（idx=u32::MAX 即 OOM abort）。上限 65536，超限拒绝。
/// 红锚用 idx=上限+1（修复前 resize 成功使 len=65537 → 断言红；
/// u32::MAX 极端形态由同一上限拦截，留痕注明无法安全运行）。
#[test]
fn test_u213_register_function_bounded() {
    let mut vm = VitroVM::new();
    let limit = vitro_native::vm::core::MAX_FUNCTIONS;
    let meta = vitro_native::vm::core::FuncMeta::default();
    // 预算内正常注册
    assert!(
        vm.register_function(limit as u32 - 1, meta.clone()),
        "上限内的索引应注册成功"
    );
    assert_eq!(vm.func_table_len(), limit);
    // 超限拒绝
    assert!(
        !vm.register_function(limit as u32, meta.clone()),
        "超上限索引必须拒绝（修复前 resize(65537) 直接扩表）"
    );
    assert_eq!(vm.func_table_len(), limit, "表长不得越上限增长");
    // 极端值同样拒绝（修复前 u32::MAX → resize 4G 项 OOM abort，无法安全留痕）
    assert!(!vm.register_function(u32::MAX, meta), "u32::MAX 必须拒绝");
    assert_eq!(vm.func_table_len(), limit);
}

/// ⑥ call_user_function assert! 改教学诊断：非 4 字节参数的比较器回调
/// （如 `int cmp(double, double)` 传给 qsort）修复前 assert! panic（进程
/// 崩溃）；修复后 trap 教学诊断。
#[test]
fn test_u213_call_user_function_bad_params_diagnosed() {
    let source = r#"
#include <stdio.h>
#include <stdlib.h>
int cmp(double a, double b) { return (int)(a - b); }
int main() {
    int arr[3] = {3, 1, 2};
    qsort(arr, 3, sizeof(int), cmp);
    printf("%d %d %d\n", arr[0], arr[1], arr[2]);
    return 0;
}
"#;
    let mut session = make_session(source);
    let (trapped, err, _) = run_to_end(&mut session);
    assert!(
        trapped,
        "非 4 字节参数的比较器必须产生诊断（修复前 assert! panic 崩溃进程）"
    );
    assert!(
        err.contains("比较函数") || err.contains("参数"),
        "诊断应说明比较函数参数形状问题：{err}"
    );
}

/// P1-a（审阅发现，2026-09-14）：bsearch 越界 key 早退不得泄漏 qsort_depth——
/// 该计数器与 qsort 共用（门限 8），泄漏 8 次后 bsearch 永久返回 NULL、
/// qsort 静默变 no-op（排序结果无声错误）。修复前本锚红。
#[test]
fn test_u213_bsearch_wild_key_leaks_no_depth() {
    use vitro_native::vm::host_funcs::{host_bsearch, host_free, host_malloc};
    use vitro_vm::context::VmContext;

    let source = r#"
#include <stdlib.h>
int cmp(const void *a, const void *b) { return *(const int *)a - *(const int *)b; }
int main() { return 0; }
"#;
    let mut session = make_session(source);
    session.runtime.input_mode = InputMode::Batch;
    let mut vm = VitroVM::new();
    setup_vm(&mut vm, &session);

    // host_bsearch 按调用约定依次 pop：compar, size, nmemb, base, key
    //（参数逆序压栈）
    let bsearch = |vm: &mut VitroVM, ctx: &mut VmContext<'_>, key: u64, base: u64, n: u64, sz: u64| {
        vm.push(0); // compar = NULL（字节比较）
        vm.push(sz);
        vm.push(n);
        vm.push(base);
        vm.push(key);
        host_bsearch(vm, ctx);
    };

    // 8 次越界 key（超过门限）
    for _ in 0..8 {
        bsearch(&mut vm, &mut session.as_vm_context(), 0xFFFFF, 0x1000, 4, 4);
        let _ = vm.pop();
    }
    assert_eq!(
        vm.qsort_depth(),
        0,
        "越界 key 早退不得泄漏深度计数（修复前恒 8 → bsearch/qsort 永久失效）"
    );

    // 合法 bsearch（NULL 比较器）仍可用：depth 已归零，不被门限拦截
    bsearch(&mut vm, &mut session.as_vm_context(), 0x1000, 0x1000, 4, 4);
    let r = vm.pop();
    assert_ne!(r, 0xFFFF_FFFF, "合法调用不应产出深度门限 note 路径的返回值");
    assert!(
        !session.runtime.notes().contains("嵌套深度超过限制"),
        "合法调用不得命中深度门限：{}",
        session.runtime.notes()
    );
    // malloc/free 完整性（qsort 同计数器不受牵连）
    vm.push(16);
    host_malloc(&mut vm, &mut session.as_vm_context());
    let p = vm.pop() as u32;
    vm.push(p as u64);
    host_free(&mut vm, &mut session.as_vm_context());
}

/// P1-b（审阅发现）：strerror 的 region 登记必须与 strdup 同型三步——
/// 复用路径（free_list 取回已用地址）下无条件 push_region 会造成同址双条目与
/// 索引失配；stale freed_logs 未清会让 write_memory 被 UAF 检查拦截、
/// 返回全零缓冲。修复前本锚红。
#[test]
fn test_u213_strerror_reuse_no_duplicate_regions() {
    use vitro_native::vm::host_funcs::{host_free, host_malloc, host_strerror};

    let source = "int main() { return 0; }";
    let mut session = make_session(source);
    session.runtime.input_mode = InputMode::Batch;
    session.memory.quarantine_budget = 0; // free 立即归还 free_list 可复用
    let mut vm = VitroVM::new();
    setup_vm(&mut vm, &session);

    // malloc + free 一个与 strerror 缓冲同量级的块（对齐后同 slot 尺寸）
    vm.push(16);
    host_malloc(&mut vm, &mut session.as_vm_context());
    let x = vm.pop() as u32;
    vm.push(x as u64);
    host_free(&mut vm, &mut session.as_vm_context());

    // strerror 复用该地址（先压 errnum=1 → "Invalid argument"）
    vm.push(1);
    host_strerror(&mut vm, &mut session.as_vm_context());
    let p = vm.pop() as u32;
    assert_ne!(p, 0, "strerror 应成功分配");

    // ① 无同址双条目（索引一致性）
    assert!(
        session.memory.verify_region_index().is_ok(),
        "同址双条目会令 region 索引失配（修复前无条件 push_region）"
    );
    let dup = session
        .memory
        .regions
        .iter()
        .filter(|r| r.addr == p)
        .count();
    assert_eq!(dup, 1, "复用地址必须恰好一条 region（修复前 push 出第二条）");

    // ② 缓冲内容正确（stale freed_logs 拦截 write_memory 时为全零）
    let bytes = vm.memory_ref();
    let s = String::from_utf8_lossy(&bytes[p as usize..p as usize + 24]).to_string();
    let nul = s.find('\0').unwrap_or(s.len());
    assert_eq!(
        &s[..nul],
        "Invalid argument",
        "复用路径的缓冲内容必须正确（修复前 stale freed_logs 拦截写入 → 全零）"
    );
}
