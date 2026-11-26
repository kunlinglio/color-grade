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
    let link = manifest_dir.join("compile_commands.json");
    let db = match newest_compile_database() {
        Ok(Some(db)) => db,
        Ok(None) => {
            println!(
                "cargo:warning=no bundled OCIO compile_commands.json found; the repo-root link was not updated. Ensure you have update the submodules and the compilation of ocio-sys succeeded"
            );
            return;
        }
        Err(error) => {
            println!("cargo:warning=failed to find the OCIO compile database: {error}");
            return;
        }
    };

    // Use relative links: it keeps working if the checkout is moved.
    let target = db.strip_prefix(&manifest_dir).unwrap_or(&db);

    if fs::read_link(&link).ok().as_deref() == Some(target) {
        return;
    }
    // Never replace a regular file someone put there.
    match fs::symlink_metadata(&link) {
        Ok(meta) if !meta.file_type().is_symlink() => {
            println!(
                "cargo:warning=not replacing {}: it already exists and is not a symlink",
                link.display()
            );
            return;
        }
        Ok(_) => {
            if let Err(error) = fs::remove_file(&link) {
                println!(
                    "cargo:warning=could not remove stale link {}: {error}",
                    link.display()
                );
                return;
            }
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => {
            println!(
                "cargo:warning=could not inspect {}: {error}",
                link.display()
            );
            return;
        }
    }
    if let Err(error) = create_symlink(target, &link) {
        println!("cargo:warning=could not link compile_commands.json: {error}");
    }
}

/// ocio-sys builds OCIO under `target/<profile>/build/ocio-sys-<hash>/out/cmake-*/build`.
fn newest_compile_database() -> std::io::Result<Option<PathBuf>> {
    // OUT_DIR is `<target>/<profile>/build/color-grade-<hash>/out`.
    let out_dir = PathBuf::from(
        env::var_os("OUT_DIR").ok_or_else(|| std::io::Error::other("OUT_DIR is not set"))?,
    );
    let build_root = out_dir
        .ancestors()
        .nth(2)
        .ok_or_else(|| std::io::Error::other("OUT_DIR has no Cargo build directory"))?;

    let mut newest: Option<(SystemTime, PathBuf)> = None;
    for entry in fs::read_dir(build_root)? {
        let entry = entry?;
        if !entry.file_name().to_string_lossy().starts_with("ocio-sys-") {
            continue;
        }
        let files = match fs::read_dir(entry.path().join("out")) {
            Ok(files) => files,
            // Cargo also creates ocio-sys directories for the build-script
            // executable; those have no out directory.
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(error) => return Err(error),
        };
        for file in files {
            let file = file?;
            if !file.file_name().to_string_lossy().starts_with("cmake-") {
                continue;
            }
            let db = file.path().join("build/compile_commands.json");
            let metadata = match fs::metadata(&db) {
                Ok(metadata) => metadata,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
                Err(error) => return Err(error),
            };
            if !metadata.is_file() {
                continue;
            }
            let modified = metadata.modified()?;
            if newest.as_ref().is_none_or(|(time, _)| modified > *time) {
                newest = Some((modified, db));
            }
        }
    }
    Ok(newest.map(|(_, db)| db))
}

#[cfg(unix)]
fn create_symlink(target: &Path, link: &Path) -> std::io::Result<()> {
    std::os::unix::fs::symlink(target, link)
}

#[cfg(windows)]
fn create_symlink(target: &Path, link: &Path) -> std::io::Result<()> {
    std::os::windows::fs::symlink_file(target, link)
}
