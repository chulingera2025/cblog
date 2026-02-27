use std::process::Command;

fn main() {
    // Git commit hash（短）
    let commit = Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_else(|| "unknown".to_string());

    // Git commit hash（完整）
    let commit_full = Command::new("git")
        .args(["rev-parse", "HEAD"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_else(|| "unknown".to_string());

    // 构建时间（UTC）
    let build_time = chrono_utc_now();

    // 目标三元组
    let target = std::env::var("TARGET").unwrap_or_else(|_| "unknown".to_string());

    // 构建 profile
    let profile = std::env::var("PROFILE").unwrap_or_else(|_| "unknown".to_string());

    println!("cargo:rustc-env=CBLOG_GIT_COMMIT={commit}");
    println!("cargo:rustc-env=CBLOG_GIT_COMMIT_FULL={commit_full}");
    println!("cargo:rustc-env=CBLOG_BUILD_TIME={build_time}");
    println!("cargo:rustc-env=CBLOG_BUILD_TARGET={target}");
    println!("cargo:rustc-env=CBLOG_BUILD_PROFILE={profile}");

    // 仅在 git HEAD 变化时重新运行
    println!("cargo:rerun-if-changed=.git/HEAD");
    println!("cargo:rerun-if-changed=.git/refs/");
}

fn chrono_utc_now() -> String {
    // 不引入 chrono 依赖，直接用 date 命令或 fallback
    Command::new("date")
        .args(["-u", "+%Y-%m-%dT%H:%M:%SZ"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_else(|| "unknown".to_string())
}
