use super::*;
use crate::VmContext;

pub fn host_printf_n(vm: &mut VitroVM, session: &mut VmContext<'_>) {
    let fmt_addr = vm.pop() as u32;
    let fmt = read_cstring(vm, fmt_addr);
    let specs = parse_format_specs(&fmt);
    if vm.get_stack().len() < specs.len() {
        vm.trap("printf: 格式字符串要求的参数多于实际提供的参数。", &SourceLoc::default());
        return;
    }
    let mut args = Vec::with_capacity(specs.len());
    for _ in 0..specs.len() {
        args.push(vm.pop());
    }
    let out = format_printf_string(vm, &fmt, &args);
    session.runtime.push_stdout(out);
}

pub fn host_scanf_n(vm: &mut VitroVM, session: &mut VmContext<'_>) {
    let fmt_addr = vm.pop() as u32;
    let fmt = read_cstring(vm, fmt_addr);
    // 扫描格式字符串，记录每个 % 格式符的类型、长度修饰符及空白指令
    let spec_types = parse_scanf_specs(&fmt);
    // 仅 % 转换符消耗指针参数（空白指令不取参）
    let arg_count = spec_types
        .iter()
        .filter(|s| matches!(s, ScanfItem::Spec(..)))
        .count();
    if vm.get_stack().len() < arg_count {
        vm.trap("scanf: 格式字符串要求的参数多于实际提供的参数。", &SourceLoc::default());
        return;
    }
    // 按数量 pop 指针参数
    let mut ptrs = Vec::with_capacity(arg_count);
    for _ in 0..arg_count {
        ptrs.push(vm.pop() as u32);
    }
    // V-P1-13：scanf 改为与 getchar 一致的字符流游标语义。
    // 此前每次调用整行消费（input_index += 1）：输入 "1 2\n3 4" 时第二次
    // scanf("%d") 读到 3（C 流式语义应读同行剩余的 2），%c 也读不到行尾。
    // 现从 (input_index, input_char_offset) 起拼接虚拟字节流（行间补 '\n'
    // 分隔，行本身可能不带换行），解析后按实际消费量经映射表推进游标。
    let mut stream: Vec<u8> = Vec::new();
    // stream[i] 的原始位置：(行号, 行内偏移)；off 为 -1 表示行间补位 '\n'
    let mut mapping: Vec<(usize, i32)> = Vec::new();
    {
        let lines = &session.runtime.input_lines;
        let start_idx = session.runtime.input_index;
        let start_off = session.runtime.input_char_offset;
        for (i, line) in lines.iter().enumerate().skip(start_idx) {
            let from = if i == start_idx { start_off.min(line.len()) } else { 0 };
            for (off, &b) in line.as_bytes()[from..].iter().enumerate() {
                mapping.push((i, (from + off) as i32));
                stream.push(b);
            }
            // 行不含换行符时补一个逻辑 '\n' 作为行分隔（C 的 stdin 是连续流）；
            // 消费补位后游标直达下一行首
            if !line.ends_with('\n') {
                mapping.push((i + 1, -1));
                stream.push(b'\n');
            }
        }
    }
    // 首个转换符前流已无可用内容：
    // - Batch（判分/批量）模式：输入耗尽即 EOF，返回 -1（C11 7.21.6.2：读到输入
    //   结束且未成功赋值任何项时返回 EOF）。此前无此分支，`while (scanf(...) != EOF)`
    //   这类 C 第一课习语在批量输入下会永久挂起 `waiting_input`。
    // - Interactive（默认）模式：保留"等待学生键入"的挂起语义。
    //
    // A1 遗留分支：置位 `stdin_eof` 粘滞标志并把游标推到底。否则未消费的尾部空白
    // 会被后续 `getchar` 当普通字符读出（实测输入 `7\n`，Clang 给 -1、Vitro 给 10）。
    if !stream.iter().any(|c| !c.is_ascii_whitespace()) {
        if session.runtime.input_mode == vitro_runtime::InputMode::Batch {
            session.runtime.set_stdin_eof();
            vm.push((-1i32) as i64 as u64);
            return;
        }
        for &p in ptrs.iter().rev() {
            vm.push(p as u64);
        }
        vm.push(fmt_addr as u64);
        session.runtime.waiting_input = true;
        return;
    }
    // 已粘滞 EOF：不再解析（C11 7.21.5.1 feof 语义）
    if session.runtime.stdin_eof {
        if session.runtime.input_mode == vitro_runtime::InputMode::Batch {
            session.runtime.set_stdin_eof();
            vm.push((-1i32) as i64 as u64);
            return;
        }
        for &p in ptrs.iter().rev() {
            vm.push(p as u64);
        }
        vm.push(fmt_addr as u64);
        session.runtime.waiting_input = true;
        return;
    }
    let chars: Vec<u8> = stream;
    let mut pos = 0usize;
    // 依次解析并写入各指针地址（空白指令只跳白不取参）
    let mut arg_idx = 0usize;
    // C11 7.21.6.2：scanf 返回"成功匹配并赋值的项数"（条目 3，2026-09-11 补齐）
    let mut matched = 0usize;
    // 因**输入流耗尽**（而非转换/字面量不匹配）而中止解析：用于判定 stdin EOF
    let mut exhausted = false;
    for item in spec_types.iter() {
        let (spec, len_mod) = match item {
            ScanfItem::Whitespace => {
                // C11 7.21.6.2：匹配输入中任意数量（含零）的空白
                while pos < chars.len() && chars[pos].is_ascii_whitespace() {
                    pos += 1;
                }
                continue;
            }
            ScanfItem::Literal(expected) => {
                // 普通字符指令（条目 4）：与输入流的下一个字符**精确比较**（不跳空白）。
                // 不相等则按 C11 7.21.6.2 **停止解析**（输入流保持不动），
                // 返回已成功赋值的项数。
                if pos >= chars.len() || chars[pos] != *expected {
                    break;
                }
                pos += 1;
                continue;
            }
            ScanfItem::Spec(spec, len_mod) => (*spec, *len_mod),
        };
        let ptr = ptrs[arg_idx];
        arg_idx += 1;
        match spec {
            'd' => {
                // 跳过前导空白
                while pos < chars.len() && chars[pos].is_ascii_whitespace() {
                    pos += 1;
                }
                if pos >= chars.len() {
                    exhausted = true;
                    break;
                }
                let start = pos;
                if chars[pos] == b'+' || chars[pos] == b'-' {
                    pos += 1;
                }
                while pos < chars.len() && chars[pos].is_ascii_digit() {
                    pos += 1;
                }
                let token: String = chars[start..pos].iter().map(|&b| b as char).collect();
                if len_mod >= 2 {
                    // %lld → long long (8 bytes)
                    let value: i64 = token.parse().unwrap_or(0);
                    vm.store_i64(ptr, value as u64, &SourceLoc::default());
                } else {
                    let value: i32 = token.parse().unwrap_or(0);
                    vm.store_i32(ptr, value, &SourceLoc::default());
                }
            }
            'u' => {
                while pos < chars.len() && chars[pos].is_ascii_whitespace() {
                    pos += 1;
                }
                if pos >= chars.len() {
                    exhausted = true;
                    break;
                }
                let start = pos;
                if chars[pos] == b'+' {
                    pos += 1;
                }
                while pos < chars.len() && chars[pos].is_ascii_digit() {
                    pos += 1;
                }
                let token: String = chars[start..pos].iter().map(|&b| b as char).collect();
                if len_mod >= 2 {
                    let value: u64 = token.parse().unwrap_or(0);
                    vm.store_i64(ptr, value, &SourceLoc::default());
                } else {
                    let value: u32 = token.parse().unwrap_or(0);
                    vm.store_i32(ptr, value as i32, &SourceLoc::default());
                }
            }
            'f' => {
                // 跳前导空白（%f 与 %d 同：转换前跳白）
                while pos < chars.len() && chars[pos].is_ascii_whitespace() {
                    pos += 1;
                }
                if pos >= chars.len() {
                    exhausted = true;
                    break;
                }
                let chars_view: Vec<char> = chars.iter().map(|&b| b as char).collect();
                let (token, new_pos) = read_float_token(&chars_view, pos);
                pos = new_pos;
                if token.is_empty() {
                    // 有非空白内容但解析不出浮点 token：格式不匹配（非 EOF）
                    break;
                }
                if len_mod >= 1 {
                    // %lf → double (8 bytes)
                    let value: f64 = token.parse().unwrap_or(0.0);
                    vm.store_i64(ptr, value.to_bits(), &SourceLoc::default());
                } else {
                    // %f → float (4 bytes)
                    let value: f32 = token.parse().unwrap_or(0.0);
                    vm.store_i32(ptr, value.to_bits() as i32, &SourceLoc::default());
                }
            }
            'c' => {
                // 标准 C: %c 不跳过空白（流式化后行尾字符也可被读到）
                if pos >= chars.len() {
                    exhausted = true;
                    break;
                }
                let ch = chars[pos];
                vm.store_i8(ptr, ch as i32, &SourceLoc::default());
                pos += 1;
            }
            's' => {
                // 跳过前导空白
                while pos < chars.len() && chars[pos].is_ascii_whitespace() {
                    pos += 1;
                }
                if pos >= chars.len() {
                    exhausted = true;
                    break;
                }
                let start = pos;
                while pos < chars.len() && !chars[pos].is_ascii_whitespace() {
                    pos += 1;
                }
                let token: String = chars[start..pos].iter().map(|&b| b as char).collect();
                // V-P1-6：栈上缓冲区容量校验（token + '\0'）
                if check_stack_buffer_capacity(vm, ptr, token.len() + 1, "scanf(\"%s\")") {
                    return;
                }
                // 写入目标缓冲区并追加 '\0'
                for (j, ch) in token.chars().enumerate() {
                    vm.store_i8(ptr + j as u32, ch as i32, &SourceLoc::default());
                }
                vm.store_i8(ptr + token.len() as u32, 0, &SourceLoc::default());
            }
            _ => {
                // 不支持的转换符：未消费输入，不计入"成功赋值项数"
                continue;
            }
        }
        matched += 1;
    }
    // V-P1-13：按实际消费量经映射表推进游标（未消费部分留给后续输入函数）
    if pos > 0 {
        if let Some(&(idx, off)) = mapping.get(pos - 1) {
            if off < 0 {
                // 行间补位 '\n' 被消费：游标直达下一行首
                session.runtime.input_index = idx;
                session.runtime.input_char_offset = 0;
            } else {
                let line_len = session.runtime.input_lines.get(idx).map(|l| l.len()).unwrap_or(0);
                session.runtime.input_index = idx;
                session.runtime.input_char_offset = ((off + 1) as usize).min(line_len);
            }
        }
    }
    // A1 遗留分支：解析走到流末端仍有转换符未成功赋值 → stdin 已到 EOF。
    // 置位粘滞标志并推游标到底（否则未消费的尾部空白会被后续 getchar 读出）。
    //
    // 判据必须是"**因流耗尽**而 break"，不能只看 `matched < arg_count`：
    // `scanf("a=%d", &x)` 输入 `b=1` 时字面量不匹配也会 break，但那是转换失败
    // （输入流保持不动、非 EOF），误置 EOF 位会让后续读取全部失效。
    if session.runtime.input_mode == vitro_runtime::InputMode::Batch && exhausted {
        session.runtime.set_stdin_eof();
    }
    // 返回值：成功匹配并赋值的项数（与 sscanf 一致；此前 scanf 不返回值，
    // 教学代码 `int r = scanf(...)` 会被 typeck 判为 void→int 错误，见条目 3）
    vm.push(matched as u64);
}

