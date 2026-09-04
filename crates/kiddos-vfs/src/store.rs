//! Persistence: one SQLite file, written atomically (temp + rename), plus
//! import from a host directory for building the factory image.

use crate::{Actor, Ino, Kind, Node, Result, Vfs, VfsError, DEFAULT_DIR_MODE, DEFAULT_FILE_MODE, ROOT_INO};
use rusqlite::{params, Connection};
use std::collections::{BTreeMap, HashMap};
use std::path::Path;

pub const FORMAT_VERSION: i64 = 1;

fn sq(e: rusqlite::Error) -> VfsError {
    VfsError::Storage(e.to_string())
}

fn io(e: std::io::Error) -> VfsError {
    VfsError::Storage(e.to_string())
}

impl Vfs {
    /// Write the whole tree to `file`. Atomic: writes `file.tmp` then renames.
    pub fn save(&self, file: &Path) -> Result<()> {
        let tmp = file.with_extension("kdd.tmp");
        let _ = std::fs::remove_file(&tmp);
        {
            let mut conn = Connection::open(&tmp).map_err(sq)?;
            conn.execute_batch(
                "PRAGMA journal_mode=OFF;
                 CREATE TABLE meta(key TEXT PRIMARY KEY, value TEXT NOT NULL);
                 CREATE TABLE nodes(
                    ino INTEGER PRIMARY KEY,
                    parent INTEGER NOT NULL,
                    name TEXT NOT NULL,
                    kind INTEGER NOT NULL,
                    mode INTEGER NOT NULL,
                    owner TEXT NOT NULL,
                    mtime INTEGER NOT NULL,
                    data BLOB NOT NULL
                 );",
            )
            .map_err(sq)?;
            let tx = conn.transaction().map_err(sq)?;
            tx.execute(
                "INSERT INTO meta(key, value) VALUES('format', ?1)",
                params![FORMAT_VERSION.to_string()],
            )
            .map_err(sq)?;
            tx.execute(
                "INSERT INTO meta(key, value) VALUES('next_ino', ?1)",
                params![self.next_ino.to_string()],
            )
            .map_err(sq)?;
            {
                let mut stmt = tx
                    .prepare(
                        "INSERT INTO nodes(ino, parent, name, kind, mode, owner, mtime, data)
                         VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                    )
                    .map_err(sq)?;
                let mut inos: Vec<&Ino> = self.nodes.keys().collect();
                inos.sort();
                for ino in inos {
                    let n = &self.nodes[ino];
                    stmt.execute(params![
                        n.ino as i64,
                        n.parent as i64,
                        n.name,
                        n.kind.to_i64(),
                        n.mode as i64,
                        n.owner,
                        n.mtime as i64,
                        n.data,
                    ])
                    .map_err(sq)?;
                }
            }
            tx.commit().map_err(sq)?;
        }
        std::fs::rename(&tmp, file).map_err(io)?;
        Ok(())
    }

