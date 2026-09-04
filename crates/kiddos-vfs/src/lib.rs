//! The virtual hard drive.
//!
//! Everything the kid can `ls` lives here. The tree is in memory; [`Vfs::save`]
//! writes it atomically to one SQLite file, [`Vfs::load`] reads it back, and
//! [`Vfs::from_dir`] builds a factory image from a host directory at build
//! time. Paths handed to the VFS are always absolute; use [`normalize`] to
//! join a relative path against a cwd first.
//!
//! Permissions are a teachable simplification of Unix: `rwx` for the owner and
//! `rwx` for everyone else (group bits are stored and shown but treated like
//! "other"). `root` bypasses everything.

pub mod path;
mod store;

pub use path::{basename, dirname, normalize};

use std::collections::{BTreeMap, HashMap};

pub type Ino = u64;
pub const ROOT_INO: Ino = 1;

pub const R: u16 = 4;
pub const W: u16 = 2;
pub const X: u16 = 1;

pub const DEFAULT_FILE_MODE: u16 = 0o644;
pub const DEFAULT_DIR_MODE: u16 = 0o755;

const MAX_SYMLINK_DEPTH: u32 = 40;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Kind {
    Dir,
    File,
    Symlink,
}

impl Kind {
    fn to_i64(self) -> i64 {
        match self {
            Kind::Dir => 0,
            Kind::File => 1,
            Kind::Symlink => 2,
        }
    }
    fn from_i64(v: i64) -> Option<Kind> {
        Some(match v {
            0 => Kind::Dir,
            1 => Kind::File,
            2 => Kind::Symlink,
            _ => return None,
        })
    }
}

/// Who is doing the operation. Builtins run as `kid`; parent mode is `root`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Actor {
    pub user: String,
    pub root: bool,
}

impl Actor {
    pub fn user(name: &str) -> Actor {
        Actor {
            user: name.to_string(),
            root: name == "root",
        }
    }
    pub fn root() -> Actor {
        Actor::user("root")
    }
}

#[derive(Debug, Clone)]
pub struct Node {
    pub ino: Ino,
    pub parent: Ino,
    pub name: String,
    pub kind: Kind,
    pub mode: u16,
    pub owner: String,
    pub mtime: u64,
    /// File contents, or the symlink target as UTF-8.
    pub data: Vec<u8>,
    pub children: BTreeMap<String, Ino>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Stat {
    pub ino: Ino,
    pub name: String,
    pub kind: Kind,
    pub mode: u16,
    pub owner: String,
    pub mtime: u64,
    pub size: u64,
}

impl Stat {
    pub fn is_dir(&self) -> bool {
        self.kind == Kind::Dir
    }
    pub fn is_file(&self) -> bool {
        self.kind == Kind::File
    }
    pub fn is_symlink(&self) -> bool {
        self.kind == Kind::Symlink
    }
    /// `-rwxr-xr-x` style string.
    pub fn mode_string(&self) -> String {
        let k = match self.kind {
            Kind::Dir => 'd',
            Kind::File => '-',
            Kind::Symlink => 'l',
        };
        let mut s = String::with_capacity(10);
        s.push(k);
        for shift in [6u16, 3, 0] {
            let bits = (self.mode >> shift) & 7;
            s.push(if bits & R != 0 { 'r' } else { '-' });
            s.push(if bits & W != 0 { 'w' } else { '-' });
            s.push(if bits & X != 0 { 'x' } else { '-' });
        }
        s
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum VfsError {
    #[error("{0}: No such file or directory")]
    NotFound(String),
    #[error("{0}: Not a directory")]
    NotADir(String),
    #[error("{0}: Is a directory")]
    IsADir(String),
    #[error("{0}: File exists")]
    Exists(String),
    #[error("{0}: Directory not empty")]
    NotEmpty(String),
    #[error("{0}: Permission denied")]
    Permission(String),
    #[error("{0}: Too many levels of symbolic links")]
    Loop(String),
    #[error("{0}: Invalid argument")]
    Invalid(String),
    #[error("{0}: Operation not permitted")]
    NotPermitted(String),
    #[error("storage error: {0}")]
    Storage(String),
}

pub type Result<T> = std::result::Result<T, VfsError>;

pub struct Vfs {
    nodes: HashMap<Ino, Node>,
    next_ino: Ino,
    changes: u64,
    clock: Box<dyn Fn() -> u64 + Send + Sync>,
}

impl std::fmt::Debug for Vfs {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Vfs({} nodes)", self.nodes.len())
    }
}

impl Default for Vfs {
    fn default() -> Self {
        Vfs::new()
    }
}

fn system_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

impl Vfs {
    /// An empty drive with just `/`.
    pub fn new() -> Vfs {
        let mut nodes = HashMap::new();
        nodes.insert(
            ROOT_INO,
            Node {
                ino: ROOT_INO,
                parent: ROOT_INO,
                name: String::new(),
                kind: Kind::Dir,
                mode: DEFAULT_DIR_MODE,
                owner: "root".into(),
                mtime: system_now(),
                data: Vec::new(),
                children: BTreeMap::new(),
            },
        );
        Vfs {
            nodes,
            next_ino: ROOT_INO + 1,
            changes: 0,
            clock: Box::new(system_now),
        }
    }