pub fn host_ungetc(vm: &mut VitroVM, session: &mut VmContext<'_>) {
    let ch = vm.pop() as i32;
    let _stream = vm.pop() as i32;
    session.runtime.ungetc_char = Some(ch);
    vm.push(ch as i64 as u64);
}

pub fn host_getchar(vm: &mut VitroVM, session: &mut VmContext<'_>) {
    // 先检查 ungetc 缓存
    if let Some(ch) = session.runtime.ungetc_char.take() {
        vm.push(ch as i64 as u64);
        return;
    }
    // A1 遗留分支：stdin EOF 粘滞 —— 一旦置位，后续 getchar 恒 -1。
    // 此前不检查该位，scanf 判定的 EOF 对 getchar 不可见（实测输入 `7\n`
    // 时 getchar 会把 scanf 未消费的 '\n' 当普通字符返回 10，Clang 返回 -1）。
    if session.runtime.stdin_eof {
        if session.runtime.input_mode == vitro_runtime::InputMode::Batch {
            vm.push((-1i32) as i64 as u64);
        } else {
            session.runtime.waiting_input = true;
        }
        return;
    }
    // 先检查是否有可用输入
    let has_input = {
        let mut idx = session.runtime.input_index;
        let mut offset = session.runtime.input_char_offset;
        let mut found = false;
        while idx < session.runtime.input_lines.len() {
            if offset < session.runtime.input_lines[idx].len() {
                found = true;
                break;
            }
            idx += 1;
            offset = 0;
        }
        found
    };
    if !has_input {
        if session.runtime.input_mode == vitro_runtime::InputMode::Batch {
            // Batch 模式：输入耗尽后返回 EOF (-1)，并置位粘滞标志
            session.runtime.set_stdin_eof();
            vm.push((-1i32) as u64);
        } else {
            session.runtime.waiting_input = true;
        }
        return;
    }
    let mut result = -1i32;
    while session.runtime.input_index < session.runtime.input_lines.len() {
        let line = &session.runtime.input_lines[session.runtime.input_index];
        if session.runtime.input_char_offset < line.len() {
            let ch = line.as_bytes()[session.runtime.input_char_offset];
            session.runtime.input_char_offset += 1;
            result = ch as i32;
            break;
        } else {
            session.runtime.input_index += 1;
            session.runtime.input_char_offset = 0;
        }
    }
    vm.push(result as u64);
}

