use crate::expr::{is_lvalue_expr, ExprGen};
use vitro_ast::*;
use vitro_runtime::opcode::OpCode;

use super::super::BytecodeGen;

impl BytecodeGen {
    /// 尝试处理 C++ class 类型局部变量初始化。
    /// 返回 true 表示已生成完整初始化代码，调用方应结束当前变量初始化。
    /// 返回 false 表示不是 C++ 特殊情况，调用方应继续走通用 struct/class 拷贝路径。
    pub(crate) fn try_gen_cpp_class_init(
        &mut self,
        vty: &Type,
        e: &mut Expr,
        local_offset: i32,
        loc: &SourceLoc,
    ) -> bool {
        if !vty.is_class() {
            return false;
        }

        // C++ 构造函数初始化语法：Type name(args);
        if let Expr::Call {
            name: ctor_name,
            args: ctor_args,
            ..
        } = e
        {
            if ctor_name.starts_with("__ctor__") {
                if let Type::Class { name: class_name, .. } = vty {
                    // VM do_call pops args in parameter-declaration order.
                    // Args already include this as the first parameter.
                    let mut temp_cleanups: Vec<(i32, String)> = Vec::new();
                    for arg in ctor_args.iter_mut().rev() {
                        // RValueRef arguments (e.g. std::move) must be passed
                        // as the address of the source object.
                        if arg.ty().is_rvalue_ref() {
                            self.gen_addr(arg, loc);
                        } else if let Expr::Unary { op: UnaryOp::Addr, operand, .. } = arg {
                            let (temp_class, has_dtor) = {
                                let operand_ty = operand.ty();
                                if (operand_ty.is_class() || operand_ty.is_struct())
                                    && matches!(operand.as_ref(), Expr::Call { .. } | Expr::CallPtr { .. })
                                {
                                    let name = operand_ty.name().to_string();
                                    let dtor = format!("__dtor__{}", name);
                                    (name, self.func_index.contains_key(&dtor))
                                } else {
                                    (String::new(), false)
                                }
                            };
                            if has_dtor {
                                let cleanup_slot = self.next_local_offset;
                                self.next_local_offset += 4;
                                self.gen_expr(operand);
                                self.emit(OpCode::StoreLocal, cleanup_slot, loc);
                                temp_cleanups.push((cleanup_slot, temp_class));
                                self.emit(OpCode::LoadLocal, cleanup_slot, loc);
                            } else {
                                self.gen_expr(arg);
                            }
                        } else {
                            self.gen_expr(arg);
                        }
                    }
                    if let Some(&idx) = self.func_index.get(ctor_name) {
                        self.emit(OpCode::Call, idx, loc);
                    }
                    // 析构为按 const 引用传递而生成的临时类对象。
                    for (slot, temp_class) in temp_cleanups.iter().rev() {
                        let dtor_name = format!("__dtor__{}", temp_class);
                        if let Some(&idx) = self.func_index.get(&dtor_name) {
                            // slot 中保存的是临时对象本身的地址，直接作为 this 传入析构函数。
                            self.emit(OpCode::LoadLocal, *slot, loc);
                            self.emit(OpCode::Call, idx, loc);
                        }
                    }
                    self.record_class_var(local_offset, class_name);
                    return true;
                }
            }
        }

        // Lambda 闭包：gen_lambda 在栈上推闭包对象地址，直接保存地址（不逐字段拷贝）
        if matches!(e, Expr::Lambda { .. }) {
            self.gen_expr(e);
            self.emit(OpCode::StoreLocal, local_offset, loc);
            return true;
        }

        // C++ implicit move ctor: call __ctor__{Class}__move when
        // initializing from an rvalue (std::move or RValueRef).
        if e.ty().is_rvalue_ref() || matches!(e, Expr::Move { .. }) {
            if let Type::Class { name: class_name, .. } = vty {
                let move_ctor_name = format!("__ctor__{}__move", class_name);
                if self.func_index.contains_key(&move_ctor_name) {
                    // VM do_call pops args in parameter-declaration order.
                    // We must push them right-to-left so the first pop() gets 'this'.
                    // other = source address (pushed first)
                    self.gen_addr(e, loc);
                    // this = &local_var (pushed second, popped first)
                    self.emit(OpCode::GetFrameBase, 0, loc);
                    self.emit(OpCode::PushConst, local_offset, loc);
                    self.emit(OpCode::Add, 0, loc);
                    if let Some(&idx) = self.func_index.get(&move_ctor_name) {
                        self.emit(OpCode::Call, idx, loc);
                    }
                    self.record_class_var(local_offset, class_name);
                } else {
                    self.gen_struct_copy_to_local(local_offset, e, loc);
                }
                return true;
            }
        }

        false
    }

    /// C++ reference initialization: store address of initializer.
    pub(crate) fn gen_cpp_reference_init(&mut self, _vty: &Type, e: &mut Expr, local_offset: i32, loc: &SourceLoc) {
        if e.ty().is_reference() || e.ty().is_rvalue_ref() {
            // The initializer itself is a reference expression; gen_expr
            // already leaves the target address on the stack.
            self.gen_expr(e);
        } else if is_lvalue_expr(e) {
            self.gen_addr(e, loc);
        } else {
            // Rvalue: extend lifetime by storing into a temporary local,
            // then bind the reference to that temporary's address.
            let temp_offset = self.next_local_offset;
            let temp_sz = (self.type_size(e.ty()) + 3) & !3;
            self.next_local_offset += temp_sz;
            self.gen_expr(e);
            self.emit(OpCode::StoreLocal, temp_offset, loc);
            self.emit(OpCode::GetFrameBase, 0, loc);
            self.emit(OpCode::PushConst, temp_offset, loc);
            self.emit(OpCode::Add, 0, loc);
        }
        self.emit(OpCode::StoreLocal, local_offset, loc);
    }

    /// 对未初始化的 C++ class 类型局部变量调用默认构造函数并记录 RAII。
    /// 返回 true 表示已处理。
    pub(crate) fn try_gen_cpp_class_default_ctor(&mut self, vty: &Type, local_offset: i32, loc: &SourceLoc) -> bool {
        if !vty.is_class() {
            return false;
        }
        if let Type::Class { name: class_name, .. } = vty {
            self.record_class_var(local_offset, class_name);
            self.emit_class_default_ctor(class_name, local_offset, loc);
        }
        true
    }
}
