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
    if !p.fs().exists(&entry) {
        return Err(format!("{name} has no {} to run.", m.entry));
    }
    let mut argv = vec![entry];
    argv.extend(args.iter().cloned());
    let mut s = Spawn::child_of(p, argv);
    s.env.insert("CART".into(), dir);
    s.env.insert("GAME".into(), name.to_string());
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
