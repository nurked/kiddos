//! Process table, spawning, keys, events, machine state.

use crate::host::{HostCaps, HostRequest, MachineConfig};
use crate::proc::{CapSet, Proc};
use crate::registry::{Command, Topic};
use crate::stream::{Input, Output};
use crate::{CmdResult, Console, ExitCode, Interrupted, Key, Screen, Vfs, VfsError, KID_HOME, KID_USER};
use kiddos_i18n::Lang;
use kiddos_vfs::{Actor, Kind, X};
use parking_lot::{Condvar, Mutex, RwLock};
use std::any::{Any, TypeId};
use std::collections::{BTreeMap, HashMap, VecDeque};
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicUsize, Ordering};
use std::sync::Arc;
use std::thread::JoinHandle;

pub type Pid = u32;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProcState {
    Running,
    /// Blocked waiting for a key or sleeping.
    Waiting,
}

#[derive(Debug, Clone)]
pub struct ProcInfo {
    pub pid: Pid,
    pub ppid: Pid,
    pub name: String,
    pub user: String,
    pub started_ms: u64,
    pub state: ProcState,
}

struct ProcEntry {
    info: ProcInfo,
    killed: Arc<AtomicBool>,
    interruptible: bool,
}

/// Things other parts of the machine (the tutor) can listen to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    Boot,
    /// The shell ran a line. `status` is `$?`.
    CommandRun {
        line: String,
        status: ExitCode,
        cwd: String,
    },
    ProcessExit {
        name: String,
        status: ExitCode,
    },
    LangChanged(Lang),
}

/// Everything needed to start a process.
pub struct Spawn {
    pub argv: Vec<String>,
    pub cwd: String,
    pub env: BTreeMap<String, String>,
    pub user: String,
    pub stdin: Input,
    pub stdout: Output,
    pub stderr: Output,
    pub caps: CapSet,
    pub ppid: Pid,
    /// Ctrl-C kills it. False only for the login shell.
    pub interruptible: bool,
}

impl Spawn {
    pub fn new(argv: Vec<String>) -> Spawn {
        let mut env = BTreeMap::new();
        env.insert("HOME".into(), KID_HOME.to_string());
        env.insert("USER".into(), KID_USER.to_string());
        env.insert("PATH".into(), "/bin:~/bin".to_string());
        env.insert("SHELL".into(), "/bin/ksh".to_string());
        Spawn {
            argv,
            cwd: KID_HOME.to_string(),
            env,
            user: KID_USER.to_string(),
            stdin: Input::Tty,
            stdout: Output::Tty,
            stderr: Output::Tty,
            caps: CapSet::default(),
            ppid: 0,
            interruptible: true,
        }
    }

    /// Inherit cwd, env, user and caps from `parent`; streams default to tty.
    pub fn child_of(parent: &Proc, argv: Vec<String>) -> Spawn {
        Spawn {
            argv,
            cwd: parent.cwd(),
            env: parent.env_all(),
            user: parent.user.clone(),
            stdin: Input::Tty,
            stdout: Output::Tty,
            stderr: Output::Tty,
            caps: parent.caps.clone(),
            ppid: parent.pid,
            interruptible: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SpawnError {
    NotFound(String),
    NotExecutable(String),
    IsDir(String),
    ParentOnly(String),
    Vfs(VfsError),
}

impl std::fmt::Display for SpawnError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SpawnError::NotFound(n) => write!(f, "{n}: command not found"),
            SpawnError::NotExecutable(p) => write!(f, "{p}: Permission denied (not executable)"),
            SpawnError::IsDir(p) => write!(f, "{p}: Is a directory"),
            SpawnError::ParentOnly(n) => write!(f, "{n}: parent only"),
            SpawnError::Vfs(e) => write!(f, "{e}"),
        }
    }
}

impl From<VfsError> for SpawnError {
    fn from(e: VfsError) -> Self {
        match e {
            VfsError::NotFound(p) => SpawnError::NotFound(p),
            VfsError::IsADir(p) => SpawnError::IsDir(p),
            e => SpawnError::Vfs(e),
        }
    }
}

/// A running child. `wait` joins it and returns the exit code.
pub struct Child {
    pub pid: Pid,
    handle: Mutex<Option<JoinHandle<ExitCode>>>,
}

