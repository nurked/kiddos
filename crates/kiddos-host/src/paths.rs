//! The four files the app touches on the host, and nothing else.

use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct Paths {
    pub dir: PathBuf,
    pub drive: PathBuf,
    pub config: PathBuf,
    pub parent_hash: PathBuf,
    pub log: PathBuf,
    /// `.kdc` cartridges go in and out through here.
    pub carts: PathBuf,
}

impl Paths {
    /// `~/Library/Application Support/KidDOS` (mac), `%APPDATA%\KidDOS`
    /// (win), `~/.local/share/kiddos` (linux). `KIDDOS_HOME` overrides.
    pub fn default_dir() -> PathBuf {
        if let Ok(p) = std::env::var("KIDDOS_HOME") {
            return PathBuf::from(p);
        }
        directories::ProjectDirs::from("", "", "KidDOS")
            .map(|d| d.data_dir().to_path_buf())
            .unwrap_or_else(|| PathBuf::from(".kiddos"))
    }

    pub fn in_dir(dir: PathBuf) -> Paths {
        Paths {
            drive: dir.join("drive.kdd"),
            config: dir.join("config.toml"),
            parent_hash: dir.join("parent.hash"),
            log: dir.join("log.txt"),
            carts: dir.join("carts"),
            dir,
        }
    }

    pub fn new() -> Paths {
        Paths::in_dir(Paths::default_dir())
    }

    pub fn ensure(&self) -> std::io::Result<()> {
        std::fs::create_dir_all(&self.dir)?;
        std::fs::create_dir_all(&self.carts)
    }
}

impl Default for Paths {
    fn default() -> Self {
        Paths::new()
    }
}
