//! A cartridge is a folder under `/games` with a `cart.toml`. Adding one
//! never needs a recompile: the entry is a script (or later `.bas` /
//! `.wasm`) that the kernel runs by its shebang.

use kiddos_kernel::{CapSet, Console, Proc, Spawn};
use serde::Deserialize;

pub const GAMES_DIR: &str = "/games";

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct Manifest {
    pub name: String,
    pub title: String,
    pub version: String,
    pub author: String,
    /// Path relative to the cart folder. Run through the kernel, so any
    /// shebang works.
    pub entry: String,
    pub description: String,
    pub lang: Vec<String>,
    pub difficulty: u8,
    /// Capabilities requested: "speak", "sound".
    pub caps: Vec<String>,
    pub min_kiddos: String,
    /// Commands granted on completion (used from Phase 4 on).
    pub unlocks: Vec<String>,
    /// Folders (`~` allowed) where the kid is "inside the game": the tutor
    /// keeps quiet there.
    pub world: Vec<String>,
    /// Memory a compiled entry may use, in MB (0 = the sandbox default of
    /// 16; Doom asks for 64). Capped by the sandbox at 256.
    pub memory_mb: u32,
}

impl Default for Manifest {
    fn default() -> Self {
        Manifest {
            name: String::new(),
            title: String::new(),
            version: "0.1.0".into(),
            author: String::new(),
            entry: "main.sh".into(),
            description: String::new(),
            lang: vec!["en".into()],
            difficulty: 1,
            caps: vec!["speak".into(), "sound".into()],
            min_kiddos: "0.2.0".into(),
            memory_mb: 0,
            unlocks: Vec::new(),
            world: Vec::new(),
        }
    }
}

/// Parse a manifest from its text (no process needed).
pub fn parse_manifest(name: &str, text: &str) -> Result<Manifest, String> {
    let mut m: Manifest = toml::from_str(text).map_err(|e| format!("{name}'s cart.toml is broken: {e}"))?;
    if m.name.is_empty() {
        m.name = name.to_string();
    }
    if m.title.is_empty() {
        m.title = name.to_string();
    }
    Ok(m)
}

/// Every world folder of every installed cartridge, as absolute paths.
pub fn all_worlds(vfs: &kiddos_kernel::Vfs, home: &str) -> Vec<String> {
    let root = kiddos_kernel::Actor::root();
    let mut out = Vec::new();
    for e in vfs
        .readdir(GAMES_DIR, &root)
        .unwrap_or_default()
        .into_iter()
        .filter(|e| e.is_dir())
    {
        let Ok(text) = vfs.read_string(&format!("{GAMES_DIR}/{}/cart.toml", e.name), &root) else {
            continue;
        };
        if let Ok(m) = parse_manifest(&e.name, &text) {
            for w in m.world {
                out.push(kiddos_vfs::normalize("/", &kiddos_vfs::path::expand_tilde(&w, home)));
            }
        }
    }
    out
}

pub fn dir_of(name: &str) -> String {
    format!("{GAMES_DIR}/{name}")
}

/// Read and validate `/games/<name>/cart.toml`.
pub fn load(p: &Proc, name: &str) -> Result<Manifest, String> {
    if name.is_empty() || name.contains('/') || name.starts_with('.') {
        return Err(format!("{name} is not a game name."));
    }
    let path = format!("{}/cart.toml", dir_of(name));
    let text = p
        .fs()
        .read_string(&path)
        .map_err(|_| format!("I don't have a game called {name}."))?;
    parse_manifest(name, &text)
}

/// Every installed cartridge, sorted by name.
pub fn list(p: &Proc) -> Vec<Manifest> {
    let mut out: Vec<Manifest> = p
        .fs()
        .readdir(GAMES_DIR)
        .unwrap_or_default()
        .into_iter()
        .filter(|e| e.is_dir())
        .filter_map(|e| load(p, &e.name).ok())
        .collect();
    out.sort_by(|a, b| a.name.cmp(&b.name));
    out
}

