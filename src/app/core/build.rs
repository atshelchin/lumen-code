//! Records the version for the screens: the last commit that changed anything outside dist/
//! (the same rule as the web build, src/tools/build.js), its date, and whether the sources had
//! uncommitted changes. Outside a git checkout the version is "unknown".
use std::process::Command;

fn git(args: &[&str]) -> Option<String> {
    let out = Command::new("git").args(args).current_dir(env!("CARGO_MANIFEST_DIR")).output().ok()?;
    out.status.success().then(|| String::from_utf8_lossy(&out.stdout).trim().to_string())
}

fn main() {
    let root = git(&["rev-parse", "--show-toplevel"]);
    let in_root = |args: &[&str]| -> Option<String> {
        let mut all = vec!["-C", root.as_deref()?];
        all.extend_from_slice(args);
        git(&all)
    };
    // a shallow clone (CI) may not reach the last source commit: then HEAD
    let full = in_root(&["log", "-1", "--format=%H", "--", ".", ":!dist"]).filter(|s| !s.is_empty()).or_else(|| in_root(&["rev-parse", "HEAD"]));
    let date = in_root(&["log", "-1", "--format=%cs", "--", ".", ":!dist"]).filter(|s| !s.is_empty()).or_else(|| in_root(&["log", "-1", "--format=%cs"]));
    let dirty = in_root(&["status", "--porcelain", "--", ".", ":!dist"]).is_some_and(|s| !s.is_empty());
    let commit = full.as_deref().map_or("unknown".to_string(), |f| f.chars().take(7).collect());
    println!("cargo:rustc-env=LUMEN_COMMIT={commit}");
    println!("cargo:rustc-env=LUMEN_COMMIT_FULL={}", full.unwrap_or_default());
    println!("cargo:rustc-env=LUMEN_COMMIT_DATE={}", date.unwrap_or_default());
    println!("cargo:rustc-env=LUMEN_DIRTY={}", if dirty { "1" } else { "" });
    // build again when the commit or the index changes
    if let Some(dir) = in_root(&["rev-parse", "--absolute-git-dir"]) {
        println!("cargo:rerun-if-changed={dir}/HEAD");
        println!("cargo:rerun-if-changed={dir}/index");
        if let Some(head) = in_root(&["symbolic-ref", "-q", "HEAD"]) {
            println!("cargo:rerun-if-changed={dir}/{head}");
        }
    }
    println!("cargo:rerun-if-changed=build.rs");
}