    /// Replace the clock (the kernel passes the machine clock; tests pass a
    /// constant).
    pub fn set_clock(&mut self, clock: Box<dyn Fn() -> u64 + Send + Sync>) {
        self.clock = clock;
    }

    /// Increments on every mutation. The autosaver watches this.
    pub fn changes(&self) -> u64 {
        self.changes
    }

    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    fn now(&self) -> u64 {
        (self.clock)()
    }

    fn touch_changes(&mut self) {
        self.changes += 1;
    }

    fn node(&self, ino: Ino) -> &Node {
        self.nodes.get(&ino).expect("dangling inode")
    }

    fn node_mut(&mut self, ino: Ino) -> &mut Node {
        self.nodes.get_mut(&ino).expect("dangling inode")
    }

    fn alloc(&mut self, parent: Ino, name: &str, kind: Kind, mode: u16, owner: &str) -> Ino {
        let ino = self.next_ino;
        self.next_ino += 1;
        let now = self.now();
        self.nodes.insert(
            ino,
            Node {
                ino,
                parent,
                name: name.to_string(),
                kind,
                mode,
                owner: owner.to_string(),
                mtime: now,
                data: Vec::new(),
                children: BTreeMap::new(),
            },
        );
        let p = self.node_mut(parent);
        p.children.insert(name.to_string(), ino);
        p.mtime = now;
        self.touch_changes();
        ino
    }

    fn can(&self, node: &Node, actor: &Actor, bit: u16) -> bool {
        if actor.root {
            return true;
        }
        let shift = if node.owner == actor.user { 6 } else { 0 };
        (node.mode >> shift) & bit != 0
    }

    fn check(&self, node: &Node, actor: &Actor, bit: u16, path: &str) -> Result<()> {
        if self.can(node, actor, bit) {
            Ok(())
        } else {
            Err(VfsError::Permission(path.to_string()))
        }
    }

    fn components(path: &str) -> Result<Vec<&str>> {
        if !path.starts_with('/') {
            return Err(VfsError::Invalid(path.to_string()));
        }
        Ok(path.split('/').filter(|c| !c.is_empty() && *c != ".").collect())
    }

    /// Walk `path`, following symlinks. If `follow_last` is false the final
    /// component may be a symlink and is returned as such.
    fn walk(&self, path: &str, follow_last: bool) -> Result<Ino> {
        let comps = Self::components(path)?;
        self.walk_from(ROOT_INO, &comps, follow_last, 0, path)
    }

    fn walk_from(&self, start: Ino, comps: &[&str], follow_last: bool, depth: u32, orig: &str) -> Result<Ino> {
        if depth > MAX_SYMLINK_DEPTH {
            return Err(VfsError::Loop(orig.to_string()));
        }
        let mut cur = start;
        for (i, comp) in comps.iter().enumerate() {
            let node = self.node(cur);
            if node.kind != Kind::Dir {
                return Err(VfsError::NotADir(orig.to_string()));
            }
            let next = if *comp == ".." {
                node.parent
            } else {
                match node.children.get(*comp) {
                    Some(&n) => n,
                    None => return Err(VfsError::NotFound(orig.to_string())),
                }
            };
            let is_last = i + 1 == comps.len();
            let next_node = self.node(next);
            if next_node.kind == Kind::Symlink && (!is_last || follow_last) {
                let target = String::from_utf8_lossy(&next_node.data).to_string();
                let tcomps: Vec<&str> = target.split('/').filter(|c| !c.is_empty() && *c != ".").collect();
                let base = if target.starts_with('/') { ROOT_INO } else { cur };
                let resolved = self.walk_from(base, &tcomps, true, depth + 1, orig)?;
                if is_last {
                    return Ok(resolved);
                }
                cur = resolved;
            } else {
                cur = next;
            }
        }
        Ok(cur)
    }

