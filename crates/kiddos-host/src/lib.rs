//! The real machine. This crate is the only one that touches the host OS:
//! window and keys (winit), speech, sound, clock, the config directory,
//! the parent password and the log. Everything the kernel needs is behind
//! [`HostCaps`]; [`RealHost`] implements it.

pub mod audio;
pub mod config;
pub mod keys;
pub mod password;
pub mod paths;
pub mod speech;

use kiddos_kernel::{HostCaps, HostRequest, Lang, MachineConfig};
use parking_lot::Mutex;
use std::io::Write;
use std::sync::mpsc::Sender;
use std::time::Instant;

pub use paths::Paths;

pub struct RealHost {
    start: Instant,
    paths: Paths,
    requests: Sender<HostRequest>,
    /// Called after each request is queued so the event loop wakes up.
    wake: Box<dyn Fn() + Send + Sync>,
    beeper: audio::Beeper,
    speaker: speech::Speaker,
    log_file: Mutex<Option<std::fs::File>>,
}

impl RealHost {
    pub fn new(paths: Paths, requests: Sender<HostRequest>, wake: Box<dyn Fn() + Send + Sync>) -> RealHost {
        let log_file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&paths.log)
            .ok();
        RealHost {
            start: Instant::now(),
            paths,
            requests,
            wake,
            beeper: audio::Beeper::new(),
            speaker: speech::Speaker::new(),
            log_file: Mutex::new(log_file),
        }
    }

    pub fn paths(&self) -> &Paths {
        &self.paths
    }
}

fn timestamp() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    format!("{secs}")
}

impl HostCaps for RealHost {
    fn now_ms(&self) -> u64 {
        self.start.elapsed().as_millis() as u64
    }
    fn unix_time(&self) -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0)
    }
    fn sleep_ms(&self, ms: u64) {
        std::thread::sleep(std::time::Duration::from_millis(ms));
    }
    fn speak(&self, text: &str, lang: Lang) {
        self.speaker.speak(text, lang);
    }
    fn beep(&self, freq: u32, ms: u32) {
        self.beeper.beep(freq, ms);
    }
    fn request(&self, r: HostRequest) {
        let _ = self.requests.send(r);
        (self.wake)();
    }
    fn config_changed(&self, cfg: &MachineConfig) {
        if let Err(e) = config::save(&self.paths.config, cfg) {
            log::warn!("could not save config: {e}");
        }
    }
    fn log(&self, line: &str) {
        log::info!("{line}");
        if let Some(f) = self.log_file.lock().as_mut() {
            let _ = writeln!(f, "{} {}", timestamp(), line);
        }
    }
    fn verify_parent_password(&self, password: &str) -> Option<bool> {
        password::verify(&self.paths.parent_hash, password)
    }
    fn set_parent_password(&self, password: &str) -> Result<(), String> {
        password::set(&self.paths.parent_hash, password)
    }
    fn read_log(&self, max_lines: usize) -> Vec<String> {
        let text = std::fs::read_to_string(&self.paths.log).unwrap_or_default();
        let lines: Vec<String> = text.lines().map(|l| l.to_string()).collect();
        let start = lines.len().saturating_sub(max_lines);
        lines[start..].to_vec()
    }
    fn tz_offset_secs(&self) -> i32 {
        local_tz_offset()
    }
    fn list_cart_files(&self) -> Vec<String> {
        let mut out: Vec<String> = std::fs::read_dir(&self.paths.carts)
            .map(|rd| {
                rd.filter_map(|e| e.ok())
                    .map(|e| e.file_name().to_string_lossy().to_string())
                    .filter(|n| n.ends_with(".kdc"))
                    .collect()
            })
            .unwrap_or_default();
        out.sort();
        out
    }
    fn read_cart_file(&self, name: &str) -> Result<Vec<u8>, String> {
        if name.contains('/') || name.contains('\\') || name.starts_with('.') {
            return Err("cartridge names are plain file names".into());
        }
        std::fs::read(self.paths.carts.join(name)).map_err(|e| format!("{name}: {e}"))
    }
    fn write_cart_file(&self, name: &str, data: &[u8]) -> Result<String, String> {
        if name.contains('/') || name.contains('\\') || name.starts_with('.') {
            return Err("cartridge names are plain file names".into());
        }
        let path = self.paths.carts.join(name);
        std::fs::write(&path, data).map_err(|e| format!("{}: {e}", path.display()))?;
        Ok(path.display().to_string())
    }
    fn cart_folder_hint(&self) -> String {
        self.paths.carts.display().to_string()
    }
    fn c_compiler_available(&self) -> Result<(), String> {
        self.find_clang().map(|_| ())
    }
    fn compile_c(&self, files: &[(String, Vec<u8>)]) -> Result<Vec<u8>, String> {
        let clang = self.find_clang()?;
        let stamp = format!("{}-{}", std::process::id(), self.now_ms());
        let dir = self.paths.build.join(stamp);
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        let mut sources = Vec::new();
        for (name, data) in files {
            let safe = std::path::Path::new(name)
                .file_name()
                .map(|f| f.to_string_lossy().to_string())
                .unwrap_or_default();
            if safe.is_empty() {
                continue;
            }
            std::fs::write(dir.join(&safe), data).map_err(|e| e.to_string())?;
            if safe.ends_with(".c") {
                sources.push(safe);
            }
        }
        let out = dir.join("out.wasm");
        let mut cmd = std::process::Command::new(&clang);
        cmd.current_dir(&dir)
            .args([
                "--target=wasm32",
                "-O2",
                "-nostdlib",
                "-fno-builtin",
                "-Wall",
                "-Wno-unused-function",
                "-I.",
            ])
            .args(["-Wl,--no-entry", "-Wl,--export=main", "-Wl,-z,stack-size=65536"])
            .args(&sources)
            .arg("-o")
            .arg(&out);
        let output = cmd
            .output()
            .map_err(|e| format!("could not run {}: {e}", clang.display()))?;
        let text = format!(
            "{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        let result = if output.status.success() {
            std::fs::read(&out).map_err(|e| e.to_string())
        } else {
            Err(text)
        };
        let _ = std::fs::remove_dir_all(&dir);
        result
    }
}

impl RealHost {
    /// `KIDDOS_CC`, else `packs/c/bin/clang` beside the drive.
    fn find_clang(&self) -> Result<std::path::PathBuf, String> {
        if let Ok(p) = std::env::var("KIDDOS_CC") {
            let p = std::path::PathBuf::from(p);
            if p.exists() {
                return Ok(p);
            }
            return Err(format!("KIDDOS_CC points at {}, which does not exist", p.display()));
        }
        let packed = self.paths.packs.join("c").join("bin").join("clang");
        if packed.exists() {
            return Ok(packed);
        }
        Err(format!(
            "this machine has no C compiler yet. A parent installs the C pack into {} (a clang that can make wasm32; see docs/PACKS.md).",
            self.paths.packs.join("c").display()
        ))
    }
}

#[cfg(unix)]
pub fn local_tz_offset() -> i32 {
    unsafe {
        let t = libc::time(std::ptr::null_mut());
        let mut tm: libc::tm = std::mem::zeroed();
        if libc::localtime_r(&t, &mut tm).is_null() {
            return 0;
        }
        tm.tm_gmtoff as i32
    }
}

#[cfg(not(unix))]
pub fn local_tz_offset() -> i32 {
    0
}
