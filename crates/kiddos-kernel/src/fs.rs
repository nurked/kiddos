//! The filesystem as one process sees it: paths relative to its cwd, `~`
//! expanded, permissions checked as its user, `/dev` wired up, and an
//! optional jail for cartridges.

use crate::proc::Proc;
use crate::Console;
use kiddos_vfs::{normalize, path::expand_tilde, Stat, VfsError};

pub struct Fs<'a> {
    pub(crate) proc: &'a Proc,
}

type R<T> = Result<T, VfsError>;

impl<'a> Fs<'a> {
    /// Absolute, normalized path for `p` as this process sees it.
    pub fn path(&self, p: &str) -> String {
        normalize(&self.proc.cwd(), &expand_tilde(p, &self.proc.home()))
    }

    fn jailed(&self, abs: &str) -> R<String> {
        if let Some(roots) = &self.proc.caps.jail {
            let ok = roots
                .iter()
                .any(|r| abs == r || abs.starts_with(&format!("{}/", r.trim_end_matches('/'))));
            if !ok && !abs.starts_with("/dev/") {
                return Err(VfsError::Permission(abs.to_string()));
            }
        }
        Ok(abs.to_string())
    }

    fn abs(&self, p: &str) -> R<String> {
        self.jailed(&self.path(p))
    }

    fn actor(&self) -> kiddos_vfs::Actor {
        self.proc.actor()
    }

    pub fn exists(&self, p: &str) -> bool {
        match self.abs(p) {
            Ok(a) => self.proc.kernel().vfs.lock().exists(&a),
            Err(_) => false,
        }
    }

    pub fn is_dir(&self, p: &str) -> bool {
        match self.abs(p) {
            Ok(a) => self.proc.kernel().vfs.lock().is_dir(&a),
            Err(_) => false,
        }
    }

    pub fn stat(&self, p: &str) -> R<Stat> {
        let a = self.abs(p)?;
        self.proc.kernel().vfs.lock().stat(&a)
    }

    pub fn lstat(&self, p: &str) -> R<Stat> {
        let a = self.abs(p)?;
        self.proc.kernel().vfs.lock().lstat(&a)
    }

    pub fn realpath(&self, p: &str) -> R<String> {
        let a = self.abs(p)?;
        self.proc.kernel().vfs.lock().realpath(&a)
    }

    pub fn readlink(&self, p: &str) -> R<String> {
        let a = self.abs(p)?;
        self.proc.kernel().vfs.lock().readlink(&a)
    }

    pub fn read(&self, p: &str) -> R<Vec<u8>> {
        let a = self.abs(p)?;
        match a.as_str() {
            "/dev/null" => return Ok(Vec::new()),
            "/dev/tty" | "/dev/speaker" => return Ok(Vec::new()),
            _ => {}
        }
        self.proc.kernel().vfs.lock().read(&a, &self.actor())
    }

    pub fn read_string(&self, p: &str) -> R<String> {
        Ok(String::from_utf8_lossy(&self.read(p)?).to_string())
    }

    /// Read a file line by line (no trailing newline on lines).
    pub fn read_lines(&self, p: &str) -> R<Vec<String>> {
        let s = self.read_string(p)?;
        Ok(s.lines().map(|l| l.to_string()).collect())
    }

    /// Handles `/dev` targets. Returns Ok(true) if the path was a device and
    /// the write has been fully handled.
    fn dev_write(&self, abs: &str, data: &[u8]) -> Option<R<()>> {
        match abs {
            "/dev/null" => Some(Ok(())),
            "/dev/tty" => {
                self.proc
                    .kernel()
                    .screen
                    .lock()
                    .write_str(&String::from_utf8_lossy(data));
                Some(Ok(()))
            }
            "/dev/speaker" => {
                let text = String::from_utf8_lossy(data);
                for line in text.lines() {
                    if !line.trim().is_empty() {
                        self.proc.speak(line.trim());
                    }
                }
                Some(Ok(()))
            }
            _ => None,
        }
    }

    pub fn write(&self, p: &str, data: &[u8]) -> R<()> {
        let a = self.abs(p)?;
        if let Some(r) = self.dev_write(&a, data) {
            return r;
        }
        self.proc.kernel().vfs.lock().write(&a, data, &self.actor())
    }

    pub fn append(&self, p: &str, data: &[u8]) -> R<()> {
        let a = self.abs(p)?;
        if let Some(r) = self.dev_write(&a, data) {
            return r;
        }
        self.proc.kernel().vfs.lock().append(&a, data, &self.actor())
    }

    /// Prepare a file for `>` (truncate/create) or `>>` (create if missing)
    /// and return the absolute path for an [`crate::Output::File`].
    pub fn open_for_write(&self, p: &str, append: bool) -> R<String> {
        let a = self.abs(p)?;
        if a.starts_with("/dev/") {
            return Ok(a);
        }
        let mut vfs = self.proc.kernel().vfs.lock();
        if append {
            if !vfs.exists(&a) {
                vfs.write(&a, &[], &self.actor())?;
            } else {
                vfs.stat(&a).and_then(|s| {
                    if s.is_dir() {
                        Err(VfsError::IsADir(a.clone()))
                    } else {
                        Ok(())
                    }
                })?;
            }
        } else {
            vfs.write(&a, &[], &self.actor())?;
        }
        Ok(a)
    }

    pub fn touch(&self, p: &str) -> R<()> {
        let a = self.abs(p)?;
        self.proc.kernel().vfs.lock().touch(&a, &self.actor())
    }

