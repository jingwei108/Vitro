use vitro_ast::Type;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Symbol {
    pub name: String,
    pub addr: u32,
    pub is_local: bool,
    pub ty: Type,
    pub scope_depth: i32,
    pub func_name: String,
    /// 声明处的源码行号（0 = 未知/不适用）。
    ///
    /// 用途：`VitroVM::get_variable_snapshot` 据此判定"该符号在当前执行点是否已进入
    /// 作用域"。这正是同名变量（两个 `for` 各声明一个 `i`）无法区分活跃者的根因 ——
    /// 只按符号表顺序或分配地址都无法排除"尚未执行到声明处"的变量，而 `scope_depth`
    /// 在所有构造点都是常量（1 = 局部 / 0 = 静态），不表达嵌套深度。
    pub decl_line: i32,
}