    /// Split into (parent inode, basename), resolving symlinks in the parent.
    fn parent_of(&self, path: &str) -> Result<(Ino, String)> {
        let comps = Self::components(path)?;
        let Some(last) = comps.last() else {
            return Err(VfsError::Invalid(path.to_string()));
        };
        if *last == ".." {
            return Err(VfsError::Invalid(path.to_string()));
        }
        let parent = self.walk_from(ROOT_INO, &comps[..comps.len() - 1], true, 0, path)?;
        if self.node(parent).kind != Kind::Dir {
            return Err(VfsError::NotADir(path.to_string()));
        }
        Ok((parent, last.to_string()))
    }

    fn stat_of(&self, ino: Ino) -> Stat {
        let n = self.node(ino);
        Stat {
            ino,
            name: n.name.clone(),
            kind: n.kind,
            mode: n.mode,
            owner: n.owner.clone(),
            mtime: n.mtime,
            size: match n.kind {
                Kind::Dir => n.children.len() as u64,
                _ => n.data.len() as u64,
            },
        }
    }

    // ---- queries -------------------------------------------------------

    /// Does `actor` have permission `bit` (R/W/X) on `path`?
    pub fn has_access(&self, path: &str, actor: &Actor, bit: u16) -> Result<bool> {
        let ino = self.walk(path, true)?;
        Ok(self.can(self.node(ino), actor, bit))
    }

    pub fn exists(&self, path: &str) -> bool {
        self.walk(path, true).is_ok()
    }

    pub fn is_dir(&self, path: &str) -> bool {
        self.walk(path, true)
            .map(|i| self.node(i).kind == Kind::Dir)
            .unwrap_or(false)
    }

    /// Stat following symlinks.
    pub fn stat(&self, path: &str) -> Result<Stat> {
        Ok(self.stat_of(self.walk(path, true)?))
    }

    /// Stat without following the final symlink.
    pub fn lstat(&self, path: &str) -> Result<Stat> {
        Ok(self.stat_of(self.walk(path, false)?))
    }

    pub fn readlink(&self, path: &str) -> Result<String> {
        let ino = self.walk(path, false)?;
        let n = self.node(ino);
        if n.kind != Kind::Symlink {
            return Err(VfsError::Invalid(path.to_string()));
        }
        Ok(String::from_utf8_lossy(&n.data).to_string())
    }

    /// The logical path of an inode (`/home/kid/x`).
    pub fn path_of(&self, ino: Ino) -> String {
        let mut parts = Vec::new();
        let mut cur = ino;
        while cur != ROOT_INO {
            let n = self.node(cur);
            parts.push(n.name.clone());
            cur = n.parent;
        }
        parts.reverse();
        format!("/{}", parts.join("/"))
    }

    /// Resolve symlinks to the real path.
    pub fn realpath(&self, path: &str) -> Result<String> {
        Ok(self.path_of(self.walk(path, true)?))
    }

    pub fn read(&self, path: &str, actor: &Actor) -> Result<Vec<u8>> {
        let ino = self.walk(path, true)?;
        let n = self.node(ino);
        if n.kind == Kind::Dir {
            return Err(VfsError::IsADir(path.to_string()));
        }
        self.check(n, actor, R, path)?;
        Ok(n.data.clone())
    }

    pub fn read_string(&self, path: &str, actor: &Actor) -> Result<String> {
        Ok(String::from_utf8_lossy(&self.read(path, actor)?).to_string())
    }

    pub fn readdir(&self, path: &str, actor: &Actor) -> Result<Vec<Stat>> {
        let ino = self.walk(path, true)?;
        let n = self.node(ino);
        if n.kind != Kind::Dir {
            return Err(VfsError::NotADir(path.to_string()));
        }
        self.check(n, actor, R, path)?;
        Ok(n.children.values().map(|&c| self.stat_of(c)).collect())
    }

