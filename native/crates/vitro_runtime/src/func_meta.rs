use vitro_ast::Type;

/// 局部缓冲区描述（V-P1-6 栈缓冲区溢出检测用）：
/// 编译期登记的栈上数组（如 `char buf[4]`），offset 相对帧 locals_base。
#[derive(Debug, Clone, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub struct LocalBuffer {
    /// 相对 locals_base 的字节偏移。
    pub offset: i32,
    /// 缓冲区总字节数。
    pub size: i32,
    /// 变量名（教学诊断用）。
    pub name: String,
}

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct FuncMeta {
    pub ip: usize,
    /// 参数总 word 数（以 4-byte words 计），供 Call 指令弹栈使用。
    pub arg_count: i32,
    /// 参数个数（供 call_user_function 使用，与总 word 数不同）。
    pub param_count: i32,
    pub local_count: i32,
    pub param_sizes: Vec<i32>,
    /// 函数返回类型（codegen 使用；session/vm 中保持默认 Void 即可）。
    #[serde(default)]
    pub return_type: Type,
    /// 是否为变参函数。
    #[serde(default)]
    pub is_variadic: bool,
    /// 栈上局部数组缓冲区表（V-P1-6）：strcpy/strcat/scanf("%s") 等宿主函数
    /// 据此对栈缓冲区做容量校验（此前只查堆 region，栈上 `char buf[4]`
    /// 被静默覆写）。
    #[serde(default)]
    pub local_buffers: Vec<LocalBuffer>,
}
