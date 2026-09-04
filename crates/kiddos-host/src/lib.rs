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
    fn list_packs(&self) -> Vec<(String, String)> {
        let mut out = Vec::new();
        if let Ok(rd) = std::fs::read_dir(&self.paths.packs) {
            for e in rd.filter_map(|e| e.ok()) {
                let dir = e.path();
                if !dir.is_dir() {
                    continue;
                }
                let folder = e.file_name().to_string_lossy().to_string();
                let (_, desc) = self.read_pack_toml(&dir);
                let desc = if desc.is_empty() {
                    if dir.join("bin").join("clang").exists() {
                        "a C compiler (clang)".to_string()
                    } else {
                        "(no pack.toml)".to_string()
                    }
                } else {
                    desc
                };
                out.push((folder, desc));
            }
        }
        out.sort();
        out
    }
    fn install_pack(&self, file: &str) -> Result<String, String> {
        use std::io::Read;
        if file.contains('/') || file.starts_with('.') {
            return Err("pack names are plain file names".into());
        }
        let bytes = std::fs::read(self.paths.carts.join(file)).map_err(|e| format!("{file}: {e}"))?;
        let mut za = zip::ZipArchive::new(std::io::Cursor::new(bytes))
            .map_err(|_| format!("{file} is not a .kdp (zip) file"))?;
        // find pack.toml, possibly under one top-level folder
        let mut prefix = String::new();
        let mut found = false;
        for i in 0..za.len() {
            let f = za.by_index(i).map_err(|e| e.to_string())?;
            let n = f.name().to_string();
            if n == "pack.toml" {
                found = true;
                break;
            }
            if n.ends_with("/pack.toml") && n.matches('/').count() == 1 {
                prefix = n.trim_end_matches("pack.toml").to_string();
                found = true;
                break;
            }
        }
        if !found {
            return Err(format!("{file} has no pack.toml inside"));
        }
        let mut toml_text = String::new();
        za.by_name(&format!("{prefix}pack.toml"))
            .map_err(|e| e.to_string())?
            .read_to_string(&mut toml_text)
            .map_err(|e| e.to_string())?;
        let name = toml_text
            .lines()
            .find_map(|l| {
                l.split_once('=')
                    .filter(|(k, _)| k.trim() == "name")
                    .map(|(_, v)| v.trim().trim_matches('"').to_string())
            })
            .ok_or("pack.toml has no name")?;
        if name.is_empty() || !name.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') {
            return Err(format!("bad pack name {name:?}"));
        }
        let dest = self.pack_dir(&name);
        if dest.exists() {
            if dest.is_symlink() {
                std::fs::remove_file(&dest).map_err(|e| e.to_string())?;
            } else {
                std::fs::remove_dir_all(&dest).map_err(|e| e.to_string())?;
            }
        }
        let mut count = 0usize;
        for i in 0..za.len() {
            let mut f = za.by_index(i).map_err(|e| e.to_string())?;
            let n = f.name().replace('\\', "/");
            let Some(rel) = n.strip_prefix(&prefix) else { continue };
            if rel.is_empty() || rel.contains("..") || rel.starts_with('/') {
                continue;
            }
            let out = dest.join(rel);
            if f.is_dir() {
                std::fs::create_dir_all(&out).map_err(|e| e.to_string())?;
                continue;
            }
            if let Some(parent) = out.parent() {
                std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
            }
            let mut data = Vec::new();
            f.read_to_end(&mut data).map_err(|e| e.to_string())?;
            std::fs::write(&out, &data).map_err(|e| e.to_string())?;
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let exec = f.unix_mode().map(|m| m & 0o111 != 0).unwrap_or(false) || rel.starts_with("bin/");
                if exec {
                    let _ = std::fs::set_permissions(&out, std::fs::Permissions::from_mode(0o755));
                }
            }
            count += 1;
        }
        Ok(format!("{name}: {count} files into {}", dest.display()))
    }
    fn remove_pack(&self, name: &str) -> Result<(), String> {
        if name.is_empty() || !name.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') {
            return Err("bad pack name".into());
        }
        let dir = self.pack_dir(name);
        if dir.is_symlink() {
            std::fs::remove_file(&dir).map_err(|e| e.to_string())
        } else if dir.is_dir() {
            std::fs::remove_dir_all(&dir).map_err(|e| e.to_string())
        } else {
            Err(format!("there is no pack called {name}"))
        }
    }
    fn c_compiler_available(&self) -> Result<(), String> {
        self.find_clang().map(|_| ())
    }
    fn go_compiler_available(&self) -> Result<(), String> {
        self.find_tinygo().map(|_| ())
    }
    fn compile_go(&self, files: &[(String, Vec<u8>)], pkg: &[(String, Vec<u8>)]) -> Result<Vec<u8>, String> {
        let tinygo = self.find_tinygo()?;
        let stamp = format!("go-{}-{}", std::process::id(), self.now_ms());
        let dir = self.paths.build.join(stamp);
        let pkgdir = dir.join("kiddos");
        std::fs::create_dir_all(&pkgdir).map_err(|e| e.to_string())?;
        for (name, data) in files {
            let safe = std::path::Path::new(name)
                .file_name()
                .map(|f| f.to_string_lossy().to_string())
                .unwrap_or_default();
            if safe.ends_with(".go") {
                std::fs::write(dir.join(&safe), data).map_err(|e| e.to_string())?;
            }
        }
        for (name, data) in pkg {
            let safe = std::path::Path::new(name)
                .file_name()
                .map(|f| f.to_string_lossy().to_string())
                .unwrap_or_default();
            if !safe.is_empty() {
                std::fs::write(pkgdir.join(&safe), data).map_err(|e| e.to_string())?;
            }
        }
        std::fs::write(
            dir.join("go.mod"),
            "module kidprog

go 1.22

require kiddos v0.0.0

replace kiddos => ./kiddos
",
        )
        .map_err(|e| e.to_string())?;
        // TinyGo's bare wasm target never calls Go's main: export one that does
        std::fs::write(
            dir.join("zz_kiddos_entry.go"),
            "package main\n\n//export kiddos_main\nfunc kiddosMain() { main() }\n",
        )
        .map_err(|e| e.to_string())?;
        let out = dir.join("out.wasm");
        let mut path = String::new();
        if let Some(bin) = tinygo.parent() {
            path.push_str(&bin.display().to_string());
            path.push(':');
        }
        path.push_str(&self.paths.packs.join("go").join("bin").display().to_string());
        path.push(':');
        path.push_str(&self.paths.packs.join("go").join("go").join("bin").display().to_string());
        path.push_str(":/opt/homebrew/bin:/usr/local/go/bin:/usr/local/bin:/usr/bin:/bin:");
        path.push_str(&std::env::var("PATH").unwrap_or_default());
        let mut cmd = std::process::Command::new(&tinygo);
        cmd.env("PATH", path)
            .env("GOFLAGS", "-mod=mod")
            .env("GOPROXY", "off")
            .env("GOSUMDB", "off")
            .env("HOME", &self.paths.build)
            .current_dir(&dir)
            .args(["build", "-target=wasm-unknown", "-opt=z", "-no-debug", "-o"])
            .arg(&out)
            .arg(".");
        if let Some(root) = self
            .paths
            .packs
            .join("go")
            .join("go")
            .canonicalize()
            .ok()
            .filter(|p| p.join("bin").join("go").exists())
        {
            cmd.env("GOROOT", &root);
        }
        let output = cmd
            .output()
            .map_err(|e| format!("could not run {}: {e}", tinygo.display()))?;
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
        // clang finds wasm-ld on PATH; an app launched from a desktop icon
        // has almost none, so add the usual places and clang's own folder.
        let mut path = String::new();
        if let Some(bin) = clang.parent() {
            path.push_str(&bin.display().to_string());
            path.push(':');
        }
        path.push_str(&self.paths.packs.join("c").join("bin").display().to_string());
        path.push_str(":/opt/homebrew/bin:/usr/local/bin:/usr/bin:/bin:");
        path.push_str(&std::env::var("PATH").unwrap_or_default());
        let mut cmd = std::process::Command::new(&clang);
        cmd.env("PATH", path);
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
            .args(["-Wl,--no-entry", "-Wl,--export-all", "-Wl,-z,stack-size=65536"])
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
    fn pack_dir(&self, name: &str) -> std::path::PathBuf {
        self.paths.packs.join(name)
    }

    fn read_pack_toml(&self, dir: &std::path::Path) -> (String, String) {
        let text = std::fs::read_to_string(dir.join("pack.toml")).unwrap_or_default();
        let mut name = String::new();
        let mut desc = String::new();
        for line in text.lines() {
            if let Some((k, v)) = line.split_once('=') {
                let v = v.trim().trim_matches('"').to_string();
                match k.trim() {
                    "name" => name = v,
                    "description" => desc = v,
                    _ => {}
                }
            }
        }
        (name, desc)
    }

    /// `KIDDOS_TINYGO`, else `packs/go/bin/tinygo` beside the drive.
    fn find_tinygo(&self) -> Result<std::path::PathBuf, String> {
        if let Ok(p) = std::env::var("KIDDOS_TINYGO") {
            let p = std::path::PathBuf::from(p);
            if p.exists() {
                return Ok(p);
            }
            return Err(format!("KIDDOS_TINYGO points at {}, which does not exist", p.display()));
        }
        let packed = self.paths.packs.join("go").join("bin").join("tinygo");
        if packed.exists() {
            return Ok(packed);
        }
        Err(format!(
            "this machine has no Go compiler yet. A parent installs the Go pack into {} (see docs/PACKS.md).",
            self.paths.packs.join("go").display()
        ))
    }

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