    /// Recursive size in bytes of a file or directory subtree.
    pub fn size_of(&self, path: &str) -> Result<u64> {
        let ino = self.walk(path, true)?;
        Ok(self.size_rec(ino))
    }

    fn size_rec(&self, ino: Ino) -> u64 {
        let n = self.node(ino);
        match n.kind {
            Kind::Dir => n.children.values().map(|&c| self.size_rec(c)).sum(),
            _ => n.data.len() as u64,
        }
    }

    /// Visit `path` and everything under it (depth-first, sorted), calling
    /// `f(path, stat, depth)`. Symlinks are reported, not followed.
    pub fn walk_tree(&self, path: &str, f: &mut dyn FnMut(&str, &Stat, usize)) -> Result<()> {
        let ino = self.walk(path, true)?;
        let base = if path == "/" {
            String::new()
        } else {
            path.trim_end_matches('/').to_string()
        };
        let st = self.stat_of(ino);
        f(if base.is_empty() { "/" } else { &base }, &st, 0);
        self.walk_rec(ino, &base, 1, f);
        Ok(())
    }

    fn walk_rec(&self, ino: Ino, base: &str, depth: usize, f: &mut dyn FnMut(&str, &Stat, usize)) {
        let n = self.node(ino);
        if n.kind != Kind::Dir {
            return;
        }
        for (name, &c) in &n.children {
            let p = format!("{}/{}", base, name);
            let st = self.stat_of(c);
            f(&p, &st, depth);
            if st.kind == Kind::Dir {
                self.walk_rec(c, &p, depth + 1, f);
            }
        }
    }

    /// Total bytes across all files (for `df`).
    pub fn used_bytes(&self) -> u64 {
        self.nodes
            .values()
            .filter(|n| n.kind == Kind::File)
            .map(|n| n.data.len() as u64)
            .sum()
    }

    // ---- mutations -----------------------------------------------------

    /// Create or truncate a file and write `data`.
    pub fn write(&mut self, path: &str, data: &[u8], actor: &Actor) -> Result<()> {
        self.write_impl(path, data, false, actor)
    }

    pub fn append(&mut self, path: &str, data: &[u8], actor: &Actor) -> Result<()> {
        self.write_impl(path, data, true, actor)
    }

    fn write_impl(&mut self, path: &str, data: &[u8], append: bool, actor: &Actor) -> Result<()> {
        match self.walk(path, true) {
            Ok(ino) => {
                let n = self.node(ino);
                if n.kind == Kind::Dir {
                    return Err(VfsError::IsADir(path.to_string()));
                }
                self.check(n, actor, W, path)?;
                let now = self.now();
                let n = self.node_mut(ino);
                if append {
                    n.data.extend_from_slice(data);
                } else {
                    n.data = data.to_vec();
                }
                n.mtime = now;
                self.touch_changes();
                Ok(())
            }
            Err(VfsError::NotFound(_)) => {
                let (parent, name) = self.parent_of(path)?;
                let p = self.node(parent);
                self.check(p, actor, W, path)?;
                let ino = self.alloc(parent, &name, Kind::File, DEFAULT_FILE_MODE, &actor.user);
                self.node_mut(ino).data = data.to_vec();
                Ok(())
            }
            Err(e) => Err(e),
        }
    }

    /// Create an empty file if missing, otherwise update its mtime.
    pub fn touch(&mut self, path: &str, actor: &Actor) -> Result<()> {
        match self.walk(path, true) {
            Ok(ino) => {
                let n = self.node(ino);
                self.check(n, actor, W, path)?;
                let now = self.now();
                self.node_mut(ino).mtime = now;
                self.touch_changes();
                Ok(())
            }
            Err(VfsError::NotFound(_)) => self.write(path, &[], actor),
            Err(e) => Err(e),
        }
    }

    pub fn mkdir(&mut self, path: &str, actor: &Actor) -> Result<()> {
        if self.walk(path, false).is_ok() {
            return Err(VfsError::Exists(path.to_string()));
        }
        let (parent, name) = self.parent_of(path)?;
        let p = self.node(parent);
        self.check(p, actor, W, path)?;
        self.alloc(parent, &name, Kind::Dir, DEFAULT_DIR_MODE, &actor.user);
        Ok(())
    }

