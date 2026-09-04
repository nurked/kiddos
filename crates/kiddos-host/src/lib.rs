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
