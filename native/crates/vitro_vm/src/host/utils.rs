use super::VitroVM;
use vitro_shared::SourceLoc;

pub(crate) fn read_cbytes(vm: &VitroVM, addr: u32) -> Vec<u8> {
    let mem = vm.get_memory_slice();
    let start = addr as usize;
    if start >= mem.len() {
        return Vec::new();
    }
    mem[start..].iter().take_while(|&&b| b != 0).copied().collect()
}

pub(crate) fn read_cstring(vm: &VitroVM, addr: u32) -> String {
    let bytes = read_cbytes(vm, addr);
    String::from_utf8_lossy(&bytes).into_owned()
}

/// 当前时间戳（毫秒）。
#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn current_time_millis() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

/// Web 平台使用 `js_sys::Date::now()` 获取时间戳（毫秒）。
#[cfg(target_arch = "wasm32")]
pub(crate) fn current_time_millis() -> u64 {
    js_sys::Date::now() as u64
}

/// 跳过 printf 格式字符串中的修饰符（宽度、精度、长度等），返回真正的格式字母列表。
/// 例如 "%6d" 返回 ['d']，"%.2f" 返回 ['f']，"%%" 不返回任何内容。
/// 解析一个 printf/scanf 格式说明符（% 之后的内容）。
/// 返回 (格式字母, 是否 ll, 精度)。
#[allow(clippy::type_complexity)]
pub(crate) fn parse_format_spec(
    chars: &mut std::iter::Peekable<std::str::Chars<'_>>,
) -> Option<(char, bool, Option<usize>, Option<usize>, String)> {
    // 收集 flags
    let mut flags = String::new();
    while let Some(&c) = chars.peek() {
        if c == '-' || c == '+' || c == ' ' || c == '#' || c == '0' {
            if !flags.contains(c) {
                flags.push(c);
            }
            chars.next();
        } else {
            break;
        }
    }
    // 解析 width
    let mut width: Option<usize> = None;
    let mut width_str = String::new();
    while let Some(&c) = chars.peek() {
        if c.is_ascii_digit() {
            width_str.push(c);
            chars.next();
        } else if c == '*' {
            chars.next();
            break;
        } else {
            break;
        }
    }
    if !width_str.is_empty() {
        width = width_str.parse().ok();
    }
    // 解析 precision
    let mut precision: Option<usize> = None;
    if let Some(&'.') = chars.peek() {
        chars.next();
        let mut prec_str = String::new();
        while let Some(&c) = chars.peek() {
            if c.is_ascii_digit() {
                prec_str.push(c);
                chars.next();
            } else if c == '*' {
                chars.next();
                break;
            } else {
                break;
            }
        }
        if !prec_str.is_empty() {
            precision = prec_str.parse().ok();
        }
    }
    // 跳过长度修饰符，记录是否 ll
    let mut is_ll = false;
    if let Some(&c) = chars.peek() {
        if c == 'l' || c == 'h' || c == 'L' || c == 'z' || c == 'j' || c == 't' {
            chars.next();
            if let Some(&c2) = chars.peek() {
                if c == 'l' && c2 == 'l' {
                    is_ll = true;
                    chars.next();
                } else if c == 'h' && c2 == 'h' {
                    chars.next();
                }
            }
        }
    }
    // 格式字母
    chars.peek().copied().map(|spec| {
        chars.next();
        (spec, is_ll, precision, width, flags)
    })
}

pub(crate) fn parse_format_specs(fmt: &str) -> Vec<char> {
    let mut specs = Vec::new();
    let mut chars = fmt.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '%' {
            if let Some(&next) = chars.peek() {
                if next == '%' {
                    chars.next(); // 跳过 %%
                } else if let Some((spec, _, _, _, _)) = parse_format_spec(&mut chars) {
                    specs.push(spec);
                }
            }
        }
    }
    specs
}

