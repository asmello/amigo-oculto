//! Build script: derives a short content hash of `static/` so asset URLs can carry
//! `?v=<hash>` and be cached immutably.

use std::collections::hash_map::DefaultHasher;
use std::fs;
use std::hash::{Hash, Hasher};
use std::path::Path;

fn hash_dir(dir: &Path, hasher: &mut DefaultHasher) {
    let mut entries: Vec<_> = fs::read_dir(dir)
        .unwrap_or_else(|e| panic!("reading {}: {e}", dir.display()))
        .map(|e| e.expect("dir entry").path())
        .collect();
    entries.sort();
    for path in entries {
        println!("cargo:rerun-if-changed={}", path.display());
        if path.is_dir() {
            hash_dir(&path, hasher);
        } else {
            path.to_string_lossy().hash(hasher);
            fs::read(&path)
                .unwrap_or_else(|e| panic!("reading {}: {e}", path.display()))
                .hash(hasher);
        }
    }
}

fn main() {
    println!("cargo:rerun-if-changed=static");
    let mut hasher = DefaultHasher::new();
    hash_dir(Path::new("static"), &mut hasher);
    println!(
        "cargo:rustc-env=ASSET_VERSION={:08x}",
        hasher.finish() as u32
    );
}