/// Run a cartridge and wait for it. The entry gets `CART` in its
/// environment and only the capabilities the manifest asked for.
pub fn launch(p: &Proc, name: &str, args: &[String]) -> Result<i32, String> {
    let m = load(p, name)?;
    let dir = dir_of(name);
    let entry = format!("{dir}/{}", m.entry);
    // games written in Rust ship a folder (docs, levels) and name a command
    let entry = if p.fs().exists(&entry) {
        entry
    } else if !m.entry.contains('/') && p.kernel().command(&m.entry).is_some() {
        m.entry.clone()
    } else {
        return Err(format!("{name} has no {} to run.", m.entry));
    };
    let mut argv = vec![entry];
    argv.extend(args.iter().cloned());
    let mut s = Spawn::child_of(p, argv);
    s.env.insert("CART".into(), dir);
    s.env.insert("GAME".into(), name.to_string());
    if m.memory_mb > 0 {
        // read by the WASM sandbox (kiddos_wasm::MEMORY_ENV)
        s.env.insert("KIDDOS_MEMORY_MB".into(), m.memory_mb.to_string());
    }
    s.caps = CapSet {
        speak: m.caps.iter().any(|c| c == "speak"),
        sound: m.caps.iter().any(|c| c == "sound"),
        jail: None,
    };
    p.kernel().log(&format!("play {name}"));
    let child = p.spawn(s).map_err(|e| e.to_string())?;
    let status = child.wait();
    p.set_color(kiddos_console::colors::DEFAULT_FG, kiddos_console::colors::DEFAULT_BG);
    p.cursor_show(true);
    Ok(status)
}

#[cfg(test)]
mod tests {
    use super::Manifest;

    #[test]
    fn manifest_defaults() {
        let m: Manifest = toml::from_str("name = \"snake\"\ntitle = \"Snake\"\n").unwrap();
        assert_eq!(m.entry, "main.sh");
        assert_eq!(m.caps, vec!["speak", "sound"]);
        assert_eq!(m.title, "Snake");
    }
}

// ---- .kdc files ---------------------------------------------------------
//
// A `.kdc` is a plain zip of the cartridge folder. Parents can open it with
// anything. It travels through the host's cartridge folder, the one place
// files cross the wall between the fake machine and the real one.

pub mod kdc {
    use kiddos_kernel::{Actor, Vfs};
    use std::io::{Cursor, Read, Write};

    pub struct Entry {
        /// Path relative to the cartridge folder, e.g. `man/snake.md`.
        pub path: String,
        pub data: Vec<u8>,
        pub exec: bool,
    }

    /// Zip the folder at `dir` (absolute VFS path).
    pub fn pack(vfs: &Vfs, dir: &str) -> Result<Vec<u8>, String> {
        let root = Actor::root();
        if !vfs.is_dir(dir) {
            return Err(format!("{dir} is not a folder"));
        }
        let mut files: Vec<(String, Vec<u8>, bool)> = Vec::new();
        let base = dir.trim_end_matches('/');
        let mut err = None;
        let _ = vfs.walk_tree(dir, &mut |path, st, _| {
            if st.is_file() {
                let rel = path.strip_prefix(&format!("{base}/")).unwrap_or(path).to_string();
                match vfs.read(path, &root) {
                    Ok(data) => files.push((rel, data, st.mode & 0o111 != 0)),
                    Err(e) => err = Some(e.to_string()),
                }
            }
        });
        if let Some(e) = err {
            return Err(e);
        }
        if !files.iter().any(|(p, _, _)| p == "cart.toml") {
            return Err(format!("{dir} has no cart.toml, so it is not a cartridge"));
        }
        let mut zw = zip::ZipWriter::new(Cursor::new(Vec::new()));
        for (rel, data, exec) in files {
            let opts = zip::write::SimpleFileOptions::default()
                .compression_method(zip::CompressionMethod::Deflated)
                .unix_permissions(if exec { 0o755 } else { 0o644 });
            zw.start_file(rel, opts).map_err(|e| e.to_string())?;
            zw.write_all(&data).map_err(|e| e.to_string())?;
        }
        Ok(zw.finish().map_err(|e| e.to_string())?.into_inner())
    }