    /// `mkdir -p`
    pub fn mkdir_p(&mut self, path: &str, actor: &Actor) -> Result<()> {
        let comps = Self::components(path)?;
        let mut cur = String::new();
        for c in comps {
            cur.push('/');
            cur.push_str(c);
            match self.walk(&cur, true) {
                Ok(ino) => {
                    if self.node(ino).kind != Kind::Dir {
                        return Err(VfsError::NotADir(cur));
                    }
                }
                Err(VfsError::NotFound(_)) => self.mkdir(&cur, actor)?,
                Err(e) => return Err(e),
            }
        }
        Ok(())
    }

    pub fn symlink(&mut self, target: &str, linkpath: &str, actor: &Actor) -> Result<()> {
        if self.walk(linkpath, false).is_ok() {
            return Err(VfsError::Exists(linkpath.to_string()));
        }
        let (parent, name) = self.parent_of(linkpath)?;
        let p = self.node(parent);
        self.check(p, actor, W, linkpath)?;
        let ino = self.alloc(parent, &name, Kind::Symlink, 0o777, &actor.user);
        self.node_mut(ino).data = target.as_bytes().to_vec();
        Ok(())
    }

    fn detach(&mut self, ino: Ino) {
        let (parent, name) = {
            let n = self.node(ino);
            (n.parent, n.name.clone())
        };
        let now = self.now();
        let p = self.node_mut(parent);
        p.children.remove(&name);
        p.mtime = now;
        self.nodes.remove(&ino);
        self.touch_changes();
    }

    /// Remove a file or symlink.
    pub fn unlink(&mut self, path: &str, actor: &Actor) -> Result<()> {
        let ino = self.walk(path, false)?;
        if ino == ROOT_INO {
            return Err(VfsError::NotPermitted(path.to_string()));
        }
        let n = self.node(ino);
        if n.kind == Kind::Dir {
            return Err(VfsError::IsADir(path.to_string()));
        }
        let parent = self.node(n.parent);
        self.check(parent, actor, W, path)?;
        self.detach(ino);
        Ok(())
    }

    /// Remove an empty directory.
    pub fn rmdir(&mut self, path: &str, actor: &Actor) -> Result<()> {
        let ino = self.walk(path, false)?;
        if ino == ROOT_INO {
            return Err(VfsError::NotPermitted(path.to_string()));
        }
        let n = self.node(ino);
        if n.kind != Kind::Dir {
            return Err(VfsError::NotADir(path.to_string()));
        }
        if !n.children.is_empty() {
            return Err(VfsError::NotEmpty(path.to_string()));
        }
        let parent = self.node(n.parent);
        self.check(parent, actor, W, path)?;
        self.detach(ino);
        Ok(())
    }

    /// `rm -r`. Checks permissions on the whole subtree before touching
    /// anything, so it either fully succeeds or does nothing.
    pub fn remove_tree(&mut self, path: &str, actor: &Actor) -> Result<()> {
        let ino = self.walk(path, false)?;
        if ino == ROOT_INO {
            return Err(VfsError::NotPermitted(path.to_string()));
        }
        let parent = self.node(self.node(ino).parent);
        self.check(parent, actor, W, path)?;
        self.check_tree_writable(ino, actor, path)?;
        self.remove_rec(ino);
        self.detach(ino);
        Ok(())
    }

    fn check_tree_writable(&self, ino: Ino, actor: &Actor, path: &str) -> Result<()> {
        let n = self.node(ino);
        if n.kind == Kind::Dir {
            self.check(n, actor, W, path)?;
            for &c in n.children.values() {
                self.check_tree_writable(c, actor, path)?;
            }
        }
        Ok(())
    }

    fn remove_rec(&mut self, ino: Ino) {
        let children: Vec<Ino> = self.node(ino).children.values().copied().collect();
        for c in children {
            self.remove_rec(c);
            self.nodes.remove(&c);
        }
        self.node_mut(ino).children.clear();
    }

