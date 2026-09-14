//! Session 操作公共函数
//!
//! 为 `flutter_bridge.rs` 和 `capi/mod.rs` 提供统一的运行/单步核心逻辑，
//! 消除两端的重复代码。

use crate::engine::compile_pipeline::setup_vm;
use crate::session::Session;
use crate::vm::core::VitroVM;

/// 生成内存泄漏报告并追加到输出。
///
/// 遍历所有未释放的堆区域，按分配行号排序，
/// 输出形如 "第 12 行的 malloc 未被 free" 的提示。
pub fn append_leak_report(session: &mut Session) {
    let leaks: Vec<_> = session
        .memory
        .regions
        .iter()
        .filter(|r| r.is_heap && !r.is_freed && r.alloc_by != "vfs")
        .collect();

    if leaks.is_empty() {
        return;
    }

    let mut lines: Vec<String> = Vec::new();
    // 首行前导 `\n` 用于与最后一段程序输出分隔；每段的尾随换行由 `push_note` 统一补齐
    // （否则 display 视图零分隔拼接会把报告压成一行：`===== 报告 =====发现 1 处…`）。
    lines.push("\n===== 内存泄漏检测报告 =====".to_string());

    let mut sorted = leaks.clone();
    sorted.sort_by_key(|r| r.alloc_line);

    let total_leaked: i32 = sorted.iter().map(|r| r.size).sum();
    lines.push(format!("发现 {} 处未释放的堆内存，共 {} 字节：", sorted.len(), total_leaked));

    for r in &sorted {
        let by = if r.alloc_by.is_empty() { "malloc" } else { &r.alloc_by };
        if r.alloc_line > 0 {
            lines.push(format!(
                "  • 第 {} 行的 {} 分配了 {} 字节 (addr=0x{:04X})，未被 free",
                r.alloc_line, by, r.size, r.addr
            ));
        } else {
            lines.push(format!("  • {} 分配了 {} 字节 (addr=0x{:04X})，未被 free", by, r.size, r.addr));
        }
    }

    lines.push("💡 提示：在 C 语言中，malloc 分配的内存需要对应 free 释放，否则会造成内存泄漏。".to_string());
    lines.push("==============================".to_string());

    // E-P1-5：泄漏报告是引擎附注，走 note 通道——绝不混入程序 stdout。
    for line in lines {
        session.runtime.push_note(line);
    }
}

/// 初始化程序运行环境（非 resume 场景）
pub fn reset_runtime(session: &mut Session) {
    session.runtime.clear_output();
    session.runtime.error.clear();
    session.runtime.trace.clear();
    session.memory.regions.clear();
    // U2#2：索引随 regions 一起清空（不变量：键集与 addr 集一致）
    session.memory.region_index.clear();
    session.memory.free_list.clear();
    // 隔离区随运行一起清空（quarantine_budget 是会话级配置，保留）
    session.memory.quarantine.clear();
    session.memory.quarantine_bytes = 0;
    // R1 ①：动态堆起点——越过全局数据末端（含 argv 顶界）。此前写死 HEAP_START，
    // "全局数据 > 20KB 且使用 malloc" 会静默压坏堆数据（AGENTS.md 已知限制销项）。
    let heap_base = vitro_runtime::compute_heap_base(
        session.compile.global_data_end,
        session.runtime.argc,
        &session.runtime.argv,
    );
    session.memory.set_heap_base(heap_base);
    session.memory.alloc_counter = 0;
    session.vfs = crate::vm::vfs::VirtualFileSystem::new();
    session.runtime.running = true;
}

/// 初始化单步执行环境
pub fn reset_runtime_for_step(session: &mut Session) {
    reset_runtime(session);
    session.runtime.step_count = 0;
    session.runtime.step_mode = true;
}

/// 为 VM 注入预设测试文件
pub fn inject_preset_files(vm: &mut VitroVM, session: &mut Session) {
    let mut vfs = std::mem::take(&mut session.vfs);
    vfs.inject_preset_file("test.txt", b"hello\nworld\n", vm, &mut session.memory);
    vfs.inject_preset_file("numbers.txt", b"1 2 3 4 5\n", vm, &mut session.memory);
    session.vfs = vfs;
}

/// 执行全速运行。
///
/// 调用前需确保 session 已编译且 VM 可用。
/// 返回 `(运行返回值, 是否等待输入)`；若运行出错返回 `Err`。
pub fn execute_run(session: &mut Session) -> Result<(i32, bool), String> {
    let is_resume = session.runtime.waiting_input;

    if !is_resume {
        reset_runtime(session);
    }
    session.runtime.step_mode = false;
    session.runtime.waiting_input = false;

    // B47: 用 catch_unwind 保护 take 后的 VM，避免 setup_vm / inject_preset_files / vm.run
    // 中 panic 导致 VM 永久丢失。panic 后把 VM 还回 session 并返回错误。
    // wasm32-unknown-unknown 不支持 catch_unwind，因此 Web 平台直接执行。
    let mut vm_slot = Some(session.vm.take().unwrap_or_default());
    // wasm32 分支需要可变闭包以调用；desktop 分支通过 catch_unwind 按值消费，
    // `mut` 在 desktop 下会触发 unused_mut 警告，因此统一允许。
    #[allow(unused_mut)]
    let mut run_vm = || {
        let vm = match vm_slot.as_mut() {
            Some(vm) => vm,
            None => return -1,
        };
        if !is_resume {
            setup_vm(vm, session);
            inject_preset_files(vm, session);
        } else {
            vm.resume();
        }
        vm.run(&mut session.as_vm_context())
    };
    #[cfg(not(target_arch = "wasm32"))]
    let run_result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(run_vm));
    #[cfg(target_arch = "wasm32")]
    let run_result: Result<i32, ()> = Ok(run_vm());
    let vm = match vm_slot.take() {
        Some(vm) => vm,
        None => {
            session.runtime.running = false;
            return Err("VM 状态异常：运行前未正确初始化".to_string());
        }
    };

    match run_result {
        Ok(ret) => {
            if vm.has_error() {
                session.runtime.error = vm.get_error().to_string();
                session.runtime.running = false;
                session.vm = Some(vm);
                Err(session.runtime.error.clone())
            } else if session.runtime.waiting_input {
                session.vm = Some(vm);
                Ok((ret, true))
            } else {
                // E-P1-5：运行完成提示是引擎附注，走 note 通道。
                session.runtime.push_note(format!("程序运行完成，返回值：{}\n", ret));
                append_leak_report(session);
                session.runtime.running = false;
                session.vm = Some(vm);
                Ok((ret, false))
            }
        }
        Err(_) => {
            session.vm = Some(vm);
            Err("运行时发生内部错误（panic）".to_string())
        }
    }
}