pub(crate) fn apply_width(s: &str, width: Option<usize>, flags: &str) -> String {
    let w = match width {
        Some(w) => w,
        None => return s.to_string(),
    };
    if s.len() >= w {
        return s.to_string();
    }
    let pad_len = w - s.len();
    let left_align = flags.contains('-');
    let zero_pad = flags.contains('0') && !left_align;
    let pad_char = if zero_pad { '0' } else { ' ' };
    if left_align {
        format!("{}{}", s, pad_char.to_string().repeat(pad_len))
    } else {
        format!("{}{}", pad_char.to_string().repeat(pad_len), s)
    }
}

pub(crate) fn trim_trailing_zeros_and_dot(s: &str) -> String {
    if !s.contains('.') {
        return s.to_string();
    }
    let mut result = s.trim_end_matches('0').to_string();
    if result.ends_with('.') {
        result.pop();
    }
    result
}

pub(crate) fn format_g(val: f64, prec: usize, upper: bool) -> String {
    if val.is_nan() {
        return if upper { "NAN" } else { "nan" }.to_string();
    }
    if val.is_infinite() {
        return if val.is_sign_positive() {
            if upper { "INF" } else { "inf" }.to_string()
        } else {
            if upper { "-INF" } else { "-inf" }.to_string()
        };
    }
    if val == 0.0 {
        return "0".to_string();
    }

    let prec = prec.max(1);
    let abs_val = val.abs();
    let exp = abs_val.log10().floor() as i32;

    let mut result;
    let e_char = if upper { 'E' } else { 'e' };

    if exp < -4 || exp >= prec as i32 {
        let mantissa = abs_val / 10f64.powi(exp);
        let s = format!("{:.*}", prec - 1, mantissa);
        let s = trim_trailing_zeros_and_dot(&s);
        result = format!("{}{}{:+#03}", s, e_char, exp);
    } else {
        let frac_digits = (prec as i32 - 1 - exp).max(0) as usize;
        let s = format!("{:.*}", frac_digits, abs_val);
        result = trim_trailing_zeros_and_dot(&s);
    }

    if val < 0.0 && !result.starts_with('-') {
        result = format!("-{}", result);
    }
    result
}

/// 根据格式字符串和参数列表生成 printf 输出。
pub(crate) fn format_printf_string(vm: &VitroVM, fmt: &str, args: &[u64]) -> String {
    let mut out = String::new();
    let mut used = 0;
    let mut chars = fmt.chars().peekable();
    while let Some(ch) = chars.next() {
        if used < args.len() && ch == '%' {
            if let Some(&next) = chars.peek() {
                if next == '%' {
                    out.push('%');
                    chars.next();
                } else if let Some((spec, is_ll, precision, width, flags)) = parse_format_spec(&mut chars) {
                    let arg = args[used];
                    let mut piece = String::new();
                    match spec {
                        'd' | 'i' => {
                            let val = if is_ll {
                                (arg as i64).to_string()
                            } else {
                                (arg as i32).to_string()
                            };
                            piece = apply_width(&val, width, &flags);
                        }
                        'u' => {
                            let val = if is_ll {
                                arg.to_string()
                            } else {
                                (arg as u32).to_string()
                            };
                            piece = apply_width(&val, width, &flags);
                        }
                        'x' => {
                            let val = if is_ll {
                                format!("{:x}", arg)
                            } else {
                                format!("{:x}", arg as u32)
                            };
                            piece = apply_width(&val, width, &flags);
                        }
                        'X' => {
                            let val = if is_ll {
                                format!("{:X}", arg)
                            } else {
                                format!("{:X}", arg as u32)
                            };
                            piece = apply_width(&val, width, &flags);
                        }
                        'o' => {
                            let val = if is_ll {
                                format!("{:o}", arg)
                            } else {
                                format!("{:o}", arg as u32)
                            };
                            piece = apply_width(&val, width, &flags);
                        }
                        'p' => {
                            let val = format!("{:p}", arg as u32 as *const ());
                            piece = apply_width(&val, width, &flags);
                        }
                        'f' => {
                            let f = f64::from_bits(arg);
                            let prec = precision.unwrap_or(6);
                            let val = format!("{:.*}", prec, f);
                            piece = apply_width(&val, width, &flags);
                        }
                        'g' => {
                            let f = f64::from_bits(arg);
                            let prec = precision.unwrap_or(6);
                            let val = format_g(f, prec, false);
                            piece = apply_width(&val, width, &flags);
                        }
                        'G' => {
                            let f = f64::from_bits(arg);
                            let prec = precision.unwrap_or(6);
                            let val = format_g(f, prec, true);
                            piece = apply_width(&val, width, &flags);
                        }
                        's' => {
                            let val = read_cstring(vm, arg as u32);
                            piece = apply_width(&val, width, &flags);
                        }
                        'c' => {
                            let val = (arg as u8 as char).to_string();
                            piece = apply_width(&val, width, &flags);
                        }
                        _ => {
                            piece.push(ch);
                            piece.push(spec);
                        }
                    }
                    out.push_str(&piece);
                    used += 1;
                }
            } else {
                out.push(ch);
            }
        } else {
            out.push(ch);
        }
    }
    out
}