impl Child {
    pub fn wait(&self) -> ExitCode {
        let h = self.handle.lock().take();
        match h {
            Some(h) => h.join().unwrap_or(1),
            None => 0,
        }
    }
}

type Listener = Box<dyn Fn(&Event) + Send + Sync>;

pub struct Kernel {
    pub screen: Mutex<Screen>,
    pub vfs: Mutex<Vfs>,
    host: Arc<dyn HostCaps>,
    keys: Mutex<VecDeque<Key>>,
    key_cv: Condvar,
    key_waiters: AtomicUsize,
    procs: Mutex<BTreeMap<Pid, ProcEntry>>,
    next_pid: AtomicU32,
    registry: RwLock<BTreeMap<String, Command>>,
    config: Mutex<MachineConfig>,
    listeners: Mutex<Vec<Listener>>,
    last_speak_ms: Mutex<Option<u64>>,
    last_beep_ms: Mutex<u64>,
    shutting_down: AtomicBool,
    /// The shell sets this while a command runs so `ps` and Ctrl-C know.
    pub kid_name: Mutex<String>,
    extensions: Mutex<HashMap<TypeId, Arc<dyn Any + Send + Sync>>>,
}

const SPEAK_MIN_GAP_MS: u64 = 400;
const SPEAK_MAX_CHARS: usize = 400;
const BEEP_MAX_MS: u32 = 2000;

impl Kernel {
    pub fn new(vfs: Vfs, host: Arc<dyn HostCaps>, config: MachineConfig, cols: u16, rows: u16) -> Arc<Kernel> {
        let kid_name = config.kid_name.clone();
        let k = Arc::new(Kernel {
            screen: Mutex::new(Screen::new(cols, rows)),
            vfs: Mutex::new(vfs),
            host,
            keys: Mutex::new(VecDeque::new()),
            key_cv: Condvar::new(),
            key_waiters: AtomicUsize::new(0),
            procs: Mutex::new(BTreeMap::new()),
            next_pid: AtomicU32::new(1),
            registry: RwLock::new(BTreeMap::new()),
            config: Mutex::new(config),
            listeners: Mutex::new(Vec::new()),
            last_speak_ms: Mutex::new(None),
            last_beep_ms: Mutex::new(0),
            shutting_down: AtomicBool::new(false),
            kid_name: Mutex::new(kid_name),
            extensions: Mutex::new(HashMap::new()),
        });
        {
            let host = k.host.clone();
            let unix = move || host.unix_time();
            k.vfs.lock().set_clock(Box::new(unix));
        }
        k.register(Command::new("init", init, "the first process", Topic::Hidden));
        k
    }

    pub fn host(&self) -> &dyn HostCaps {
        &*self.host
    }

    // ---- registry ------------------------------------------------------

    pub fn register(&self, cmd: Command) {
        self.registry.write().insert(cmd.name.to_string(), cmd);
    }

    pub fn unregister(&self, name: &str) {
        self.registry.write().remove(name);
    }

    pub fn command(&self, name: &str) -> Option<Command> {
        self.registry.read().get(name).cloned()
    }

    pub fn commands(&self) -> Vec<Command> {
        self.registry.read().values().cloned().collect()
    }

    pub fn command_names(&self) -> Vec<String> {
        self.registry.read().keys().cloned().collect()
    }

    /// Make `/bin` mirror the registry so `ls /bin` teaches where commands
    /// live. Hidden commands are not listed.
    pub fn sync_bin(&self) {
        let cmds = self.commands();
        let mut vfs = self.vfs.lock();
        let root = Actor::root();
        let _ = vfs.mkdir_p("/bin", &root);
        let existing: Vec<String> = vfs
            .readdir("/bin", &root)
            .map(|v| v.into_iter().map(|s| s.name).collect())
            .unwrap_or_default();
        for name in &existing {
            if !cmds.iter().any(|c| c.name == name && c.topic != Topic::Hidden) {
                let _ = vfs.unlink(&format!("/bin/{name}"), &root);
            }
        }
        for c in cmds.iter().filter(|c| c.topic != Topic::Hidden) {
            let p = format!("/bin/{}", c.name);
            if !existing.iter().any(|n| n == c.name) {
                let _ = vfs.write(&p, c.summary.as_bytes(), &root);
                let _ = vfs.chmod(&p, 0o755, &root);
            }
        }
    }

