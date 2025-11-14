use std::env;
use std::fs;
use std::path::Path;
use std::process::Command;

fn main() {
    let out_dir = env::var("OUT_DIR").unwrap();
    let dest = Path::new(&out_dir).join("build_info.rs");
    let now = chrono::Utc::now().to_rfc3339();

    let git_hash = Command::new("git")
        .args(["rev-parse", "HEAD"])
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|| "unknown".into());

    fs::write(
        dest,
        format!("pub const VERSION: &str = \"{} {}\";", git_hash, now),
    )
    .unwrap();
}
