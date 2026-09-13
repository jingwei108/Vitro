#[derive(Debug, Clone, Copy, Default, serde::Serialize, serde::Deserialize)]
pub struct SourceLoc {
    pub line: i32,
    pub column: i32,
    /// 源码文件标识。0 表示用户主文件（默认），非 0 用于区分标准库 /
    /// 预编译字节码 / 头文件等外部源码位置，避免外部行号污染覆盖率统计。
    #[serde(default)]
    pub file_id: i32,
}

impl SourceLoc {
    /// 创建用户主文件（file_id=0）的源码位置。
    pub fn new(line: i32, column: i32) -> Self {
        Self { line, column, file_id: 0 }
    }

    /// 创建指定文件标识的源码位置。
    pub fn with_file(line: i32, column: i32, file_id: i32) -> Self {
        Self { line, column, file_id }
    }
}
