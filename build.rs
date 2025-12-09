use std::env;
use std::fs;
use std::path::Path;
use std::process::Command;

use chrono::SecondsFormat;

fn main() {
    let out_dir = env::var("OUT_DIR").unwrap();
    let dest = Path::new(&out_dir).join("build_info.rs");
    let now = chrono::Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true);
    let version = env!("CARGO_PKG_VERSION");
    let git_hash = Command::new("git")
        .args(["rev-parse", "HEAD"])
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim()[0..7].to_string())
        .unwrap_or_else(|| "unknown".into());

    fs::write(
        dest,
        format!(
            "pub const VERSION: &str = \"{}-{}-{}\";",
            version, git_hash, now
        ),
    )
    .unwrap();
}