    // ---- keys ----------------------------------------------------------

    /// Feed one key from the host. Ctrl-C interrupts the foreground.
    pub fn push_key(&self, key: Key) {
        if key == Key::Ctrl('c') && self.interrupt_foreground() > 0 {
            return;
        }
        self.keys.lock().push_back(key);
        self.key_cv.notify_all();
    }

    /// Feed text as key presses (`\n` becomes Enter).
    pub fn push_text(&self, text: &str) {
        for c in text.chars() {
            match c {
                '\n' => self.push_key(Key::Enter),
                '\t' => self.push_key(Key::Tab),
                c => self.push_key(Key::Char(c)),
            }
        }
    }

    pub fn keys_pending(&self) -> usize {
        self.keys.lock().len()
    }

    /// True when no keys are queued and some process is blocked on input.
    pub fn is_idle(&self) -> bool {
        self.keys.lock().is_empty() && self.key_waiters.load(Ordering::SeqCst) > 0
    }

    pub(crate) fn poll_key(&self) -> Option<Key> {
        self.keys.lock().pop_front()
    }

    /// Remove and return the first queued key matching `pred`, leaving the
    /// rest in order. Full-screen programs use it to notice Ctrl-C while
    /// busy without eating the keys a game will want.
    pub fn take_key_if(&self, pred: impl Fn(&Key) -> bool) -> Option<Key> {
        let mut q = self.keys.lock();
        let i = q.iter().position(pred)?;
        q.remove(i)
    }

    pub(crate) fn wait_key(&self, killed: &AtomicBool) -> Result<Key, Interrupted> {
        let mut q = self.keys.lock();
        self.key_waiters.fetch_add(1, Ordering::SeqCst);
        let r = loop {
            if let Some(k) = q.pop_front() {
                break Ok(k);
            }
            if killed.load(Ordering::SeqCst) || self.shutting_down() {
                break Err(Interrupted);
            }
            self.key_cv.wait_for(&mut q, std::time::Duration::from_millis(50));
        };
        self.key_waiters.fetch_sub(1, Ordering::SeqCst);
        r
    }

    // ---- processes -----------------------------------------------------

    fn resolve(&self, argv: &[String], cwd: &str, user: &str) -> Result<(Command, Vec<String>), SpawnError> {
        let name = argv.first().cloned().unwrap_or_default();
        let actor = Actor::user(user);
        let home = if user == "root" { crate::ROOT_HOME } else { KID_HOME };
        let as_path = |p: &str| kiddos_vfs::normalize(cwd, &kiddos_vfs::path::expand_tilde(p, home));
        let mut path: Option<String> = None;
        if name.contains('/') {
            path = Some(as_path(&name));
        } else if let Some(cmd) = self.command(&name) {
            if cmd.parent_only && user != "root" {
                return Err(SpawnError::ParentOnly(name));
            }
            return Ok((cmd, argv.to_vec()));
        } else {
            let candidate = format!("{home}/bin/{name}");
            if self.vfs.lock().exists(&candidate) {
                path = Some(candidate);
            }
        }
        let Some(path) = path else {
            return Err(SpawnError::NotFound(name));
        };
        let (interp, first_line) = {
            let vfs = self.vfs.lock();
            let st = vfs.stat(&path)?;
            if st.kind == Kind::Dir {
                return Err(SpawnError::IsDir(path));
            }
            if !vfs.has_access(&path, &actor, X)? {
                return Err(SpawnError::NotExecutable(path));
            }
            let data = vfs.read(&path, &actor)?;
            let first = data.split(|b| *b == b'\n').next().unwrap_or(&[]);
            let first = String::from_utf8_lossy(first).to_string();
            let interp = first
                .strip_prefix("#!")
                .map(|s| s.trim())
                .and_then(|s| s.split_whitespace().next())
                .map(|s| kiddos_vfs::basename(s).to_string())
                .unwrap_or_else(|| "ksh".to_string());
            (interp, first)
        };
        let _ = first_line;
        let Some(cmd) = self.command(&interp) else {
            return Err(SpawnError::NotFound(interp));
        };
        let mut full = vec![interp, path];
        full.extend(argv.iter().skip(1).cloned());
        Ok((cmd, full))
    }

