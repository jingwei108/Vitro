//! 构建期注入 git 短哈希 → `VITRO_GIT_HASH`（`vitro_engine_version()` 消费，
//! S5 A4 / 回放纪律 #2 的版本锚定依赖：版本串必须含锚定 commit）。
use std::path::PathBuf;
use std::process::Command;

/// 执行 git 并返回 stdout（失败返回 None）。
fn git(args: &[&str]) -> Option<String> {
    Command::new("git")
        .args(args)
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
}

fn main() {
    // ── 重跑条件（**必须显式声明**）─────────────────────────────────────────
    // 此前依赖"无 rerun-if 指令 = 包内文件变更即重跑"的默认行为，但它漏掉了
    // **提交本身**：一次 `git commit` 不改变任何包内文件，于是 build.rs 不重跑，
    // 嵌入的哈希停留在上一个"被改动源码"的时刻。实测踩坑：HEAD 已是 `622a859`
    // 而 release dll 仍报 `0.1.0 (94c16d2)`，回放 S5 A4b 的版本锚定因此形同虚设
    // （极易在"验证的是陈旧二进制"的情况下拿到假绿）。
    //
    // 声明 rerun-if-changed 会**关闭**默认启发，故包内路径（src / Cargo.toml）须
    // 一并显式列出；`.git/HEAD` 与它指向的 ref 文件负责捕获提交/切分支。
    println!("cargo:rerun-if-changed=src");
    println!("cargo:rerun-if-changed=Cargo.toml");
    let git_dir = git(&["rev-parse", "--absolute-git-dir"]).map(PathBuf::from);
    if let Some(dir) = &git_dir {
        println!("cargo:rerun-if-changed={}", dir.join("HEAD").display());
        // HEAD 指向的 ref（如 refs/heads/master）才是提交后真正变化的文件
        if let Some(ref_name) = git(&["rev-parse", "--symbolic-full-name", "HEAD"]) {
            if !ref_name.is_empty() {
                println!("cargo:rerun-if-changed={}", dir.join(&ref_name).display());
            }
        }
        // 打包引用（git gc / clone 后 ref 可能落在 packed-refs）
        println!("cargo:rerun-if-changed={}", dir.join("packed-refs").display());
    }

    // ── 哈希注入 ────────────────────────────────────────────────────────────
    let hash = git(&["rev-parse", "--short", "HEAD"]).unwrap_or_else(|| "unknown".to_string());
    println!("cargo:rustc-env=VITRO_GIT_HASH={}", hash);
}