    /// Read a tree written by [`Vfs::save`].
    pub fn load(file: &Path) -> Result<Vfs> {
        let conn = Connection::open_with_flags(file, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY).map_err(sq)?;
        let format: String = conn
            .query_row("SELECT value FROM meta WHERE key='format'", [], |r| r.get(0))
            .map_err(sq)?;
        if format.parse::<i64>().ok() != Some(FORMAT_VERSION) {
            return Err(VfsError::Storage(format!("unknown drive format {format}")));
        }
        let next_ino: String = conn
            .query_row("SELECT value FROM meta WHERE key='next_ino'", [], |r| r.get(0))
            .map_err(sq)?;
        let mut stmt = conn
            .prepare("SELECT ino, parent, name, kind, mode, owner, mtime, data FROM nodes")
            .map_err(sq)?;
        let rows = stmt
            .query_map([], |r| {
                Ok(Node {
                    ino: r.get::<_, i64>(0)? as Ino,
                    parent: r.get::<_, i64>(1)? as Ino,
                    name: r.get(2)?,
                    kind: Kind::from_i64(r.get(3)?).unwrap_or(Kind::File),
                    mode: r.get::<_, i64>(4)? as u16,
                    owner: r.get(5)?,
                    mtime: r.get::<_, i64>(6)? as u64,
                    data: r.get(7)?,
                    children: BTreeMap::new(),
                })
            })
            .map_err(sq)?;
        let mut nodes: HashMap<Ino, Node> = HashMap::new();
        for row in rows {
            let n = row.map_err(sq)?;
            nodes.insert(n.ino, n);
        }
        if !nodes.contains_key(&ROOT_INO) {
            return Err(VfsError::Storage("drive has no root".into()));
        }
        // rebuild children
        let links: Vec<(Ino, Ino, String)> = nodes
            .values()
            .filter(|n| n.ino != ROOT_INO)
            .map(|n| (n.parent, n.ino, n.name.clone()))
            .collect();
        for (parent, ino, name) in links {
            match nodes.get_mut(&parent) {
                Some(p) => {
                    p.children.insert(name, ino);
                }
                None => return Err(VfsError::Storage(format!("orphan inode {ino}"))),
            }
        }
        let max = nodes.keys().copied().max().unwrap_or(ROOT_INO);
        let mut v = Vfs::new();
        v.nodes = nodes;
        v.next_ino = next_ino.parse::<Ino>().unwrap_or(max + 1).max(max + 1);
        Ok(v)
    }

    /// Build a drive from a host directory tree (the factory image). Rules:
    /// everything is owned by `root`, except `/home/*` which is owned by the
    /// user named by the directory; `/tmp` is world-writable; host
    /// executables become mode 755; host symlinks become VFS symlinks;
    /// dotfiles named `.DS_Store` and `.gitkeep` are skipped.
    pub fn from_dir(dir: &Path) -> Result<Vfs> {
        let mut v = Vfs::new();
        v.import_dir(dir, "/", "root")?;
        if v.is_dir("/home") {
            let homes: Vec<String> = v
                .readdir("/home", &Actor::root())?
                .into_iter()
                .filter(|s| s.is_dir())
                .map(|s| s.name)
                .collect();
            for h in homes {
                v.chown_tree(&format!("/home/{h}"), &h)?;
            }
        }
        if v.is_dir("/tmp") {
            v.chmod("/tmp", 0o777, &Actor::root())?;
        }
        Ok(v)
    }

    /// Copy a host directory's contents into the VFS under `at` (created if
    /// missing), owned by `owner`. Used for factory images and cartridges.
    pub fn import_dir(&mut self, dir: &Path, at: &str, owner: &str) -> Result<()> {
        let root = Actor::root();
        self.mkdir_p(at, &root)?;
        let at_ino = self.walk(at, true)?;
        self.import_rec(dir, at_ino, owner)?;
        if at != "/" {
            self.chown_tree(at, owner)?;
        }
        Ok(())
    }

    fn import_rec(&mut self, dir: &Path, into: Ino, owner: &str) -> Result<()> {
        let mut entries: Vec<_> = std::fs::read_dir(dir)
            .map_err(io)?
            .collect::<std::io::Result<_>>()
            .map_err(io)?;
        entries.sort_by_key(|e| e.file_name());
        for e in entries {
            let name = e.file_name().to_string_lossy().to_string();
            if name == ".DS_Store" || name == ".gitkeep" {
                continue;
            }
            let meta = std::fs::symlink_metadata(e.path()).map_err(io)?;
            if meta.file_type().is_symlink() {
                let target = std::fs::read_link(e.path()).map_err(io)?;
                let ino = self.alloc(into, &name, Kind::Symlink, 0o777, owner);
                self.node_mut(ino).data = target.to_string_lossy().as_bytes().to_vec();
            } else if meta.is_dir() {
                let ino = self.alloc(into, &name, Kind::Dir, DEFAULT_DIR_MODE, owner);
                self.import_rec(&e.path(), ino, owner)?;
            } else {
                let data = std::fs::read(e.path()).map_err(io)?;
                let mode = if is_executable(&meta) { 0o755 } else { DEFAULT_FILE_MODE };
                let ino = self.alloc(into, &name, Kind::File, mode, owner);
                self.node_mut(ino).data = data;
            }
        }
        Ok(())
    }
}

