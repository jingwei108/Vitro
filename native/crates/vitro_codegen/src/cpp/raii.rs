use vitro_runtime::opcode::OpCode;
use vitro_shared::SourceLoc;

use super::super::{BytecodeGen, ClassVarEntry};

impl BytecodeGen {
    /// 记录当前 scope 中声明的类类型局部变量，供作用域退出时析构。
    pub(crate) fn record_class_var(&mut self, offset: i32, class_name: &str) {
        if let Some(frame) = self.local_scope_stack.last_mut() {
            frame.class_vars.push(ClassVarEntry {
                offset,
                class_name: class_name.to_string(),
            });
        }
    }

    /// 生成对栈上指定偏移处类对象的析构函数调用。
    pub(crate) fn emit_class_dtor(&mut self, class_name: &str, offset: i32, loc: &SourceLoc) {
        let dtor_name = format!("__dtor__{}", class_name);
        if let Some(&idx) = self.func_index.get(&dtor_name) {
            self.emit(OpCode::GetFrameBase, 0, loc);
            self.emit(OpCode::PushConst, offset, loc);
            self.emit(OpCode::Add, 0, loc);
            self.emit(OpCode::Call, idx, loc);
        }
    }

    /// 生成对栈上指定偏移处类对象的构造函数调用（无参默认构造函数）。
    pub(crate) fn emit_class_default_ctor(&mut self, class_name: &str, offset: i32, loc: &SourceLoc) {
        let ctor_name = format!("__ctor__{}", class_name);
        if let Some(&idx) = self.func_index.get(&ctor_name) {
            self.emit(OpCode::GetFrameBase, 0, loc);
            self.emit(OpCode::PushConst, offset, loc);
            self.emit(OpCode::Add, 0, loc);
            self.emit(OpCode::Call, idx, loc);
        }
    }

    /// 按从内到外的顺序，析构 `start_frame_idx ..=` 当前最外层之间的所有 scope 的类变量。
    ///
    /// `start_frame_idx` 是 `local_scope_stack` 的**帧索引**（0 = 函数最外层 block；
    /// 函数参数不进 scope 栈）。调用方负责按控制流语义给出起点：
    /// - `return`：0（全部析构）
    /// - `continue`：循环体自身 frame（for-init/range-for 临时 frame 仍存活）
    /// - `break`：while/do-while 为循环体 frame；for/range-for 为 for-init frame
    ///   （跳过整个 for 语句，init 对象随循环一起销毁）
    pub(crate) fn emit_dtors_for_scope_exit(&mut self, start_frame_idx: usize, loc: &SourceLoc) {
        let current_depth = self.local_scope_stack.len();
        if current_depth == 0 || current_depth < start_frame_idx {
            return;
        }
        // 先收集所有需要析构的类变量信息，避免 borrow 冲突
        let mut dtors: Vec<(String, i32)> = Vec::new();
        for frame_idx in (start_frame_idx..current_depth).rev() {
            let frame = &self.local_scope_stack[frame_idx];
            for cv in frame.class_vars.iter().rev() {
                dtors.push((cv.class_name.clone(), cv.offset));
            }
        }
        for (class_name, offset) in dtors {
            self.emit_class_dtor(&class_name, offset, loc);
        }
    }
}
