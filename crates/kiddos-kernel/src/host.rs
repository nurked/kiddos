//! The only bridge between the fake machine and the real one.

use kiddos_i18n::Lang;
use parking_lot::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

/// Things the kernel asks the host to do. The host may ignore any of them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HostRequest {
    /// Turn the machine off (parent mode `shutdown`).
    Shutdown,
    /// Restart the machine (reload the drive, re-run boot).
    Reboot,
    ExitFullscreen,
    EnterFullscreen,
    Crt(bool),
    Font(String),
    /// Reset the drive to the factory image on next boot.
    ResetDrive,
}

/// Per-machine settings the kernel owns and the host persists.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MachineConfig {
    pub lang: Lang,
    pub kid_name: String,
    pub crt: bool,
    pub font: String,
    pub hostname: String,
}

impl Default for MachineConfig {
    fn default() -> Self {
        MachineConfig {
            lang: Lang::En,
            kid_name: String::new(),
            crt: true,
            font: "cpc".into(),
            hostname: "kiddos".into(),
        }
    }
}

pub trait HostCaps: Send + Sync {
    /// Milliseconds since boot.
    fn now_ms(&self) -> u64;
    /// Unix time in seconds.
    fn unix_time(&self) -> u64;
    /// Block the calling thread. Headless hosts may use a virtual clock.
    fn sleep_ms(&self, ms: u64);
    fn speak(&self, text: &str, lang: Lang);
    fn beep(&self, freq: u32, ms: u32);
    fn request(&self, r: HostRequest);
    fn config_changed(&self, cfg: &MachineConfig);
    /// Append a line to the parent-visible log.
    fn log(&self, line: &str);
    /// Parent password check. `None` means no password has been set.
    fn verify_parent_password(&self, password: &str) -> Option<bool>;
    fn set_parent_password(&self, password: &str) -> Result<(), String>;
    /// Lines of the parent log (most recent last).
    fn read_log(&self, max_lines: usize) -> Vec<String>;
    /// Local time zone offset from UTC, for `date` and `ls -l`.
    fn tz_offset_secs(&self) -> i32 {
        0
    }
    /// `.kdc` files a parent dropped into the host's cartridge folder.
    fn list_cart_files(&self) -> Vec<String> {
        Vec::new()
    }
    fn read_cart_file(&self, _name: &str) -> Result<Vec<u8>, String> {
        Err("this machine has no cartridge folder".into())
    }
    /// Write a `.kdc` into the cartridge folder; returns where it went.
    fn write_cart_file(&self, _name: &str, _data: &[u8]) -> Result<String, String> {
        Err("this machine has no cartridge folder".into())
    }
    /// Where parents put cartridges, for messages.
    fn cart_folder_hint(&self) -> String {
        "the cartridge folder".into()
    }
    /// Installed toolchain packs: (name, description from pack.toml).
    fn list_packs(&self) -> Vec<(String, String)> {
        Vec::new()
    }
    /// Unpack a `.kdp` from the cartridge folder into the packs folder.
    /// Returns a one-line summary.
    fn install_pack(&self, _file: &str) -> Result<String, String> {
        Err("this machine cannot install packs".into())
    }
    fn remove_pack(&self, _name: &str) -> Result<(), String> {
        Err("this machine cannot remove packs".into())
    }
    /// Is a wasm32 C compiler available? `Err` carries the explanation.
    fn c_compiler_available(&self) -> Result<(), String> {
        Err("this machine has no C compiler. A parent can add the C pack (see docs).".into())
    }
    /// Compile the given source files (name, bytes; `kiddos.h` included)
    /// into one wasm module. `Err` carries the compiler's own output.
    fn compile_c(&self, _files: &[(String, Vec<u8>)]) -> Result<Vec<u8>, String> {
        Err("no C compiler".into())
    }
    /// Is TinyGo available? `Err` carries the explanation.
    fn go_compiler_available(&self) -> Result<(), String> {
        Err("this machine has no Go compiler. A parent can add the Go pack (see docs).".into())
    }
    /// Compile Go sources (name, bytes) plus the `kiddos` package files
    /// (path under `kiddos/`, bytes) into one wasm module.
    fn compile_go(&self, _files: &[(String, Vec<u8>)], _pkg: &[(String, Vec<u8>)]) -> Result<Vec<u8>, String> {
        Err("no Go compiler".into())
    }
}