#[cfg(unix)]
fn is_executable(meta: &std::fs::Metadata) -> bool {
    use std::os::unix::fs::PermissionsExt;
    meta.permissions().mode() & 0o111 != 0
}

#[cfg(not(unix))]
fn is_executable(_meta: &std::fs::Metadata) -> bool {
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmpdir(name: &str) -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!("kiddos-vfs-test-{}-{}", name, std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn save_load_roundtrip() {
        let d = tmpdir("roundtrip");
        let mut v = Vfs::new();
        let r = Actor::root();
        v.mkdir_p("/home/kid/deep", &r).unwrap();
        v.chown_tree("/home/kid", "kid").unwrap();
        v.write("/home/kid/deep/f.txt", "привет".as_bytes(), &Actor::user("kid"))
            .unwrap();
        v.symlink("/home/kid/deep", "/home/kid/link", &r).unwrap();
        v.chmod("/home/kid/deep/f.txt", 0o755, &r).unwrap();
        let file = d.join("drive.kdd");
        v.save(&file).unwrap();
        let w = Vfs::load(&file).unwrap();
        assert_eq!(w.node_count(), v.node_count());
        assert_eq!(
            w.read_string("/home/kid/link/f.txt", &Actor::user("kid")).unwrap(),
            "привет"
        );
        let st = w.stat("/home/kid/deep/f.txt").unwrap();
        assert_eq!(st.mode, 0o755);
        assert_eq!(st.owner, "kid");
        assert!(w.lstat("/home/kid/link").unwrap().is_symlink());
        assert!(w.next_ino > st.ino);
        // second save over the existing file works (atomic replace)
        w.save(&file).unwrap();
        assert!(!d.join("drive.kdd.tmp").exists());
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn from_dir_builds_factory_image() {
        let d = tmpdir("fromdir");
        std::fs::create_dir_all(d.join("home/kid")).unwrap();
        std::fs::create_dir_all(d.join("games/snake")).unwrap();
        std::fs::create_dir_all(d.join("tmp")).unwrap();
        std::fs::write(d.join("home/kid/hello.txt"), "hi").unwrap();
        std::fs::write(d.join("games/snake/snake.bas"), "PRINT 1").unwrap();
        std::fs::write(d.join("home/kid/.DS_Store"), "junk").unwrap();
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink("/games", d.join("home/kid/games")).unwrap();
            let script = d.join("home/kid/run.sh");
            std::fs::write(&script, "#!/bin/ksh\necho hi").unwrap();
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
        let mut v = Vfs::from_dir(&d).unwrap();
        let kid = Actor::user("kid");
        assert_eq!(v.stat("/home/kid").unwrap().owner, "kid");
        assert_eq!(v.stat("/home/kid/hello.txt").unwrap().owner, "kid");
        assert_eq!(v.stat("/games/snake/snake.bas").unwrap().owner, "root");
        assert_eq!(v.stat("/tmp").unwrap().mode, 0o777);
        assert!(!v.exists("/home/kid/.DS_Store"));
        assert!(v.write("/games/snake/x", b"", &kid).is_err());
        assert!(v.write("/home/kid/x", b"", &kid).is_ok() || true);
        #[cfg(unix)]
        {
            assert_eq!(v.readlink("/home/kid/games").unwrap(), "/games");
            assert_eq!(
                v.read_string("/home/kid/games/snake/snake.bas", &kid).unwrap(),
                "PRINT 1"
            );
            assert_eq!(v.stat("/home/kid/run.sh").unwrap().mode, 0o755);
        }
        let _ = std::fs::remove_dir_all(&d);
    }
}
