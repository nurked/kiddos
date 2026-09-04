//! A process: the view of the machine a running command gets.

use crate::fs::Fs;
use crate::kernel::{Child, Kernel, Pid, ProcState, Spawn, SpawnError};
use crate::stream::{Input, Output};
use crate::{Console, Interrupted, Key, KID_HOME, ROOT_HOME};
use kiddos_i18n::Lang;
use kiddos_vfs::{Actor, VfsError};
use parking_lot::Mutex;
use std::collections::BTreeMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, OnceLock, Weak};

/// What a process is allowed to do beyond reading and writing its files.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapSet {
    pub speak: bool,
    pub sound: bool,
    /// If set, filesystem access is limited to these prefixes.
    pub jail: Option<Vec<String>>,
}

impl Default for CapSet {
    fn default() -> Self {
        CapSet {
            speak: true,
            sound: true,
            jail: None,
        }
    }
}

pub struct Proc {
    pub pid: Pid,
    pub ppid: Pid,
    pub name: String,
    pub argv: Vec<String>,
    pub user: String,
    pub caps: CapSet,
    pub interruptible: bool,
    kernel: Arc<Kernel>,
    cwd: Mutex<String>,
    env: Mutex<BTreeMap<String, String>>,
    stdin: Input,
    stdin_buf: Mutex<Vec<u8>>,
    stdout: Output,
    stderr: Output,
    killed: Arc<AtomicBool>,
    self_weak: OnceLock<Weak<Proc>>,
}

impl Proc {
    pub(crate) fn new(kernel: Arc<Kernel>, pid: Pid, argv: Vec<String>, s: Spawn, killed: Arc<AtomicBool>) -> Proc {
        Proc {
            pid,
            ppid: s.ppid,
            name: argv[0].clone(),
            argv,
            user: s.user,
            caps: s.caps,
            interruptible: s.interruptible,
            kernel,
            cwd: Mutex::new(s.cwd),
            env: Mutex::new(s.env),
            stdin: s.stdin,
            stdin_buf: Mutex::new(Vec::new()),
            stdout: s.stdout,
            stderr: s.stderr,
            killed,
            self_weak: OnceLock::new(),
        }
    }

    pub(crate) fn set_self(&self, weak: Weak<Proc>) {
        let _ = self.self_weak.set(weak);
    }

    /// A shared handle to this process, for wrappers that must own one
    /// (the BASIC console). Panics only if called on a Proc the kernel did
    /// not spawn.
    pub fn arc(&self) -> Arc<Proc> {
        self.self_weak.get().and_then(|w| w.upgrade()).expect("proc handle")
    }

    pub fn kernel(&self) -> &Arc<Kernel> {
        &self.kernel
    }

    pub fn actor(&self) -> Actor {
        Actor::user(&self.user)
    }

    pub fn is_root(&self) -> bool {
        self.user == "root"
    }

    pub fn home(&self) -> String {
        if self.is_root() {
            ROOT_HOME.to_string()
        } else {
            KID_HOME.to_string()
        }
    }

    pub fn cwd(&self) -> String {
        self.cwd.lock().clone()
    }

    pub fn set_cwd(&self, p: &str) {
        *self.cwd.lock() = p.to_string();
    }

    pub fn env_get(&self, k: &str) -> Option<String> {
        self.env.lock().get(k).cloned()
    }

    pub fn env_set(&self, k: &str, v: &str) {
        self.env.lock().insert(k.to_string(), v.to_string());
    }

    pub fn env_unset(&self, k: &str) {
        self.env.lock().remove(k);
    }

    pub fn env_all(&self) -> BTreeMap<String, String> {
        self.env.lock().clone()
    }

    pub fn lang(&self) -> Lang {
        self.kernel.lang()
    }

    /// Translate a UI string in the machine's current language.
    pub fn t(&self, key: &str, args: &[(&str, &str)]) -> String {
        kiddos_i18n::t(self.lang(), key, args)
    }