    /// Read a `.kdc`. Accepts entries at the root or under one top-level
    /// folder (the way most zip tools produce them).
    pub fn unpack(bytes: &[u8]) -> Result<Vec<Entry>, String> {
        let mut za =
            zip::ZipArchive::new(Cursor::new(bytes)).map_err(|_| "this is not a .kdc (zip) file".to_string())?;
        let mut entries = Vec::new();
        for i in 0..za.len() {
            let mut f = za.by_index(i).map_err(|e| e.to_string())?;
            if f.is_dir() {
                continue;
            }
            let name = f.name().replace('\\', "/");
            if name.contains("..") || name.starts_with('/') {
                return Err(format!("{name}: unsafe path in cartridge"));
            }
            let mut data = Vec::new();
            f.read_to_end(&mut data).map_err(|e| e.to_string())?;
            let exec = f.unix_mode().map(|m| m & 0o111 != 0).unwrap_or(false) || data.starts_with(b"#!");
            entries.push(Entry { path: name, data, exec });
        }
        if entries.is_empty() {
            return Err("the cartridge is empty".into());
        }
        // strip a single common top-level folder
        if !entries.iter().any(|e| e.path == "cart.toml") {
            let first = entries[0].path.split('/').next().unwrap_or("").to_string();
            if !first.is_empty() && entries.iter().all(|e| e.path.starts_with(&format!("{first}/"))) {
                for e in &mut entries {
                    e.path = e.path[first.len() + 1..].to_string();
                }
            }
        }
        if !entries.iter().any(|e| e.path == "cart.toml") {
            return Err("no cart.toml inside; every cartridge needs one".into());
        }
        Ok(entries)
    }
}

// ---- commands -----------------------------------------------------------

use kiddos_kernel::{CmdResult, Command, Kernel, Topic};

pub fn register(k: &Kernel) {
    register_packs(k);
    k.register(Command::new(
        "newgame",
        cmd_newgame,
        "start your own game: newgame rocket",
        Topic::Programs,
    ));
    for c in [
        Command::new(
            "carts",
            cmd_carts,
            "cartridge files in the parent's folder, and what is installed",
            Topic::Parent,
        )
        .parent(),
        Command::new(
            "install",
            cmd_install,
            "install a .kdc cartridge: install rocket",
            Topic::Parent,
        )
        .parent(),
        Command::new("uninstall", cmd_uninstall, "remove an installed game", Topic::Parent).parent(),
        Command::new(
            "share",
            cmd_share,
            "pack a game folder into a .kdc to give away: share /home/kid/rocket",
            Topic::Parent,
        )
        .parent(),
    ] {
        k.register(c);
    }
}

fn valid_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 24
        && name
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
        && name.chars().next().map(|c| c.is_ascii_lowercase()).unwrap_or(false)
}