pub fn host_putchar(vm: &mut VitroVM, session: &mut VmContext<'_>) {
    let val = vm.pop();
    session.runtime.push_stdout((val as u8 as char).to_string());
}

pub fn host_fprintf_n(vm: &mut VitroVM, session: &mut VmContext<'_>) {
    let stream = vm.pop();
    let fmt_addr = vm.pop() as u32;
    let fmt = read_cstring(vm, fmt_addr);
    let specs = parse_format_specs(&fmt);
    if vm.get_stack().len() < specs.len() {
        vm.trap("fprintf: 格式字符串要求的参数多于实际提供的参数。", &SourceLoc::default());
        return;
    }
    let mut args = Vec::with_capacity(specs.len());
    for _ in 0..specs.len() {
        args.push(vm.pop());
    }
    let out = format_printf_string(vm, &fmt, &args);
    // E-P1-5：stderr(2) 分流到 stderr 通道，不再混入 stdout；stdout(1) 与其它流
    // 维持既有"直接输出"行为（fprintf 到自定义 FILE* 未落盘属既有偏差，另行记录）。
    if stream == 2 {
        session.runtime.push_stderr(out);
    } else {
        session.runtime.push_stdout(out);
    }
}

pub fn host_puts(vm: &mut VitroVM, session: &mut VmContext<'_>) {
    let s_addr = vm.pop() as u32;
    let s = read_cstring(vm, s_addr);
    session.runtime.push_stdout(s + "\n");
    vm.push(1); // puts returns non-negative on success
}

