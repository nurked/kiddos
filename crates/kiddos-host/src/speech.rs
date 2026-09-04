//! Text to speech through the platform's own voice. macOS: `say`.
//! Windows: PowerShell + SAPI. Linux: `espeak-ng` (or `espeak`) if present.
//! If nothing works, the machine stays silent; the text was printed anyway.

use kiddos_kernel::Lang;
use parking_lot::Mutex;
use std::process::{Child, Command, Stdio};

pub struct Speaker {
    current: Mutex<Option<Child>>,
}

impl Default for Speaker {
    fn default() -> Self {
        Speaker::new()
    }
}

impl Speaker {
    pub fn new() -> Speaker {
        Speaker {
            current: Mutex::new(None),
        }
    }

    /// Start speaking; whatever was being said is cut off.
    pub fn speak(&self, text: &str, lang: Lang) {
        let text: String = text.chars().filter(|c| !c.is_control()).take(400).collect();
        if text.trim().is_empty() {
            return;
        }
        let mut cur = self.current.lock();
        if let Some(mut c) = cur.take() {
            let _ = c.kill();
            let _ = c.wait();
        }
        *cur = spawn_tts(&text, lang);
    }
}

#[cfg(target_os = "macos")]
fn spawn_tts(text: &str, _lang: Lang) -> Option<Child> {
    let mut cmd = Command::new("say");
    cmd.arg("--").arg(text);
    cmd.stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null());
    cmd.spawn().ok()
}

#[cfg(target_os = "windows")]
fn spawn_tts(text: &str, _lang: Lang) -> Option<Child> {
    let escaped = text.replace('\'', "''");
    let culture = "en-US";
    let script = format!(
        "Add-Type -AssemblyName System.Speech; $s = New-Object System.Speech.Synthesis.SpeechSynthesizer; \
         try {{ $s.SelectVoiceByHints('NotSet','NotSet',0,[System.Globalization.CultureInfo]'{culture}') }} catch {{}}; \
         $s.Speak('{escaped}')"
    );
    Command::new("powershell")
        .args(["-NoProfile", "-NonInteractive", "-Command", &script])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .ok()
}

#[cfg(all(unix, not(target_os = "macos")))]
fn spawn_tts(text: &str, _lang: Lang) -> Option<Child> {
    let voice = "en";
    for bin in ["espeak-ng", "espeak"] {
        if let Ok(c) = Command::new(bin)
            .args(["-v", voice, "--", text])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
        {
            return Some(c);
        }
    }
    None
}