    pub fn mkdir(&self, p: &str) -> R<()> {
        let a = self.abs(p)?;
        self.proc.kernel().vfs.lock().mkdir(&a, &self.actor())
    }

    pub fn mkdir_p(&self, p: &str) -> R<()> {
        let a = self.abs(p)?;
        self.proc.kernel().vfs.lock().mkdir_p(&a, &self.actor())
    }

    pub fn readdir(&self, p: &str) -> R<Vec<Stat>> {
        let a = self.abs(p)?;
        self.proc.kernel().vfs.lock().readdir(&a, &self.actor())
    }

    pub fn unlink(&self, p: &str) -> R<()> {
        let a = self.abs(p)?;
        self.proc.kernel().vfs.lock().unlink(&a, &self.actor())
    }

    pub fn rmdir(&self, p: &str) -> R<()> {
        let a = self.abs(p)?;
        self.proc.kernel().vfs.lock().rmdir(&a, &self.actor())
    }

    pub fn remove_tree(&self, p: &str) -> R<()> {
        let a = self.abs(p)?;
        self.proc.kernel().vfs.lock().remove_tree(&a, &self.actor())
    }

    pub fn rename(&self, from: &str, to: &str) -> R<()> {
        let a = self.abs(from)?;
        let b = self.abs(to)?;
        self.proc.kernel().vfs.lock().rename(&a, &b, &self.actor())
    }

    pub fn symlink(&self, target: &str, link: &str) -> R<()> {
        let l = self.abs(link)?;
        self.proc.kernel().vfs.lock().symlink(target, &l, &self.actor())
    }

    pub fn chmod(&self, p: &str, mode: u16) -> R<()> {
        let a = self.abs(p)?;
        self.proc.kernel().vfs.lock().chmod(&a, mode, &self.actor())
    }

    pub fn size_of(&self, p: &str) -> R<u64> {
        let a = self.abs(p)?;
        self.proc.kernel().vfs.lock().size_of(&a)
    }

    pub fn used_bytes(&self) -> u64 {
        self.proc.kernel().vfs.lock().used_bytes()
    }

    /// Depth-first visit. The callback gets (absolute path, stat, depth).
    pub fn walk_tree(&self, p: &str, f: &mut dyn FnMut(&str, &Stat, usize)) -> R<()> {
        let a = self.abs(p)?;
        self.proc.kernel().vfs.lock().walk_tree(&a, f)
    }

    /// Expand a glob pattern (`*`, `?`, `[abc]`) against the VFS. Returns
    /// matches sorted, or an empty vec if none. Only the last component may
    /// contain wildcards in v1.
    pub fn glob(&self, pattern: &str) -> Vec<String> {
        let (dir, pat) = match pattern.rfind('/') {
            Some(i) => (&pattern[..i.max(1)], &pattern[i + 1..]),
            None => (".", pattern),
        };
        let dir = if pattern.starts_with('/') && dir.is_empty() {
            "/"
        } else {
            dir
        };
        let Ok(entries) = self.readdir(dir) else {
            return Vec::new();
        };
        let mut out: Vec<String> = entries
            .into_iter()
            .filter(|e| !e.name.starts_with('.') || pat.starts_with('.'))
            .filter(|e| glob_match(pat, &e.name))
            .map(|e| match pattern.rfind('/') {
                Some(i) => format!("{}/{}", &pattern[..i], e.name),
                None => e.name,
            })
            .collect();
        out.sort();
        out
    }
}

/// Minimal glob: `*`, `?`, `[set]`.
pub fn glob_match(pat: &str, name: &str) -> bool {
    let p: Vec<char> = pat.chars().collect();
    let n: Vec<char> = name.chars().collect();
    fn rec(p: &[char], n: &[char]) -> bool {
        match p.first() {
            None => n.is_empty(),
            Some('*') => (0..=n.len()).any(|i| rec(&p[1..], &n[i..])),
            Some('?') => !n.is_empty() && rec(&p[1..], &n[1..]),
            Some('[') => {
                let Some(end) = p.iter().position(|c| *c == ']') else {
                    return !n.is_empty() && n[0] == '[' && rec(&p[1..], &n[1..]);
                };
                let set = &p[1..end];
                let (neg, set) = if set.first() == Some(&'!') {
                    (true, &set[1..])
                } else {
                    (false, set)
                };
                if n.is_empty() {
                    return false;
                }
                let mut hit = false;
                let mut i = 0;
                while i < set.len() {
                    if i + 2 < set.len() && set[i + 1] == '-' {
                        if set[i] <= n[0] && n[0] <= set[i + 2] {
                            hit = true;
                        }
                        i += 3;
                    } else {
                        if set[i] == n[0] {
                            hit = true;
                        }
                        i += 1;
                    }
                }
                hit != neg && rec(&p[end + 1..], &n[1..])
            }
            Some(c) => !n.is_empty() && n[0] == *c && rec(&p[1..], &n[1..]),
        }
    }
    rec(&p, &n)
}

pub fn has_glob(s: &str) -> bool {
    s.contains('*') || s.contains('?') || s.contains('[')
}

#[cfg(test)]
mod tests {
    use super::glob_match;

    #[test]
    fn globs() {
        assert!(glob_match("*.txt", "a.txt"));
        assert!(!glob_match("*.txt", "a.md"));
        assert!(glob_match("a?c", "abc"));
        assert!(glob_match("[a-c]x", "bx"));
        assert!(!glob_match("[!a-c]x", "bx"));
        assert!(glob_match("*", ""));
        assert!(glob_match("snake*", "snake.bas"));
    }
}