/// scanf 格式串解析项。
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum ScanfItem {
    /// `%` 转换符：(格式字母, 长度修饰符级别 0=无, 1=l/h, 2=ll)
    Spec(char, i32),
    /// 格式串中的空白指令（C11 7.21.6.2）：匹配输入中任意数量（含零）的
    /// 空白字符，直到首个非空白字符。`%c` 不自动跳白，空白指令是学生控制
    /// `scanf("%d %c")` 读到非空白字符的唯一手段（Issue A）。
    Whitespace,
    /// **普通字符指令**（C11 7.21.6.2，条目 4，2026-09-11 补齐）：格式串中的
    /// 非空白非 `%` 字符（含 `%%` 展开的 `%`），需与输入流的下一个字符**精确匹配**，
    /// 不匹配即停止解析。此前被整段忽略，`scanf("a=%d", &x)` 读 `a=5` 会得到 `x=0`。
    Literal(u8),
}

/// 解析 scanf 格式字符串，返回转换符、空白指令与普通字符指令的有序序列。
///
/// 普通字符指令（条目 4，2026-09-11 起实现精确匹配）：格式串中的非空白非 `%` 字符，
/// 以及与输入流比较的 `%%`（展开为字面 `%`）。非 ASCII 字面字符仍被忽略
/// （教学格式串中不出现；需要时按 UTF-8 多字节匹配另行设计）。
pub(crate) fn parse_scanf_specs(fmt: &str) -> Vec<ScanfItem> {
    let mut specs = Vec::new();
    let mut chars = fmt.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '%' {
            if let Some(&next) = chars.peek() {
                if next == '%' {
                    // `%%`：格式串中的字面 `%`，需与输入精确匹配
                    specs.push(ScanfItem::Literal(b'%'));
                    chars.next();
                } else {
                    // 跳过 flags
                    while let Some(&c) = chars.peek() {
                        if c == '-' || c == '+' || c == ' ' || c == '#' || c == '0' {
                            chars.next();
                        } else {
                            break;
                        }
                    }
                    // 跳过 width
                    while let Some(&c) = chars.peek() {
                        if c.is_ascii_digit() || c == '*' {
                            chars.next();
                        } else {
                            break;
                        }
                    }
                    // 跳过 precision
                    if let Some(&'.') = chars.peek() {
                        chars.next();
                        while let Some(&c) = chars.peek() {
                            if c.is_ascii_digit() || c == '*' {
                                chars.next();
                            } else {
                                break;
                            }
                        }
                    }
                    // 读取长度修饰符
                    let mut len_mod = 0i32;
                    if let Some(&c) = chars.peek() {
                        if c == 'l' {
                            len_mod = 1;
                            chars.next();
                            if let Some(&c2) = chars.peek() {
                                if c2 == 'l' {
                                    len_mod = 2;
                                    chars.next();
                                }
                            }
                        } else if c == 'h' {
                            len_mod = 1;
                            chars.next();
                            if let Some(&c2) = chars.peek() {
                                if c2 == 'h' {
                                    chars.next();
                                }
                            }
                        } else if c == 'L' {
                            chars.next();
                        }
                    }
                    // 真正的格式字母
                    if let Some(&spec) = chars.peek() {
                        specs.push(ScanfItem::Spec(spec, len_mod));
                        chars.next();
                    }
                }
            }
        } else if ch.is_whitespace() {
            specs.push(ScanfItem::Whitespace);
        } else if ch.is_ascii() {
            // 普通字符指令：需与输入流的下一个字符精确匹配（此前被整段忽略）
            specs.push(ScanfItem::Literal(ch as u8));
        }
    }
    specs
}

