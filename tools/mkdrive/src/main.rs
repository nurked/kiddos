//! `kiddos-mkdrive <content-dir> <out.kdd>`
//! Builds a drive image from a host directory. The app's build script does
//! the same thing to embed the factory image; this is for inspection.

use std::path::Path;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [src, out] = args.as_slice() else {
        eprintln!("usage: kiddos-mkdrive <content-dir> <out.kdd>");
        std::process::exit(2);
    };
    let vfs = match kiddos_vfs::Vfs::from_dir(Path::new(src)) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("mkdrive: {e}");
            std::process::exit(1);
        }
    };
    if let Err(e) = vfs.save(Path::new(out)) {
        eprintln!("mkdrive: {e}");
        std::process::exit(1);
    }
    let mut files = 0;
    let _ = vfs.walk_tree("/", &mut |_, st, _| {
        if st.is_file() {
            files += 1;
        }
    });
    println!(
        "wrote {out}: {} nodes, {} files, {} bytes of content",
        vfs.node_count(),
        files,
        vfs.used_bytes()
    );
}
