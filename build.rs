//! Point the repo-root `compile_commands.json` at the compile database that
//! ocio-sys' bundled CMake build generates, so clangd / clice can index the
//! OpenColorIO C++ sources.

use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=Cargo.toml");
    println!("cargo:rerun-if-changed=Cargo.lock");

    let manifest_dir = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").expect("set by cargo"));
    let Some(db) = newest_compile_database() else {
        return;
    };
    let link = manifest_dir.join("compile_commands.json");
    // Use relative links: it keeps working if the checkout is moved.
    let target = db.strip_prefix(&manifest_dir).unwrap_or(&db);

    if fs::read_link(&link).ok().as_deref() == Some(target) {
        return;
    }
    // Never replace a regular file someone put there.
    if let Ok(meta) = fs::symlink_metadata(&link) {
        if !meta.file_type().is_symlink() {
            return;
        }
        let _ = fs::remove_file(&link);
    }
    if let Err(error) = create_symlink(target, &link) {
        println!("cargo:warning=could not link compile_commands.json: {error}");
    }
}

/// ocio-sys builds OCIO under `target/<profile>/build/ocio-sys-<hash>/out/cmake-*/build`.
fn newest_compile_database() -> Option<PathBuf> {
    // OUT_DIR is `<target>/<profile>/build/color-grade-<hash>/out`.
    let out_dir = PathBuf::from(env::var_os("OUT_DIR")?);
    let build_root = out_dir.ancestors().nth(2)?;

    let mut newest: Option<(SystemTime, PathBuf)> = None;
    for entry in fs::read_dir(build_root).ok()?.flatten() {
        if !entry.file_name().to_string_lossy().starts_with("ocio-sys-") {
            continue;
        }
        let Ok(files) = fs::read_dir(entry.path().join("out")) else {
            continue;
        };
        for file in files.flatten() {
            if !file.file_name().to_string_lossy().starts_with("cmake-") {
                continue;
            }
            let db = file.path().join("build/compile_commands.json");
            let Ok(modified) = fs::metadata(&db).and_then(|meta| meta.modified()) else {
                continue;
            };
            if newest.as_ref().is_none_or(|(time, _)| modified > *time) {
                newest = Some((modified, db));
            }
        }
    }
    newest.map(|(_, db)| db)
}

#[cfg(unix)]
fn create_symlink(target: &Path, link: &Path) -> std::io::Result<()> {
    std::os::unix::fs::symlink(target, link)
}

#[cfg(windows)]
fn create_symlink(target: &Path, link: &Path) -> std::io::Result<()> {
    std::os::windows::fs::symlink_file(target, link)
}