/// A host that does nothing real. Used by the headless tool and tests: time
/// is virtual (sleeps advance it instantly), speech and requests are
/// recorded so tests can assert on them.
pub struct NullHost {
    start: Instant,
    virtual_ms: AtomicU64,
    pub spoken: Mutex<Vec<String>>,
    pub beeps: Mutex<Vec<(u32, u32)>>,
    pub requests: Mutex<Vec<HostRequest>>,
    pub log_lines: Mutex<Vec<String>>,
    pub parent_password: Mutex<Option<String>>,
    pub config: Mutex<Option<MachineConfig>>,
    pub fixed_unix_time: Option<u64>,
    pub cart_files: Mutex<std::collections::BTreeMap<String, Vec<u8>>>,
}

impl Default for NullHost {
    fn default() -> Self {
        NullHost::new()
    }
}

impl NullHost {
    pub fn new() -> NullHost {
        NullHost {
            start: Instant::now(),
            virtual_ms: AtomicU64::new(0),
            spoken: Mutex::new(Vec::new()),
            beeps: Mutex::new(Vec::new()),
            requests: Mutex::new(Vec::new()),
            log_lines: Mutex::new(Vec::new()),
            parent_password: Mutex::new(None),
            config: Mutex::new(None),
            fixed_unix_time: None,
            cart_files: Mutex::new(std::collections::BTreeMap::new()),
        }
    }

    /// A host whose clock is frozen (for deterministic screen diffs).
    pub fn frozen(unix_time: u64) -> NullHost {
        NullHost {
            fixed_unix_time: Some(unix_time),
            ..NullHost::new()
        }
    }
}

impl HostCaps for NullHost {
    fn now_ms(&self) -> u64 {
        self.start.elapsed().as_millis() as u64 + self.virtual_ms.load(Ordering::Relaxed)
    }
    fn unix_time(&self) -> u64 {
        self.fixed_unix_time.unwrap_or_else(|| {
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0)
        })
    }
    fn sleep_ms(&self, ms: u64) {
        self.virtual_ms.fetch_add(ms, Ordering::Relaxed);
        std::thread::yield_now();
    }
    fn speak(&self, text: &str, _lang: Lang) {
        self.spoken.lock().push(text.to_string());
    }
    fn beep(&self, freq: u32, ms: u32) {
        self.beeps.lock().push((freq, ms));
    }
    fn request(&self, r: HostRequest) {
        self.requests.lock().push(r);
    }
    fn config_changed(&self, cfg: &MachineConfig) {
        *self.config.lock() = Some(cfg.clone());
    }
    fn log(&self, line: &str) {
        self.log_lines.lock().push(line.to_string());
    }
    fn verify_parent_password(&self, password: &str) -> Option<bool> {
        self.parent_password.lock().as_ref().map(|p| p == password)
    }
    fn set_parent_password(&self, password: &str) -> Result<(), String> {
        *self.parent_password.lock() = Some(password.to_string());
        Ok(())
    }
    fn read_log(&self, max_lines: usize) -> Vec<String> {
        let l = self.log_lines.lock();
        let start = l.len().saturating_sub(max_lines);
        l[start..].to_vec()
    }
    fn list_cart_files(&self) -> Vec<String> {
        self.cart_files.lock().keys().cloned().collect()
    }
    fn read_cart_file(&self, name: &str) -> Result<Vec<u8>, String> {
        self.cart_files
            .lock()
            .get(name)
            .cloned()
            .ok_or_else(|| format!("no {name} in the cartridge folder"))
    }
    fn write_cart_file(&self, name: &str, data: &[u8]) -> Result<String, String> {
        self.cart_files.lock().insert(name.to_string(), data.to_vec());
        Ok(format!("carts/{name}"))
    }
    fn cart_folder_hint(&self) -> String {
        "carts/".into()
    }
}