/// 从 chars 中读取一个浮点数字符串，返回 (token_string, new_pos)。
pub(crate) fn read_float_token(chars: &[char], mut pos: usize) -> (String, usize) {
    while pos < chars.len() && chars[pos].is_whitespace() {
        pos += 1;
    }
    if pos >= chars.len() {
        return (String::new(), pos);
    }
    let start = pos;
    if chars[pos] == '+' || chars[pos] == '-' {
        pos += 1;
    }
    while pos < chars.len() && (chars[pos].is_ascii_digit() || chars[pos] == '.') {
        pos += 1;
    }
    let token: String = chars[start..pos].iter().collect();
    (token, pos)
}

pub(crate) trait MemorySlice {
    fn get_memory_slice(&self) -> &[u8];
}

impl MemorySlice for VitroVM {
    fn get_memory_slice(&self) -> &[u8] {
        self.memory_ref()
    }
}

pub(crate) fn read_fd_from_stream(vm: &VitroVM, stream: u32) -> u32 {
    if stream == 0 {
        return 0;
    }
    let mem = vm.get_memory_slice();
    let start = stream as usize;
    if start + 4 > mem.len() {
        return 0;
    }
    i32::from_le_bytes([mem[start], mem[start + 1], mem[start + 2], mem[start + 3]]) as u32
}

// ========== Phase A libc extensions ==========

pub(crate) fn set_errno(vm: &mut VitroVM, val: i32) {
    for sym in vm.get_symbols() {
        if sym.name == "errno" && !sym.is_local {
            let _ = vm.write_memory(sym.addr, &val.to_le_bytes());
            return;
        }
    }
}

/// V-P1-6：栈缓冲区容量校验。dest 若落在某个活跃栈帧的局部数组缓冲区内，
/// 校验 available >= needed；不足则给出 E3070 教学 trap 并返回 true。
/// 此前 strcpy/strcat/scanf("%s") 只查堆 region，栈上 `char buf[4]` 被
/// 静默覆写相邻局部变量（教学 IDE 最需要捕获的经典错误）。
pub(crate) fn check_stack_buffer_capacity(vm: &mut VitroVM, dest: u32, needed: usize, op_name: &str) -> bool {
    for frame in vm.call_stack.iter().rev() {
        for buf in &frame.local_buffers {
            let buf_addr = frame.locals_base as u64 + buf.offset as u64;
            let dest64 = dest as u64;
            if dest64 >= buf_addr && dest64 < buf_addr + buf.size as u64 {
                let available = (buf_addr + buf.size as u64 - dest64) as usize;
                if needed > available {
                    vm.trap(
                        &format!(
                            "💥 Buffer Overflow (E3070)：{} 写入栈上缓冲区 '{}' 溢出。需要写入 {} 字节，但该缓冲区从当前写入位置起仅剩 {} 字节（总大小 {} 字节）。

💡 原因：{} 不检查目标缓冲区大小，栈上数组越界会破坏相邻局部变量。
✅ 解决方法：增大数组大小，或改用带长度限制的版本（如 strncpy/fgets）。",
                            op_name, buf.name, needed, available, buf.size, op_name
                        ),
                        &SourceLoc::default(),
                    );
                    return true;
                }
                return false; // 命中缓冲区且容量足够
            }
        }
    }
    false
}