    /// Rename/move. `to` must not exist. Moving a directory into itself is
    /// rejected.
    pub fn rename(&mut self, from: &str, to: &str, actor: &Actor) -> Result<()> {
        let ino = self.walk(from, false)?;
        if ino == ROOT_INO {
            return Err(VfsError::NotPermitted(from.to_string()));
        }
        if self.walk(to, false).is_ok() {
            return Err(VfsError::Exists(to.to_string()));
        }
        let (new_parent, new_name) = self.parent_of(to)?;
        // no moving a dir under itself
        let mut cur = new_parent;
        loop {
            if cur == ino {
                return Err(VfsError::Invalid(to.to_string()));
            }
            if cur == ROOT_INO {
                break;
            }
            cur = self.node(cur).parent;
        }
        let old_parent = self.node(ino).parent;
        self.check(self.node(old_parent), actor, W, from)?;
        self.check(self.node(new_parent), actor, W, to)?;
        let old_name = self.node(ino).name.clone();
        let now = self.now();
        self.node_mut(old_parent).children.remove(&old_name);
        self.node_mut(old_parent).mtime = now;
        let np = self.node_mut(new_parent);
        np.children.insert(new_name.clone(), ino);
        np.mtime = now;
        let n = self.node_mut(ino);
        n.parent = new_parent;
        n.name = new_name;
        self.touch_changes();
        Ok(())
    }

    pub fn chmod(&mut self, path: &str, mode: u16, actor: &Actor) -> Result<()> {
        let ino = self.walk(path, true)?;
        let n = self.node(ino);
        if !actor.root && n.owner != actor.user {
            return Err(VfsError::NotPermitted(path.to_string()));
        }
        self.node_mut(ino).mode = mode & 0o777;
        self.touch_changes();
        Ok(())
    }

    pub fn chown(&mut self, path: &str, owner: &str, actor: &Actor) -> Result<()> {
        if !actor.root {
            return Err(VfsError::NotPermitted(path.to_string()));
        }
        let ino = self.walk(path, true)?;
        self.node_mut(ino).owner = owner.to_string();
        self.touch_changes();
        Ok(())
    }

    /// Replace the subtree at `path` with the one from `other` (owned by
    /// root). Missing in `other`: removed here. Used to refresh the
    /// machine's own folders from a newer factory image without touching
    /// anything the kid or a parent made.
    pub fn replace_subtree_from(&mut self, other: &Vfs, path: &str) -> Result<()> {
        let root = Actor::root();
        let src = match other.walk(path, false) {
            Ok(i) => i,
            Err(VfsError::NotFound(_)) => {
                if self.exists(path) {
                    let _ = self.remove_tree(path, &root);
                }
                return Ok(());
            }
            Err(e) => return Err(e),
        };
        if self.walk(path, false).is_ok() {
            self.remove_tree(path, &root)?;
        }
        let parent = dirname(path);
        self.mkdir_p(&parent, &root)?;
        self.copy_node_from(other, src, path)
    }

    fn copy_node_from(&mut self, other: &Vfs, src: Ino, dest: &str) -> Result<()> {
        let root = Actor::root();
        let n = other.node(src);
        match n.kind {
            Kind::Dir => {
                self.mkdir(dest, &root)?;
                self.chmod(dest, n.mode, &root)?;
                for (name, &child) in &n.children {
                    self.copy_node_from(other, child, &format!("{}/{}", dest.trim_end_matches('/'), name))?;
                }
            }
            Kind::File => {
                self.write(dest, &n.data, &root)?;
                self.chmod(dest, n.mode, &root)?;
            }
            Kind::Symlink => {
                self.symlink(&String::from_utf8_lossy(&n.data), dest, &root)?;
            }
        }
        Ok(())
    }

    /// Bring the machine's own content up to date from a newer factory
    /// image: `/etc`, `/usr`, `/lessons`, and every game the factory ships
    /// under `/games`. Games installed by a parent and everything under
    /// `/home` are left alone.
    pub fn refresh_from_factory(&mut self, factory: &Vfs) -> Result<()> {
        for p in ["/etc", "/usr", "/lessons", "/dev"] {
            self.replace_subtree_from(factory, p)?;
        }
        let root = Actor::root();
        self.mkdir_p("/games", &root)?;
        for e in factory.readdir("/games", &root)? {
            self.replace_subtree_from(factory, &format!("/games/{}", e.name))?;
        }
        Ok(())
    }