/// `newgame rocket`: a folder with a cart.toml, a BASIC program, a README
/// and a man page, ready to `run`, and ready for a parent to `export`.
fn cmd_newgame(p: &Proc, args: &[String]) -> CmdResult {
    let Some(name) = args.first() else {
        p.println(&p.t(
            "usage",
            &[("usage", "newgame <name>   (lowercase letters, like: newgame rocket)")],
        ));
        return Ok(1);
    };
    if !valid_name(name) {
        p.println("newgame: names are lowercase letters, digits and dashes, like rocket or my-game.");
        return Ok(1);
    }
    let dir = format!("{}/{name}", p.home());
    if p.fs().exists(&dir) {
        p.println(&format!(
            "newgame: {} is already here.",
            kiddos_vfs::path::tildify(&dir, &p.home())
        ));
        return Ok(1);
    }
    let files: Vec<(String, String, bool)> = vec![
        (
            "cart.toml".into(),
            format!(
                "name = \"{name}\"\ntitle = \"{title}\"\nversion = \"0.1.0\"\nauthor = \"{author}\"\nentry = \"{name}.bas\"\ndescription = \"a game by {author}\"\ncaps = [\"speak\", \"sound\"]\n",
                title = name.replace('-', " "),
                author = {
                    let n = p.kernel().kid_name.lock().clone();
                    if n.is_empty() { "kid".to_string() } else { n }
                }
            ),
            false,
        ),
        (
            format!("{name}.bas"),
            format!(
                "#!/bin/basic\n' {name}: a game. Change anything. Run it with:  run ~/{name}/{name}.bas\n\nCLS\nCOLOR 14\nPRINT \"  {name}\"\nCOLOR 7\nPRINT\nPRINT \"  Press a key. ESC ends the game.\"\n\nDO\n    k$ = KEY$\n    IF k$ = \"ESC\" THEN EXIT DO\n    PRINT \"  You pressed \"; k$\n    BEEP 660, 40\nLOOP\n\nEND 0\n"
            ),
            true,
        ),
        (
            "README.md".into(),
            format!("# {name}\n\nA game. Say what it is about here. Parents can share it with\n`share /home/kid/{name}` in parent mode.\n"),
            false,
        ),
        (
            format!("man/{name}.md"),
            format!("# {name}\n> a game\n\n## TRY THIS\n```\nplay {name}\n```\n"),
            false,
        ),
    ];
    let fs = p.fs();
    if let Err(e) = fs.mkdir_p(&format!("{dir}/man")) {
        p.complain(&e);
        return Ok(1);
    }
    for (rel, text, exec) in files {
        let path = format!("{dir}/{rel}");
        if let Err(e) = fs.write(&path, text.as_bytes()) {
            p.complain(&e);
            return Ok(1);
        }
        if exec {
            let _ = fs.chmod(&path, 0o755);
        }
    }
    p.println(&format!("Made ~/{name}/ with {name}.bas inside. Try:"));
    p.println(&format!("   run ~/{name}/{name}.bas"));
    p.println(&format!("   edit ~/{name}/{name}.bas"));
    p.println(&format!(
        "When it is good, a parent can give it away: parent, then share /home/kid/{name}"
    ));
    Ok(0)
}

fn cmd_carts(p: &Proc, _args: &[String]) -> CmdResult {
    let host = p.kernel().host();
    let files = host.list_cart_files();
    p.println(&format!("Cartridge files in {}:", host.cart_folder_hint()));
    if files.is_empty() {
        p.println("  (none; copy a .kdc file there and it will show up here)");
    }
    for f in files {
        p.println(&format!("  {f}"));
    }
    p.println("Installed games:");
    for m in list(p) {
        p.println(&format!("  {:<12} {} v{}", m.name, m.title, m.version));
    }
    Ok(0)
}

