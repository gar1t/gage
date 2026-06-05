use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::Command;

fn main() {
    write_version();
}

// Compute the version string baked into `gage --version`. Official release
// builds pin a real semver via GAGE_VERSION; source builds fall back to the
// current git commit (mirroring `rune --version`); a git-less build (e.g. a
// published crate) falls back to the crate version.
fn write_version() {
    let out_dir = PathBuf::from(env::var_os("OUT_DIR").expect("OUT_DIR set by cargo"));

    let version = env::var("GAGE_VERSION")
        .ok()
        .or_else(git_version)
        .unwrap_or_else(|| env::var("CARGO_PKG_VERSION").expect("CARGO_PKG_VERSION set by cargo"));

    fs::write(out_dir.join("version.txt"), version).expect("writing version.txt");

    println!("cargo::rerun-if-env-changed=GAGE_VERSION");
    println!("cargo::rerun-if-changed=../.git/HEAD");
}

fn git_version() -> Option<String> {
    let output = Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let rev = String::from_utf8(output.stdout).ok()?;
    let rev = rev.trim();
    (!rev.is_empty()).then(|| format!("git-{rev}"))
}
