//! E2：宏定义表——单源持有全部宏（内置预定义 + 用户 `#define`）。
//!
//! 遮蔽诊断（白箱教学层）：同名重定义时，宏体**逐 token 相同**则静默放行
//! （C 标准允许），否则记录 `W1018` 级警告提示遮蔽。警告不致命，走
//! `LexerWarning` 通道进 diagnostics。

use std::collections::HashMap;

use super::MacroDef;
use crate::token::{LexerWarning, Token};
use vitro_shared::ErrorCode;

/// 宏定义表：内置预定义宏 + 用户 `#define` 的唯一真相来源。
#[derive(Debug, Clone, Default)]
pub struct MacroTable {
    macros: HashMap<String, MacroDef>,
}

impl MacroTable {
    /// 内置宏（limits.h/stdio.h 等常量宏 + E2 预定义宏族）。
    pub fn builtin() -> Self {
        let mut t = Self { macros: crate::macros::builtin_macros() };
        // E2 预定义宏族（C23 锚定决议 §2）：
        // - `__STDC_VERSION__` 报 **202311L 名义锚点**而非保守值：bitfield 等
        //   属 C89/C99 特性，版本值再低也挡不住老代码走进不支持分支；报高值
        //   反而让 C23 新特性代码走对路径。"把版本当能力探测"的病根由三层
        //   配套解决（capabilities JSON / `__VITRO_SUBSET__` / spec 声明）。
        // - `__VITRO_SUBSET__`：条件编译的引擎专属探测宏。
        t.macros.insert(
            "__STDC_VERSION__".to_string(),
            MacroDef { params: vec![], body: vec![num_token("202311L")] },
        );
        t.macros.insert(
            "__VITRO_SUBSET__".to_string(),
            MacroDef { params: vec![], body: vec![num_token("1")] },
        );
        t
    }

    /// 定义宏。返回遮蔽警告（同名且宏体不同）——重复定义宏体相同时静默放行。
    pub fn define(&mut self, name: &str, def: MacroDef, line: i32, warnings: &mut Vec<LexerWarning>) {
        if let Some(existing) = self.macros.get(name) {
            if !tokens_identical(&existing.body, &def.body) || existing.params != def.params {
                warnings.push(LexerWarning {
                    message: format!(
                        "宏 '{}' 被重复定义且宏体不同（遮蔽了先前的定义）。若非有意为之，请先 #undef 或改名",
                        name
                    ),
                    line,
                    column: 0,
                    code: ErrorCode::W1018_MacroShadowing as i32,
                });
            }
        }
        if self.macros.len() < 4096 {
            self.macros.insert(name.to_string(), def);
        }
    }

    pub fn get(&self, name: &str) -> Option<&MacroDef> {
        self.macros.get(name)
    }

    pub fn contains(&self, name: &str) -> bool {
        self.macros.contains_key(name)
    }

    /// `#undef`（E2：条件编译下切换宏定义的教学常见写法）。
    pub fn undefine(&mut self, name: &str) {
        self.macros.remove(name);
    }
}

fn num_token(text: &str) -> Token {
    Token { ty: crate::token::TokenType::Number, text: text.to_string(), line: 0, column: 0 }
}

/// 宏体逐 token 相等比较（文本与类型都一致才算"相同定义"）。
fn tokens_identical(a: &[Token], b: &[Token]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    a.iter().zip(b.iter()).all(|(x, y)| x.ty == y.ty && x.text == y.text)
}