fn cmd_install(p: &Proc, args: &[String]) -> CmdResult {
    let Some(arg) = args.first() else {
        p.println(&p.t(
            "usage",
            &[("usage", "install <name or file.kdc>   (see them with carts)")],
        ));
        return Ok(1);
    };
    let file = if arg.ends_with(".kdc") {
        arg.clone()
    } else {
        format!("{arg}.kdc")
    };
    let host = p.kernel().host();
    let bytes = match host.read_cart_file(&file) {
        Ok(b) => b,
        Err(e) => {
            p.println(&format!("install: {e}"));
            p.println(&format!("Cartridge files live in {}", host.cart_folder_hint()));
            return Ok(1);
        }
    };
    let entries = match kdc::unpack(&bytes) {
        Ok(e) => e,
        Err(e) => {
            p.println(&format!("install: {e}"));
            return Ok(1);
        }
    };
    let manifest_text = entries
        .iter()
        .find(|e| e.path == "cart.toml")
        .map(|e| String::from_utf8_lossy(&e.data).to_string())
        .unwrap_or_default();
    let fallback = file.trim_end_matches(".kdc").to_string();
    let m = match parse_manifest(&fallback, &manifest_text) {
        Ok(m) => m,
        Err(e) => {
            p.println(&format!("install: {e}"));
            return Ok(1);
        }
    };
    if !valid_name(&m.name) {
        p.println(&format!(
            "install: {} is not a valid game name (lowercase letters, digits, dashes).",
            m.name
        ));
        return Ok(1);
    }
    let dir = dir_of(&m.name);
    let fs = p.fs();
    let replaced = fs.exists(&dir);
    if replaced {
        if let Err(e) = fs.remove_tree(&dir) {
            p.complain(&e);
            return Ok(1);
        }
    }
    for e in &entries {
        let path = format!("{dir}/{}", e.path);
        let parent = kiddos_vfs::dirname(&path);
        if let Err(err) = fs.mkdir_p(&parent) {
            p.complain(&err);
            return Ok(1);
        }
        if let Err(err) = fs.write(&path, &e.data) {
            p.complain(&err);
            return Ok(1);
        }
        if e.exec {
            let _ = fs.chmod(&path, 0o755);
        }
    }
    p.kernel().log(&format!("install {} from {file}", m.name));
    p.println(&format!(
        "{} {}: {} ({} files, unsigned cartridge). Kids can now: play {}",
        if replaced { "Replaced" } else { "Installed" },
        m.name,
        m.title,
        entries.len(),
        m.name
    ));
    Ok(0)
}

fn cmd_uninstall(p: &Proc, args: &[String]) -> CmdResult {
    let Some(name) = args.first() else {
        p.println(&p.t("usage", &[("usage", "uninstall <name>")]));
        return Ok(1);
    };
    if !valid_name(name) || !p.fs().is_dir(&dir_of(name)) {
        p.println(&format!("uninstall: there is no game called {name}. Type games."));
        return Ok(1);
    }
    if let Err(e) = p.fs().remove_tree(&dir_of(name)) {
        p.complain(&e);
        return Ok(1);
    }
    p.kernel().log(&format!("uninstall {name}"));
    p.println(&format!("Removed {name}."));
    Ok(0)
}

fn cmd_share(p: &Proc, args: &[String]) -> CmdResult {
    let Some(dir) = args.first() else {
        p.println(&p.t(
            "usage",
            &[(
                "usage",
                "share <game folder>   for example: share /home/kid/rocket   or   share /games/snake",
            )],
        ));
        return Ok(1);
    };
    let abs = p.fs().path(dir);
    let bytes = {
        let vfs = p.kernel().vfs.lock();
        kdc::pack(&vfs, &abs)
    };
    let bytes = match bytes {
        Ok(b) => b,
        Err(e) => {
            p.println(&format!("share: {e}"));
            return Ok(1);
        }
    };
    let manifest_text = p.fs().read_string(&format!("{abs}/cart.toml")).unwrap_or_default();
    let name = parse_manifest(kiddos_vfs::basename(&abs), &manifest_text)
        .map(|m| m.name)
        .unwrap_or_else(|_| kiddos_vfs::basename(&abs).to_string());
    let file = format!("{name}.kdc");
    match p.kernel().host().write_cart_file(&file, &bytes) {
        Ok(where_) => {
            p.kernel().log(&format!("share {abs} -> {file}"));
            p.println(&format!("Wrote {where_} ({} bytes). Copy that file to another KidDOS's cartridge folder and run install {name} there.", bytes.len()));
            Ok(0)
        }
        Err(e) => {
            p.println(&format!("share: {e}"));
            Ok(1)
        }
    }
}

#[cfg(test)]
mod kdc_tests {
    use super::kdc;
    use kiddos_kernel::{Actor, Vfs};

