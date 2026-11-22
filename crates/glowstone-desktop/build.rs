use std::process::Command;

fn main() {
    println!("cargo:rerun-if-env-changed=GITHUB_SHA");
    let sha = std::env::var("GITHUB_SHA").ok().filter(|value| !value.trim().is_empty())
        .or_else(|| {
            Command::new("git")
                .args(["rev-parse", "HEAD"])
                .output()
                .ok()
                .filter(|output| output.status.success())
                .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_owned())
        })
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "unknown".to_owned());
    println!("cargo:rustc-env=GLOWSTONE_BUILD_SHA={sha}");
}