pub fn host_sprintf(vm: &mut VitroVM, _session: &mut VmContext<'_>) {
    let buf_addr = vm.pop() as u32;
    let fmt_addr = vm.pop() as u32;
    let fmt = read_cstring(vm, fmt_addr);
    let specs = parse_format_specs(&fmt);
    let mut args = Vec::with_capacity(specs.len());
    for _ in 0..specs.len() {
        args.push(vm.pop());
    }
    let out = format_printf_string(vm, &fmt, &args);
    let bytes = out.as_bytes();
    for (i, &b) in bytes.iter().enumerate() {
        vm.store_i8(buf_addr + i as u32, b as i32, &SourceLoc::default());
    }
    vm.store_i8(buf_addr + bytes.len() as u32, 0, &SourceLoc::default());
    vm.push(bytes.len() as u64);
}

pub fn host_snprintf(vm: &mut VitroVM, _session: &mut VmContext<'_>) {
    let buf_addr = vm.pop() as u32;
    let size = vm.pop() as i32;
    let fmt_addr = vm.pop() as u32;
    let fmt = read_cstring(vm, fmt_addr);
    let specs = parse_format_specs(&fmt);
    let mut args = Vec::with_capacity(specs.len());
    for _ in 0..specs.len() {
        args.push(vm.pop());
    }
    let out = format_printf_string(vm, &fmt, &args);
    let bytes = out.as_bytes();
    if size > 0 {
        let n = std::cmp::min(bytes.len(), (size as usize).saturating_sub(1));
        for (i, &byte) in bytes.iter().enumerate().take(n) {
            vm.store_i8(buf_addr + i as u32, byte as i32, &SourceLoc::default());
        }
        vm.store_i8(buf_addr + n as u32, 0, &SourceLoc::default());
    }
    vm.push(bytes.len() as u64);
}