    #[test]
    fn pack_and_unpack_roundtrip() {
        let mut v = Vfs::new();
        let r = Actor::root();
        v.mkdir_p("/games/rocket/man", &r).unwrap();
        v.write("/games/rocket/cart.toml", b"name = \"rocket\"\n", &r).unwrap();
        v.write("/games/rocket/rocket.bas", b"#!/bin/basic\nPRINT 1\n", &r)
            .unwrap();
        v.chmod("/games/rocket/rocket.bas", 0o755, &r).unwrap();
        v.write("/games/rocket/man/rocket.md", b"# rocket\n", &r).unwrap();
        let bytes = kdc::pack(&v, "/games/rocket").unwrap();
        assert!(bytes.starts_with(b"PK"));
        let mut entries = kdc::unpack(&bytes).unwrap();
        entries.sort_by(|a, b| a.path.cmp(&b.path));
        let paths: Vec<&str> = entries.iter().map(|e| e.path.as_str()).collect();
        assert_eq!(paths, vec!["cart.toml", "man/rocket.md", "rocket.bas"]);
        assert!(entries[2].exec && !entries[0].exec);
        assert!(kdc::pack(&v, "/games").is_err());
        assert!(kdc::unpack(b"not a zip").is_err());
    }
}

// ---- toolchain packs (parent) ------------------------------------------

pub fn register_packs(k: &Kernel) {
    for c in [
        Command::new(
            "packs",
            cmd_packs,
            "toolchain packs (compilers) on this machine",
            Topic::Parent,
        )
        .parent(),
        Command::new(
            "install-pack",
            cmd_install_pack,
            "install a .kdp toolchain pack: install-pack c",
            Topic::Parent,
        )
        .parent(),
        Command::new("remove-pack", cmd_remove_pack, "remove a toolchain pack", Topic::Parent).parent(),
    ] {
        k.register(c);
    }
}

fn cmd_packs(p: &Proc, _args: &[String]) -> CmdResult {
    let host = p.kernel().host();
    let packs = host.list_packs();
    p.println("Installed packs:");
    if packs.is_empty() {
        p.println("  (none)");
    }
    for (name, desc) in packs {
        p.println(&format!("  {name:<10} {desc}"));
    }
    let kdp: Vec<String> = host
        .list_cart_files()
        .into_iter()
        .filter(|f| f.ends_with(".kdp"))
        .collect();
    if !kdp.is_empty() {
        p.println(&format!("Pack files waiting in {}:", host.cart_folder_hint()));
        for f in kdp {
            p.println(&format!("  {f}"));
        }
    }
    match host.c_compiler_available() {
        Ok(()) => p.println("cc works: the C pack is ready."),
        Err(e) => p.println(&format!("cc does not work yet: {e}")),
    }
    Ok(0)
}

fn cmd_install_pack(p: &Proc, args: &[String]) -> CmdResult {
    let Some(arg) = args.first() else {
        p.println(&p.t(
            "usage",
            &[("usage", "install-pack <name or file.kdp>   (see them with packs)")],
        ));
        return Ok(1);
    };
    let file = if arg.ends_with(".kdp") {
        arg.clone()
    } else {
        format!("{arg}.kdp")
    };
    match p.kernel().host().install_pack(&file) {
        Ok(summary) => {
            p.kernel().log(&format!("install-pack {file}"));
            p.println(&format!("Installed {summary}"));
            Ok(0)
        }
        Err(e) => {
            p.println(&format!("install-pack: {e}"));
            Ok(1)
        }
    }
}

fn cmd_remove_pack(p: &Proc, args: &[String]) -> CmdResult {
    let Some(name) = args.first() else {
        p.println(&p.t("usage", &[("usage", "remove-pack <name>")]));
        return Ok(1);
    };
    match p.kernel().host().remove_pack(name) {
        Ok(()) => {
            p.kernel().log(&format!("remove-pack {name}"));
            p.println(&format!("Removed pack {name}."));
            Ok(0)
        }
        Err(e) => {
            p.println(&format!("remove-pack: {e}"));
            Ok(1)
        }
    }
}