    /// The jailed, user-aware filesystem view.
    pub fn fs(&self) -> Fs<'_> {
        Fs { proc: self }
    }

    pub fn spawn(&self, s: Spawn) -> Result<Child, SpawnError> {
        self.kernel.spawn(s)
    }

    /// Run `argv` as a child with inherited streams (tty) and wait.
    pub fn run_and_wait(&self, argv: Vec<String>) -> Result<i32, SpawnError> {
        let child = self.spawn(Spawn::child_of(self, argv))?;
        Ok(child.wait())
    }

    /// See [`Kernel::take_key_if`].
    pub fn take_key_if(&self, pred: impl Fn(&Key) -> bool) -> Option<Key> {
        self.kernel.take_key_if(pred)
    }

    pub fn killed(&self) -> bool {
        self.killed.load(Ordering::SeqCst) || self.kernel.shutting_down()
    }

    pub fn kill(&self) {
        self.killed.store(true, Ordering::SeqCst);
    }

    /// Bail out if we were told to stop.
    pub fn check(&self) -> Result<(), Interrupted> {
        if self.killed() {
            Err(Interrupted)
        } else {
            Ok(())
        }
    }

    /// Turn a filesystem error into what the machine says about it.
    pub fn explain(&self, e: &VfsError) -> String {
        let (key, path) = match e {
            VfsError::NotFound(p) => ("not-found", p),
            VfsError::IsADir(p) => ("is-dir", p),
            VfsError::NotADir(p) => ("not-dir", p),
            VfsError::Permission(p) | VfsError::NotPermitted(p) => ("permission-denied", p),
            VfsError::Exists(p) => ("exists", p),
            VfsError::NotEmpty(p) => ("not-empty", p),
            _ => return e.to_string(),
        };
        let shown = kiddos_vfs::path::tildify(path, &self.home());
        self.t(key, &[("path", &shown)])
    }

    /// Print `name: explanation` to stderr.
    pub fn complain(&self, e: &VfsError) {
        self.eprint(&format!("{}: {}\n", self.name, self.explain(e)));
    }

    pub fn println(&self, s: &str) {
        self.print(s);
        self.print("\n");
    }

    pub fn eprintln(&self, s: &str) {
        self.eprint(s);
        self.eprint("\n");
    }

    fn write_to(&self, out: &Output, data: &[u8]) {
        match out {
            Output::Tty => {
                let s = String::from_utf8_lossy(data);
                self.kernel.screen.lock().write_str(&s);
            }
            Output::Null => {}
            Output::Pipe(w) => {
                if !w.write(data, &|| self.killed()) {
                    // reader went away: behave like SIGPIPE
                    self.kill();
                }
            }
            Output::File { path } => {
                let r = self.kernel.vfs.lock().append(path, data, &self.actor());
                if let Err(e) = r {
                    let msg = format!("{}: {}\n", self.name, self.explain(&e));
                    self.kernel.screen.lock().write_str(&msg);
                    self.kill();
                }
            }
            Output::Speaker(buf) => {
                let mut b = buf.lock();
                b.push_str(&String::from_utf8_lossy(data));
                while let Some(i) = b.find('\n') {
                    let line: String = b.drain(..=i).collect();
                    let line = line.trim();
                    if !line.is_empty() {
                        self.speak(line);
                    }
                }
            }
        }
    }

    pub fn write_out(&self, data: &[u8]) {
        self.write_to(&self.stdout, data);
    }

    pub fn write_err(&self, data: &[u8]) {
        self.write_to(&self.stderr, data);
    }

    /// Called by the kernel when the process ends: flush the speaker.
    pub(crate) fn close(&self) {
        if let Output::Speaker(buf) = &self.stdout {
            let rest = std::mem::take(&mut *buf.lock());
            if !rest.trim().is_empty() {
                self.speak(rest.trim());
            }
        }
    }

    /// Read everything from stdin (until EOF or, on a tty, Ctrl-D).
    pub fn read_stdin_all(&self) -> Result<Vec<u8>, Interrupted> {
        let mut out = Vec::new();
        while let Some(line) = self.read_stdin_line()? {
            out.extend_from_slice(line.as_bytes());
            out.push(b'\n');
        }
        Ok(out)
    }

    /// One line from stdin without the newline. `None` at EOF.
    pub fn read_stdin_line(&self) -> Result<Option<String>, Interrupted> {
        match &self.stdin {
            Input::Tty => self.readline_tty(""),
            Input::Null => Ok(None),
            Input::Bytes(m) => {
                let mut g = m.lock();
                let (data, pos) = &mut *g;
                if *pos >= data.len() {
                    return Ok(None);
                }
                let rest = &data[*pos..];
                let end = rest.iter().position(|b| *b == b'\n').unwrap_or(rest.len());
                let line = String::from_utf8_lossy(&rest[..end]).to_string();
                *pos += end + 1;
                Ok(Some(line))
            }
            Input::Pipe(r) => {
                let mut buf = self.stdin_buf.lock();
                loop {
                    if let Some(i) = buf.iter().position(|b| *b == b'\n') {
                        let line: Vec<u8> = buf.drain(..=i).collect();
                        return Ok(Some(String::from_utf8_lossy(&line[..line.len() - 1]).to_string()));
                    }
                    let chunk = r.read(4096, &|| self.killed());
                    if chunk.is_empty() {
                        self.check()?;
                        if buf.is_empty() {
                            return Ok(None);
                        }
                        let line = std::mem::take(&mut *buf);
                        return Ok(Some(String::from_utf8_lossy(&line).to_string()));
                    }
                    buf.extend(chunk);
                }
            }
        }
    }

    /// Canonical-mode line input straight from the keyboard with echo.
    /// Backspace works, Ctrl-D on an empty line is EOF.
    fn readline_tty(&self, prompt: &str) -> Result<Option<String>, Interrupted> {
        self.write_to(&Output::Tty, prompt.as_bytes());
        let mut line = String::new();
        loop {
            let k = self.readkey()?;
            match k {
                Key::Enter => {
                    self.write_to(&Output::Tty, b"\n");
                    return Ok(Some(line));
                }
                Key::Backspace => {
                    if line.pop().is_some() {
                        self.write_to(&Output::Tty, b"\x08 \x08");
                    }
                }
                Key::Ctrl('d') if line.is_empty() => {
                    self.write_to(&Output::Tty, b"\n");
                    return Ok(None);
                }
                Key::Ctrl('u') => {
                    let n = line.chars().count();
                    line.clear();
                    for _ in 0..n {
                        self.write_to(&Output::Tty, b"\x08 \x08");
                    }
                }
                Key::Char(c) => {
                    line.push(c);
                    let mut b = [0u8; 4];
                    self.write_to(&Output::Tty, c.encode_utf8(&mut b).as_bytes());
                }
                _ => {}
            }
        }
    }

    /// Read a line with the echo hidden (passwords).
    pub fn read_secret(&self, prompt: &str) -> Result<Option<String>, Interrupted> {
        self.write_to(&Output::Tty, prompt.as_bytes());
        let mut line = String::new();
        loop {
            match self.readkey()? {
                Key::Enter => {
                    self.write_to(&Output::Tty, b"\n");
                    return Ok(Some(line));
                }
                Key::Backspace => {
                    line.pop();
                }
                Key::Ctrl('d') if line.is_empty() => {
                    self.write_to(&Output::Tty, b"\n");
                    return Ok(None);
                }
                Key::Char(c) => line.push(c),
                _ => {}
            }
        }
    }
}

