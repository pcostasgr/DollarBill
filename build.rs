use sha2::{Digest, Sha256};
use std::{fs, path::{Path, PathBuf}, process::Command};

fn files(path: &Path, out: &mut Vec<PathBuf>) {
    if path.is_dir() {
        for entry in fs::read_dir(path).expect("read source directory") {
            files(&entry.expect("source entry").path(), out);
        }
    } else if path.is_file() { out.push(path.to_path_buf()); }
}

fn command(program: &str, args: &[&str]) -> String {
    Command::new(program).args(args).output().ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_else(|| "unavailable".into())
}

fn main() {
    let mut paths = Vec::new();
    for root in ["src", "tests", "Cargo.toml", "Cargo.lock", "build.rs"] {
        println!("cargo:rerun-if-changed={root}");
        files(Path::new(root), &mut paths);
    }
    for name in ["HEAD", "index", "refs"] {
        let path = command("git", &["rev-parse", "--git-path", name]);
        if path != "unavailable" { println!("cargo:rerun-if-changed={path}"); }
    }
    paths.sort();
    let mut hash = Sha256::new();
    for path in paths {
        let name = path.to_string_lossy().replace('\\', "/");
        // Normalize text line endings so a Windows checkout has the same ID.
        let bytes = fs::read(&path).expect("read source file");
        let bytes = match String::from_utf8(bytes.clone()) {
            Ok(text) => text.replace("\r\n", "\n").into_bytes(),
            Err(_) => bytes,
        };
        hash.update((name.len() as u64).to_le_bytes());
        hash.update(name.as_bytes());
        hash.update((bytes.len() as u64).to_le_bytes());
        hash.update(bytes);
    }
    println!("cargo:rustc-env=DOLLARBILL_SOURCE_SHA256={:x}", hash.finalize());
    println!("cargo:rustc-env=DOLLARBILL_GIT_COMMIT={}", command("git", &["rev-parse", "HEAD"]));
    println!("cargo:rustc-env=DOLLARBILL_RUSTC={}", command(&std::env::var("RUSTC").unwrap_or_else(|_| "rustc".into()), &["--version"]));
    println!("cargo:rustc-env=DOLLARBILL_TARGET={}", std::env::var("TARGET").unwrap());
}
