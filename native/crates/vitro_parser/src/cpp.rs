//! C++ 扩展语法解析子模块
//!
//! 将 Parser 中与 C++ 模式相关的状态初始化与顶层语法入口集中到这里，
//! 降低 `parser/mod.rs` 的认知负荷并明确 C/C++ 边界。
// TODO(#D10): 后续将 parse_program 中的 class/template 顶层分发、
// look_ahead_skip_stars 中的 C++ 引用判断继续下沉到本模块。

use vitro_ast::*;
use vitro_shared::SourceLoc;
use std::collections::{HashMap, HashSet};

use super::{Parser, TokenType};

/// 预注册 C++ 内置容器类型名与模板名。
pub(crate) fn register_cpp_builtin_types(
    typedef_names: &mut HashMap<String, Type>,
    template_names: &mut HashSet<String>,
) {
    for name in [
        "vitro_vec_int",
        "vitro_vec_float",
        "vitro_vec_char",
        "vitro_string",
        "vitro_list_int",
    ] {
        typedef_names.insert(
            name.to_string(),
            Type::Class {
                name: name.to_string(),
                is_const: false,
            },
        );
    }
    // 预注册内置容器模板名，支持显式实例化写法。
    // 注意：vitro_string 本身是最终类名，不在这里作为模板基名注册，
    // 避免与用户代码中的 typedef struct vitro_string { ... } 冲突。
    for name in ["vitro_vec", "vitro_list"] {
        template_names.insert(name.to_string());
        // 同时加入 typedef_names，使变量声明等类型上下文能识别该模板名。
        typedef_names.insert(
            name.to_string(),
            Type::Class {
                name: name.to_string(),
                is_const: false,
            },
        );
    }
}

impl Parser {
    /// 尝试解析 C++ 构造函数类外定义，如 `Counter::Counter() { ... }`。
    /// 若成功解析并压入 program.funcs，返回 true；否则不消费 token，返回 false。
    pub(crate) fn try_parse_cpp_ctor_out_of_line(
        &mut self,
        program: &mut ProgramNode,
        base_type: &Type,
        is_static: bool,
        is_extern: bool,
    ) -> bool {
        // parse_base_type 已经把类名 'Counter' 作为返回类型读入，当前 token 是 '::'。
        if !self.is_cpp_mode
            || !matches!(base_type, Type::Class { name, .. } if !name.is_empty())
            || !self.check(TokenType::ColonColon)
            || self.peek(1).ty != TokenType::Identifier
            || self.peek(2).ty != TokenType::LParen
            || self.peek(1).text != base_type.name()
        {
            return false;
        }

        self.consume(TokenType::ColonColon, "预期 '::'");
        let name_tok = self.consume(TokenType::Identifier, "预期构造函数名").clone();
        self.consume(TokenType::LParen, "预期 '('");
        let (params, _) = self.parse_param_list();
        self.consume(TokenType::RParen, "预期 ')'");
        let body = if self.check(TokenType::LBrace) {
            Some(self.parse_block())
        } else {
            self.consume(TokenType::Semicolon, "构造函数声明后预期 ';' 或 '{'");
            None
        };
        program.funcs.push(FuncDecl {
            loc: SourceLoc {
                line: name_tok.line,
                column: name_tok.column,
                file_id: 0,
            },
            return_type: Type::void(),
            name: format!("__ctor__{}", name_tok.text),
            params,
            body,
            is_static,
            is_extern,
            source_file: String::new(),
            is_variadic: false,
        });
        true
    }

    /// 尝试解析 C++ 限定静态字段定义，如 `int A::count = 0;`。
    /// 若成功解析并压入 program.globals，返回 true；否则不消费 token，返回 false。
    pub(crate) fn try_parse_cpp_qualified_static_field(
        &mut self,
        program: &mut ProgramNode,
        base_type: &Type,
        is_static: bool,
        is_extern: bool,
    ) -> bool {
        if !self.is_cpp_mode
            || self.current().ty != TokenType::Identifier
            || self.peek(1).ty != TokenType::ColonColon
            || self.peek(2).ty != TokenType::Identifier
        {
            return false;
        }

        let ty = base_type.clone();
        let class_name = self.consume(TokenType::Identifier, "预期类名").text.clone();
        self.consume(TokenType::ColonColon, "预期 '::'");
        let field_tok = self.consume(TokenType::Identifier, "预期字段名").clone();
        let name = format!("{}__{}", class_name, field_tok.text);
        let init = if is_extern {
            None
        } else if self.match_token(TokenType::Assign) {
            if self.check(TokenType::LBrace) {
                Some(self.parse_init_list())
            } else {
                Some(self.parse_assign())
            }
        } else {
            None
        };
        program.globals.push(GlobalDecl {
            loc: SourceLoc {
                line: self.previous().line,
                column: self.previous().column,
                file_id: 0,
            },
            ty: ty.clone(),
            name,
            init,
            is_static,
            is_extern,
            source_file: String::new(),
        });
        self.consume(TokenType::Semicolon, "静态成员定义后预期 ';'");
        true
    }

    /// 解析 C++ `class` 顶层声明。
    pub(crate) fn parse_cpp_class_decl(&mut self, program: &mut ProgramNode) {
        let class_decl = self.parse_class_decl();
        self.typedef_names.insert(
            class_decl.name.clone(),
            Type::Class {
                name: class_decl.name.clone(),
                is_const: false,
            },
        );
        program.classes.push(class_decl);
    }

    /// 解析 C++ `template` 顶层声明或显式实例化。
    pub(crate) fn parse_cpp_template_decl(&mut self, program: &mut ProgramNode) {
        if self.is_template_explicit_instantiation() {
            let inst = self.parse_template_instantiation();
            program.template_instantiations.push(inst);
        } else {
            let template_decl = self.parse_template_decl();
            // 将类模板名加入 typedef_names
            if let vitro_ast::Templateable::Class(ref c) = template_decl.decl {
                self.typedef_names.insert(
                    c.name.clone(),
                    Type::Class {
                        name: c.name.clone(),
                        is_const: false,
                    },
                );
            }
            // 类模板方法类外定义：如 template<class T> void Box<T>::set(T x) { ... }
            // 已把 body 合并到对应类模板的 method 声明中，避免生成独立函数模板。
            let mut merged = false;
            if let vitro_ast::Templateable::Func(ref f) = template_decl.decl {
                if f.body.is_some() {
                    if let Some((class_name, method_name)) = f.name.split_once("__") {
                        for t in program.templates.iter_mut() {
                            if let vitro_ast::Templateable::Class(ref mut c) = t.decl {
                                if c.name == class_name {
                                    for member in &mut c.members {
                                        if let vitro_ast::ClassMember::Method { name, body, .. } = member {
                                            if name == method_name && body.is_none() {
                                                *body = f.body.clone();
                                                merged = true;
                                                break;
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
            if !merged {
                program.templates.push(template_decl);
            }
        }
    }

    /// 解析 C++ 模式下有名字的 `struct` 声明为 class-like。
    /// 调用时当前位置应在 `struct` 关键字处，与 parse_struct_decl 相同。
    pub(crate) fn parse_cpp_class_like_struct_decl(&mut self, program: &mut ProgramNode) {
        let class_decl = self.parse_class_decl_inner(true);
        self.typedef_names.insert(
            class_decl.name.clone(),
            Type::Class {
                name: class_decl.name.clone(),
                is_const: false,
            },
        );
        program.classes.push(class_decl);
    }
}
