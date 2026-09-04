//! Bake content/factory-drive into a SQLite image embedded in the binary.

use std::path::PathBuf;

fn main() {
    let out = PathBuf::from(std::env::var("OUT_DIR").expect("OUT_DIR"));
    let src =
        PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").expect("manifest dir")).join("../content/factory-drive");
    println!("cargo:rerun-if-changed={}", src.display());
    let vfs = kiddos_vfs::Vfs::from_dir(&src).expect("build factory drive from content/factory-drive");
    vfs.save(&out.join("factory.kdd")).expect("write factory.kdd");
}
