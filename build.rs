//! Add commit id & dirty flag to `CARGO_PKG_VERSION`
use std::process::Command;

fn head_path() -> String {
    let output = Command::new("git")
        .args(&["rev-parse", "--git-dir"])
        .output()
        .expect("Got $GIT_DIR");
    let git_dir = String::from_utf8_lossy(&output.stdout);
    format!("{}/HEAD", git_dir)
}

fn commits_since_release() -> String {
    let id = {
        let out = Command::new("git")
            .args(&["rev-list", "-1", "HEAD", "--", "CHANGELOG.md"])
            .output()
            .expect("A committed CHANGELOG.md");
        String::from_utf8_lossy(&out.stdout).to_string()
    };
    let range = format!("{}..HEAD", id.trim());
    let out = Command::new("git")
        .args(&["rev-list", "--count", &range, "--", "."])
        .output()
        .expect("git rev-list successful");
    String::from_utf8_lossy(&out.stdout)
        .clone()
        .trim()
        .to_owned()
}

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=Cargo.toml");
    println!("cargo:rerun-if-changed={}", head_path());

    if let Ok(status) = Command::new("git")
        .args(&["diff-index", "--quiet", "HEAD", "--"])
        .status()
    {
        let commits_since_release = commits_since_release();
        let changed_since_release = commits_since_release != "0";
        let changed_files = status.success();

        let cargo_version = env!("CARGO_PKG_VERSION");
        let version = match (changed_since_release, changed_files) {
            (false, true) => cargo_version.to_owned(),
            (false, false) => format!("{}+dirty", cargo_version),
            (true, clean) => {
                let id_out = Command::new("git")
                    .args(&["rev-parse", "--short", "HEAD"])
                    .output()
                    .expect("Executed git-rev-parse(1)");
                let id = String::from_utf8_lossy(&id_out.stdout).to_string();
                if clean {
                    format!("{}+{}.{}", cargo_version, commits_since_release, id.trim())
                } else {
                    format!(
                        "{}+{}.{}.dirty",
                        cargo_version,
                        commits_since_release,
                        id.trim()
                    )
                }
            }
        };
        println!("cargo:rustc-env=CARGO_PKG_VERSION={}", version);
    }
}
