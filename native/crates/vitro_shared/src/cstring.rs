//! C 字符串字面量的字节通道（P4，2026-09-18）。
//!
//! C 字符串的字节域是 0..=255，而 lexer 的 token 值是 Rust `String`
//! （UTF-8）——`\xHH`（HH ≥ 0x80）解码出的 `char`（U+0080..=U+00FF）
//! 一旦进 `String`，`as_bytes()` 会重编码为 2 字节 UTF-8（`"\xff"` 落
//! 0xC3 0xBF，clang 是单字节 0xFF）。
//!
//! 约定（本模块是唯一真相源）：**码点 ≤ 0xFF 的字符按 Latin-1 单字节
//! 写出，> 0xFF 的按 UTF-8 编码写出**。无歧义依据：源文件按 UTF-8 解
//! 码，多字节源字符的码点必然 > 0xFF；码点落在 U+0080..=U+00FF 的
//! `char` 只可能来自 `\xHH` 转义（或 `char::from` 语义等价物）。
//! 已知差异（诚实记录）：源内直接书写的高位单字节字符（如 Latin-1
//! 源文件的 `ü`）在本通道下写单字节，与 clang UTF-8 execution charset
//! （2 字节）不同——源内直接非 ASCII 的多字节字符（中文等，码点
//! > 0xFF）不受影响，仍按 UTF-8 写。

/// C 语义字节序列（不含结尾 NUL）。
pub fn cstring_bytes(s: &str) -> Vec<u8> {
    let mut out = Vec::with_capacity(s.len());
    for c in s.chars() {
        let cp = c as u32;
        if cp <= 0xFF {
            out.push(cp as u8);
        } else {
            let mut buf = [0u8; 4];
            out.extend_from_slice(c.encode_utf8(&mut buf).as_bytes());
        }
    }
    out
}

/// C 语义字节长度（不含结尾 NUL）——分配/对齐计算用，
/// 与 `s.len()`（UTF-8 字节数）在含 `\xHH ≥ 0x80` 转义时不同。
pub fn cstring_len(s: &str) -> usize {
    s.chars()
        .map(|c| if (c as u32) <= 0xFF { 1 } else { c.len_utf8() })
        .sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ascii_passthrough() {
        assert_eq!(cstring_bytes("hi"), b"hi");
        assert_eq!(cstring_len("hi"), 2);
    }

    /// P4 红→绿锚：\xff 转义产物（U+00FF）必须落单字节——修复前
    /// as_bytes() 路径重编码为 2 字节 UTF-8（0xC3 0xBF）。
    #[test]
    fn test_high_escape_single_byte() {
        let s = "\u{FF}\u{41}"; // \xff\x41 的解码产物
        assert_eq!(cstring_bytes(s), vec![0xFF, 0x41]);
        assert_eq!(cstring_len(s), 2);
    }

    /// 源内多字节 UTF-8 字符（中文等）按 UTF-8 写——码点 > 0xFF。
    #[test]
    fn test_multibyte_source_stays_utf8() {
        let s = "你好";
        assert_eq!(cstring_bytes(s), s.as_bytes());
        assert_eq!(cstring_len(s), 6);
    }

    /// 混合：ASCII + 高位转义 + 多字节源字符。
    #[test]
    fn test_mixed() {
        let s = "A\u{80}\u{4F60}"; // 'A' + \x80 + '你'
        assert_eq!(cstring_bytes(s), vec![0x41, 0x80, 0xE4, 0xBD, 0xA0]);
        assert_eq!(cstring_len(s), 5); // 1 + 1 + 3
    }
}