    /// Start a process. Resolution errors are returned synchronously so the
    /// shell can talk about them; the process itself runs on its own thread.
    pub fn spawn(self: &Arc<Self>, s: Spawn) -> Result<Child, SpawnError> {
        let (cmd, argv) = self.resolve(&s.argv, &s.cwd, &s.user)?;
        let pid = self.next_pid.fetch_add(1, Ordering::SeqCst);
        let killed = Arc::new(AtomicBool::new(false));
        let name = argv[0].clone();
        self.procs.lock().insert(
            pid,
            ProcEntry {
                info: ProcInfo {
                    pid,
                    ppid: s.ppid,
                    name: name.clone(),
                    user: s.user.clone(),
                    started_ms: self.host.now_ms(),
                    state: ProcState::Running,
                },
                killed: killed.clone(),
                interruptible: s.interruptible && !cmd.keep_alive,
            },
        );
        let proc = Arc::new(Proc::new(self.clone(), pid, argv, s, killed));
        proc.set_self(Arc::downgrade(&proc));
        let kernel = self.clone();
        let run = cmd.run;
        let handle = std::thread::Builder::new()
            .name(format!("kiddos:{name}"))
            .spawn(move || {
                let args: Vec<String> = proc.argv[1..].to_vec();
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| run(&proc, &args)));
                let status: ExitCode = match result {
                    Ok(Ok(code)) => code,
                    Ok(Err(Interrupted)) => 130,
                    Err(_) => {
                        proc.eprint("The program crashed. That happens. Try again!\n");
                        kernel.host.log(&format!("panic in {}", proc.name));
                        1
                    }
                };
                proc.close();
                kernel.procs.lock().remove(&pid);
                kernel.emit(Event::ProcessExit {
                    name: proc.name.clone(),
                    status,
                });
                status
            })
            .expect("spawn thread");
        Ok(Child {
            pid,
            handle: Mutex::new(Some(handle)),
        })
    }

    pub fn processes(&self) -> Vec<ProcInfo> {
        self.procs.lock().values().map(|e| e.info.clone()).collect()
    }

    pub(crate) fn set_state(&self, pid: Pid, state: ProcState) {
        if let Some(e) = self.procs.lock().get_mut(&pid) {
            e.info.state = state;
        }
    }

    /// Ask a process to stop. Returns false if there is no such process.
    pub fn kill(&self, pid: Pid) -> bool {
        match self.procs.lock().get(&pid) {
            Some(e) => {
                e.killed.store(true, Ordering::SeqCst);
                true
            }
            None => false,
        }
    }

    /// Ctrl-C: stop every interruptible process. Returns how many.
    pub fn interrupt_foreground(&self) -> usize {
        let procs = self.procs.lock();
        let mut n = 0;
        for e in procs.values() {
            if e.interruptible && !e.killed.load(Ordering::SeqCst) {
                e.killed.store(true, Ordering::SeqCst);
                n += 1;
            }
        }
        drop(procs);
        self.key_cv.notify_all();
        n
    }

    // ---- extensions ----------------------------------------------------

    /// Attach a subsystem (the tutor, later the cartridge manager) so that
    /// commands can find it by type.
    pub fn set_extension<T: Any + Send + Sync>(&self, v: Arc<T>) {
        self.extensions.lock().insert(TypeId::of::<T>(), v);
    }

    pub fn extension<T: Any + Send + Sync>(&self) -> Option<Arc<T>> {
        let map = self.extensions.lock();
        let any = map.get(&TypeId::of::<T>())?.clone();
        any.downcast::<T>().ok()
    }

    // ---- machine state -------------------------------------------------

    pub fn config(&self) -> MachineConfig {
        self.config.lock().clone()
    }

    pub fn update_config(&self, f: impl FnOnce(&mut MachineConfig)) {
        let cfg = {
            let mut c = self.config.lock();
            f(&mut c);
            c.clone()
        };
        *self.kid_name.lock() = cfg.kid_name.clone();
        self.host.config_changed(&cfg);
    }

    pub fn lang(&self) -> Lang {
        self.config.lock().lang
    }

    pub fn set_lang(&self, lang: Lang) {
        self.update_config(|c| c.lang = lang);
        self.emit(Event::LangChanged(lang));
    }

    pub fn t(&self, key: &str, args: &[(&str, &str)]) -> String {
        kiddos_i18n::t(self.lang(), key, args)
    }

    pub fn subscribe(&self, f: Listener) {
        self.listeners.lock().push(f);
    }

    pub fn emit(&self, e: Event) {
        let ls = self.listeners.lock();
        for l in ls.iter() {
            l(&e);
        }
    }

    /// Rate-limited speech. Returns false if throttled.
    pub fn speak(&self, text: &str) -> bool {
        let now = self.host.now_ms();
        let mut last = self.last_speak_ms.lock();
        if let Some(l) = *last {
            if now.saturating_sub(l) < SPEAK_MIN_GAP_MS {
                return false;
            }
        }
        *last = Some(now);
        drop(last);
        let text: String = text.chars().take(SPEAK_MAX_CHARS).collect();
        let text = text.trim();
        if text.is_empty() {
            return true;
        }
        self.host.speak(text, self.lang());
        true
    }

    pub fn beep(&self, freq: u32, ms: u32) {
        let now = self.host.now_ms();
        let mut last = self.last_beep_ms.lock();
        if now.saturating_sub(*last) < 20 {
            return;
        }
        *last = now;
        drop(last);
        self.host.beep(freq.clamp(40, 8000), ms.min(BEEP_MAX_MS));
    }

    pub fn request(&self, r: HostRequest) {
        self.host.request(r);
    }

    pub fn log(&self, line: &str) {
        self.host.log(line);
    }

    pub fn shutting_down(&self) -> bool {
        self.shutting_down.load(Ordering::SeqCst)
    }

    /// Stop everything. Processes blocked on input wake up with `Interrupted`.
    pub fn shutdown(&self) {
        self.shutting_down.store(true, Ordering::SeqCst);
        for e in self.procs.lock().values() {
            e.killed.store(true, Ordering::SeqCst);
        }
        self.key_cv.notify_all();
    }

    // ---- drive ---------------------------------------------------------

    pub fn vfs_changes(&self) -> u64 {
        self.vfs.lock().changes()
    }

    pub fn save_drive(&self, path: &std::path::Path) -> Result<(), VfsError> {
        self.vfs.lock().save(path)
    }

    pub fn screen_text(&self) -> String {
        self.screen.lock().text()
    }

    /// Start the machine: runs `init`, which prints the banner and runs the
    /// login shell. Returns the init child so the host can wait on it.
    pub fn boot(self: &Arc<Self>) -> Child {
        self.sync_bin();
        let root = Actor::root();
        {
            let mut vfs = self.vfs.lock();
            let _ = vfs.mkdir_p(KID_HOME, &root);
            let _ = vfs.chown_tree(KID_HOME, KID_USER);
            let _ = vfs.mkdir_p("/tmp", &root);
            let _ = vfs.chmod("/tmp", 0o777, &root);
        }
        self.emit(Event::Boot);
        let mut s = Spawn::new(vec!["init".into()]);
        s.interruptible = false;
        s.user = "root".into();
        s.cwd = "/".into();
        self.spawn(s).expect("init is registered")
    }
}

/// PID 1. Prints the banner, then runs the login shell forever.
fn init(p: &Proc, _args: &[String]) -> CmdResult {
    let k = p.kernel();
    let (cols, rows) = k.screen.lock().size();
    p.print(&format!(
        "\x1b[1;36mKidDOS\x1b[0m {}  ({cols} cols x {rows} rows)\n",
        crate::RELEASE
    ));
    if let Ok(motd) = p.fs().read_string("/etc/motd") {
        p.print(&motd);
        if !motd.ends_with('\n') {
            p.print("\n");
        }
    }
    p.print(&format!("{}\n\n", k.t("boot-hello", &[])));
    loop {
        if k.shutting_down() {
            return Ok(0);
        }
        let mut s = Spawn::new(vec!["ksh".into(), "-l".into()]);
        s.interruptible = false;
        s.ppid = p.pid;
        match p.spawn(s) {
            Ok(child) => {
                child.wait();
            }
            Err(e) => {
                p.eprint(&format!("init: cannot start shell: {e}\n"));
                return Ok(1);
            }
        }
        if k.shutting_down() {
            return Ok(0);
        }
    }
}