pub fn host_sscanf(vm: &mut VitroVM, _session: &mut VmContext<'_>) {
    let str_addr = vm.pop() as u32;
    let fmt_addr = vm.pop() as u32;
    let fmt = read_cstring(vm, fmt_addr);
    let src = read_cstring(vm, str_addr);
    let spec_types = parse_scanf_specs(&fmt);
    let arg_count = spec_types
        .iter()
        .filter(|s| matches!(s, ScanfItem::Spec(..)))
        .count();
    let mut ptrs = Vec::with_capacity(arg_count);
    for _ in 0..arg_count {
        ptrs.push(vm.pop() as u32);
    }
    let chars: Vec<char> = src.chars().collect();
    let mut pos = 0usize;
    let mut matched = 0usize;
    let mut arg_idx = 0usize;
    for item in spec_types.iter() {
        let (spec, len_mod) = match item {
            ScanfItem::Whitespace => {
                // C11 7.21.6.2：匹配任意数量（含零）的空白
                while pos < chars.len() && chars[pos].is_whitespace() {
                    pos += 1;
                }
                continue;
            }
            ScanfItem::Literal(expected) => {
                // 普通字符指令（条目 4）：与源串下一个字符精确比较，不等即停止解析
                // （sscanf 与 scanf 同族同修，与空白指令修复的先例一致）
                if pos >= chars.len() || chars[pos] as u32 != *expected as u32 {
                    break;
                }
                pos += 1;
                continue;
            }
            ScanfItem::Spec(spec, len_mod) => (*spec, *len_mod),
        };
        let ptr = ptrs[arg_idx];
        arg_idx += 1;
        match spec {
            'd' => {
                while pos < chars.len() && chars[pos].is_whitespace() {
                    pos += 1;
                }
                if pos >= chars.len() {
                    break;
                }
                let start = pos;
                if chars[pos] == '+' || chars[pos] == '-' {
                    pos += 1;
                }
                while pos < chars.len() && chars[pos].is_ascii_digit() {
                    pos += 1;
                }
                let token: String = chars[start..pos].iter().collect();
                if len_mod >= 2 {
                    let value: i64 = token.parse().unwrap_or(0);
                    vm.store_i64(ptr, value as u64, &SourceLoc::default());
                } else {
                    let value: i32 = token.parse().unwrap_or(0);
                    vm.store_i32(ptr, value, &SourceLoc::default());
                }
                matched += 1;
            }
            'u' => {
                while pos < chars.len() && chars[pos].is_whitespace() {
                    pos += 1;
                }
                if pos >= chars.len() {
                    break;
                }
                let start = pos;
                if chars[pos] == '+' {
                    pos += 1;
                }
                while pos < chars.len() && chars[pos].is_ascii_digit() {
                    pos += 1;
                }
                let token: String = chars[start..pos].iter().collect();
                if len_mod >= 2 {
                    let value: u64 = token.parse().unwrap_or(0);
                    vm.store_i64(ptr, value, &SourceLoc::default());
                } else {
                    let value: u32 = token.parse().unwrap_or(0);
                    vm.store_i32(ptr, value as i32, &SourceLoc::default());
                }
                matched += 1;
            }
            'f' => {
                let (token, new_pos) = read_float_token(&chars, pos);
                pos = new_pos;
                if token.is_empty() {
                    break;
                }
                if len_mod >= 1 {
                    let value: f64 = token.parse().unwrap_or(0.0);
                    vm.store_i64(ptr, value.to_bits(), &SourceLoc::default());
                } else {
                    let value: f32 = token.parse().unwrap_or(0.0);
                    vm.store_i32(ptr, value.to_bits() as i32, &SourceLoc::default());
                }
                matched += 1;
            }
            'c' => {
                if pos >= chars.len() {
                    break;
                }
                let ch = chars[pos];
                vm.store_i8(ptr, ch as i32, &SourceLoc::default());
                pos += 1;
                matched += 1;
            }
            's' => {
                while pos < chars.len() && chars[pos].is_whitespace() {
                    pos += 1;
                }
                if pos >= chars.len() {
                    break;
                }
                let start = pos;
                while pos < chars.len() && !chars[pos].is_whitespace() {
                    pos += 1;
                }
                let token: String = chars[start..pos].iter().collect();
                let bytes = token.as_bytes();
                for (j, &b) in bytes.iter().enumerate() {
                    vm.store_i8(ptr + j as u32, b as i32, &SourceLoc::default());
                }
                vm.store_i8(ptr + bytes.len() as u32, 0, &SourceLoc::default());
                matched += 1;
            }
            _ => {}
        }
    }
    vm.push(matched as u64);
}

// ========== VFS I/O extensions ==========