impl Console for Proc {
    fn size(&self) -> (u16, u16) {
        self.kernel.screen.lock().size()
    }
    fn put(&self, x: u16, y: u16, ch: char, fg: u8, bg: u8) {
        self.kernel.screen.lock().put(x, y, ch, fg, bg);
    }
    fn print(&self, s: &str) {
        self.write_out(s.as_bytes());
    }
    fn eprint(&self, s: &str) {
        self.write_err(s.as_bytes());
    }
    fn cursor(&self, x: u16, y: u16) {
        self.kernel.screen.lock().set_cursor(x, y);
    }
    fn cursor_pos(&self) -> (u16, u16) {
        self.kernel.screen.lock().cursor()
    }
    fn cursor_show(&self, visible: bool) {
        self.kernel.screen.lock().show_cursor(visible);
    }
    fn clear(&self, bg: u8) {
        self.kernel.screen.lock().clear(bg);
    }
    fn set_color(&self, fg: u8, bg: u8) {
        self.kernel.screen.lock().set_colors(fg, bg);
    }
    fn getkey(&self) -> Option<Key> {
        self.kernel.poll_key()
    }
    fn readkey(&self) -> Result<Key, Interrupted> {
        self.check()?;
        self.kernel.set_state(self.pid, ProcState::Waiting);
        let r = self.kernel.wait_key(&self.killed);
        self.kernel.set_state(self.pid, ProcState::Running);
        r
    }
    fn readline(&self, prompt: &str) -> Result<Option<String>, Interrupted> {
        if self.stdin.is_tty() {
            self.readline_tty(prompt)
        } else {
            self.read_stdin_line()
        }
    }
    fn sleep(&self, ms: u64) -> Result<(), Interrupted> {
        let mut left = ms;
        self.kernel.set_state(self.pid, ProcState::Waiting);
        while left > 0 {
            if self.killed() {
                self.kernel.set_state(self.pid, ProcState::Running);
                return Err(Interrupted);
            }
            let step = left.min(20);
            self.kernel.host().sleep_ms(step);
            left -= step;
        }
        self.kernel.set_state(self.pid, ProcState::Running);
        self.check()
    }
    fn tick(&self) -> u64 {
        self.kernel.host().now_ms()
    }
    fn beep(&self, freq: u32, ms: u32) {
        if self.caps.sound {
            self.kernel.beep(freq, ms);
        }
    }
    fn speak(&self, text: &str) -> bool {
        if !self.caps.speak {
            return false;
        }
        self.kernel.speak(text)
    }
    fn interrupted(&self) -> bool {
        self.killed()
    }
    fn stdout_is_tty(&self) -> bool {
        self.stdout.is_tty()
    }
    fn stdin_is_tty(&self) -> bool {
        self.stdin.is_tty()
    }
}