    /// Recursively set owner (used when building the factory image).
    pub fn chown_tree(&mut self, path: &str, owner: &str) -> Result<()> {
        let ino = self.walk(path, true)?;
        let mut stack = vec![ino];
        while let Some(i) = stack.pop() {
            let n = self.node_mut(i);
            n.owner = owner.to_string();
            stack.extend(n.children.values().copied());
        }
        self.touch_changes();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kid() -> Actor {
        Actor::user("kid")
    }

    fn drive() -> Vfs {
        let mut v = Vfs::new();
        v.set_clock(Box::new(|| 1_000));
        let r = Actor::root();
        v.mkdir_p("/home/kid", &r).unwrap();
        v.chown_tree("/home/kid", "kid").unwrap();
        v.mkdir("/games", &r).unwrap();
        v.write("/games/readme", b"hi", &r).unwrap();
        v.mkdir("/tmp", &r).unwrap();
        v.chmod("/tmp", 0o777, &r).unwrap();
        v
    }

    #[test]
    fn basic_files() {
        let mut v = drive();
        v.write("/home/kid/a.txt", b"hello", &kid()).unwrap();
        assert_eq!(v.read("/home/kid/a.txt", &kid()).unwrap(), b"hello");
        v.append("/home/kid/a.txt", b" world", &kid()).unwrap();
        assert_eq!(v.read_string("/home/kid/a.txt", &kid()).unwrap(), "hello world");
        let st = v.stat("/home/kid/a.txt").unwrap();
        assert_eq!(st.size, 11);
        assert_eq!(st.owner, "kid");
        assert_eq!(st.mode_string(), "-rw-r--r--");
        assert_eq!(v.stat("/home/kid").unwrap().mode_string(), "drwxr-xr-x");
    }

    #[test]
    fn permissions() {
        let mut v = drive();
        assert_eq!(
            v.write("/games/x", b"", &kid()),
            Err(VfsError::Permission("/games/x".into()))
        );
        assert_eq!(
            v.write("/games/readme", b"", &kid()),
            Err(VfsError::Permission("/games/readme".into()))
        );
        assert!(v.read("/games/readme", &kid()).is_ok());
        assert!(v.write("/tmp/x", b"1", &kid()).is_ok());
        assert!(v.chmod("/games/readme", 0o777, &kid()).is_err());
        v.chmod("/home/kid", 0o500, &kid()).unwrap();
        assert!(v.write("/home/kid/no", b"", &kid()).is_err());
        assert!(v.write("/home/kid/yes", b"", &Actor::root()).is_ok());
    }

    #[test]
    fn dirs_and_removal() {
        let mut v = drive();
        v.mkdir("/home/kid/d", &kid()).unwrap();
        v.write("/home/kid/d/f", b"x", &kid()).unwrap();
        assert_eq!(
            v.rmdir("/home/kid/d", &kid()),
            Err(VfsError::NotEmpty("/home/kid/d".into()))
        );
        assert_eq!(
            v.unlink("/home/kid/d", &kid()),
            Err(VfsError::IsADir("/home/kid/d".into()))
        );
        v.remove_tree("/home/kid/d", &kid()).unwrap();
        assert!(!v.exists("/home/kid/d"));
        assert_eq!(v.node_count(), 6);
        assert_eq!(v.mkdir("/home/kid", &kid()), Err(VfsError::Exists("/home/kid".into())));
        assert_eq!(v.unlink("/", &Actor::root()), Err(VfsError::NotPermitted("/".into())));
    }

    #[test]
    fn symlinks() {
        let mut v = drive();
        v.symlink("/games", "/home/kid/games", &kid()).unwrap();
        assert_eq!(v.read("/home/kid/games/readme", &kid()).unwrap(), b"hi");
        assert!(v.lstat("/home/kid/games").unwrap().is_symlink());
        assert!(v.stat("/home/kid/games").unwrap().is_dir());
        assert_eq!(v.realpath("/home/kid/games/readme").unwrap(), "/games/readme");
        // `..` is physical, like a real kernel: games -> /games, .. -> /
        assert_eq!(v.realpath("/home/kid/games/.."), Ok("/".to_string()));
        v.symlink("loop", "/home/kid/loop", &kid()).unwrap();
        assert!(matches!(v.read("/home/kid/loop", &kid()), Err(VfsError::Loop(_))));
        assert_eq!(v.readlink("/home/kid/games").unwrap(), "/games");
    }

    #[test]
    fn rename() {
        let mut v = drive();
        v.mkdir("/home/kid/a", &kid()).unwrap();
        v.write("/home/kid/a/f", b"1", &kid()).unwrap();
        v.rename("/home/kid/a", "/home/kid/b", &kid()).unwrap();
        assert_eq!(v.read("/home/kid/b/f", &kid()).unwrap(), b"1");
        assert_eq!(v.path_of(v.stat("/home/kid/b/f").unwrap().ino), "/home/kid/b/f");
        assert_eq!(
            v.rename("/home/kid/b", "/home/kid/b/c", &kid()),
            Err(VfsError::Invalid("/home/kid/b/c".into()))
        );
        assert!(v.rename("/home/kid/b/f", "/games/f", &kid()).is_err());
    }

    #[test]
    fn refresh_keeps_the_kid_and_updates_the_machine() {
        let mut old = drive();
        old.write("/games/readme", b"old text", &Actor::root()).unwrap();
        old.mkdir_p("/games/snake", &Actor::root()).unwrap();
        old.write("/games/snake/snake.bas", b"v1", &Actor::root()).unwrap();
        old.mkdir_p("/games/frombob", &Actor::root()).unwrap();
        old.write("/games/frombob/cart.toml", b"name = \"frombob\"", &Actor::root())
            .unwrap();
        old.write("/home/kid/mine.txt", b"keep", &kid()).unwrap();
        old.mkdir_p("/usr/share/man/en", &Actor::root()).unwrap();
        old.write("/usr/share/man/en/gone.md", b"stale", &Actor::root())
            .unwrap();
        let mut new = Vfs::new();
        let r = Actor::root();
        new.mkdir_p("/games/snake", &r).unwrap();
        new.write("/games/snake/snake.bas", b"v2", &r).unwrap();
        new.chmod("/games/snake/snake.bas", 0o755, &r).unwrap();
        new.mkdir_p("/games/vi-quest/levels", &r).unwrap();
        new.write("/games/vi-quest/levels/01.toml", b"x", &r).unwrap();
        new.mkdir_p("/usr/share/man/en", &r).unwrap();
        new.write("/usr/share/man/en/vi.md", b"# vi", &r).unwrap();
        new.mkdir_p("/etc", &r).unwrap();
        new.write("/etc/motd", b"hello", &r).unwrap();
        new.mkdir_p("/home/kid", &r).unwrap();
        new.write("/home/kid/welcome.txt", b"factory welcome", &r).unwrap();
        old.refresh_from_factory(&new).unwrap();
        assert_eq!(old.read("/games/snake/snake.bas", &kid()).unwrap(), b"v2");
        assert_eq!(old.stat("/games/snake/snake.bas").unwrap().mode, 0o755);
        assert!(old.exists("/games/vi-quest/levels/01.toml"));
        assert!(old.exists("/games/frombob/cart.toml"), "parent-installed game kept");
        assert!(old.exists("/games/readme"), "not a factory game folder: kept");
        assert!(!old.exists("/usr/share/man/en/gone.md"));
        assert!(old.exists("/usr/share/man/en/vi.md"));
        assert_eq!(old.read("/etc/motd", &kid()).unwrap(), b"hello");
        assert_eq!(old.read("/home/kid/mine.txt", &kid()).unwrap(), b"keep");
        assert!(!old.exists("/home/kid/welcome.txt"), "home is the kid's");
        assert_eq!(old.stat("/games/vi-quest").unwrap().owner, "root");
    }

    #[test]
    fn walk_and_sizes() {
        let mut v = drive();
        v.write("/home/kid/x", b"12345", &kid()).unwrap();
        v.mkdir("/home/kid/d", &kid()).unwrap();
        v.write("/home/kid/d/y", b"123", &kid()).unwrap();
        assert_eq!(v.size_of("/home/kid").unwrap(), 8);
        let mut seen = Vec::new();
        v.walk_tree("/home/kid", &mut |p, _s, d| seen.push((p.to_string(), d)))
            .unwrap();
        assert_eq!(
            seen,
            vec![
                ("/home/kid".to_string(), 0),
                ("/home/kid/d".to_string(), 1),
                ("/home/kid/d/y".to_string(), 2),
                ("/home/kid/x".to_string(), 1)
            ]
        );
    }
}
