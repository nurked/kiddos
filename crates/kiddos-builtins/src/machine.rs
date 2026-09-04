//! Machine controls: speak beep crt font lang reboot shutdown parent

use crate::util::wants_help;
use kiddos_kernel::{CmdResult, Command, Console, HostRequest, Kernel, Lang, Proc, Spawn, Topic};
use std::sync::Mutex;

pub fn register(k: &Kernel) {
    use Topic::Machine as M;
    for c in [
        Command::new("speak", speak, "make the machine talk out loud", M),
        Command::new("beep", beep, "make a sound", M),
        Command::new("crt", crt, "turn the old-TV look on or off", M),
        Command::new("font", font, "change the letters", M),
        Command::new("lang", lang, "which language the machine speaks", M),
        Command::new("reboot", reboot, "restart the machine", M),
        Command::new("shutdown", shutdown, "turn the machine off (parents only)", M).parent(),
        Command::new("parent", parent, "parent mode (needs the parent password)", M),
    ] {
        k.register(c);
    }
}

fn speak(p: &Proc, args: &[String]) -> CmdResult {
    if wants_help(p, args) {
        return Ok(0);
    }
    let text = if args.is_empty() {
        if p.stdin_is_tty() {
            p.println(&p.t("usage", &[("usage", "speak hello there   or   echo hi | speak")]));
            return Ok(1);
        }
        String::from_utf8_lossy(&p.read_stdin_all()?).to_string()
    } else {
        args.join(" ")
    };
    if !p.caps.speak {
        p.println(&p.t("speak-denied", &[]));
        return Ok(1);
    }
    let mut status = 0;
    for line in text.lines().filter(|l| !l.trim().is_empty()) {
        if !p.speak(line.trim()) {
            p.println(&p.t("speak-too-fast", &[]));
            status = 1;
            p.sleep(500)?;
        } else {
            p.sleep(400)?;
        }
    }
    Ok(status)
}

fn beep(p: &Proc, args: &[String]) -> CmdResult {
    let freq: u32 = args.first().and_then(|s| s.parse().ok()).unwrap_or(880);
    let ms: u32 = args.get(1).and_then(|s| s.parse().ok()).unwrap_or(150);
    p.beep(freq, ms);
    p.print("\x07");
    p.sleep(ms as u64)?;
    Ok(0)
}

fn crt(p: &Proc, args: &[String]) -> CmdResult {
    let on = match args.first().map(|s| s.as_str()) {
        Some("on") => true,
        Some("off") => false,
        _ => {
            let cur = p.kernel().config().crt;
            p.println(&p.t(if cur { "crt-on" } else { "crt-off" }, &[]));
            p.println(&p.t("usage", &[("usage", "crt on   or   crt off")]));
            return Ok(0);
        }
    };
    p.kernel().update_config(|c| c.crt = on);
    p.kernel().request(HostRequest::Crt(on));
    p.println(&p.t(if on { "crt-on" } else { "crt-off" }, &[]));
    Ok(0)
}

pub const FONTS: [&str; 1] = ["cpc"];

fn font(p: &Proc, args: &[String]) -> CmdResult {
    match args.first() {
        Some(name) if FONTS.contains(&name.as_str()) => {
            p.kernel().update_config(|c| c.font = name.clone());
            p.kernel().request(HostRequest::Font(name.clone()));
            p.println(&p.t("font-set", &[("font", name)]));
            Ok(0)
        }
        Some(other) => {
            p.println(&format!("font: I don't have {other}. I have: {}", FONTS.join(", ")));
            Ok(1)
        }
        None => {
            p.println(&p.t("font-set", &[("font", &p.kernel().config().font)]));
            p.println(&format!("Fonts: {}", FONTS.join(", ")));
            Ok(0)
        }
    }
}

fn lang(p: &Proc, args: &[String]) -> CmdResult {
    let langs: Vec<String> = Lang::all()
        .iter()
        .map(|l| format!("{} ({})", l.code(), l.native_name()))
        .collect();
    match args.first() {
        Some(code) => match Lang::from_code(code) {
            Some(l) => {
                p.kernel().set_lang(l);
                let msg = p.t("lang-set", &[]);
                p.println(&msg);
                p.speak(&msg);
                Ok(0)
            }
            None => {
                p.println(&p.t("lang-unknown", &[("langs", &langs.join(", "))]));
                Ok(1)
            }
        },
        None => {
            p.println(&format!("{} — {}", p.lang().code(), p.lang().native_name()));
            p.println(&p.t("lang-unknown", &[("langs", &langs.join(", "))]));
            Ok(0)
        }
    }
}

fn reboot(p: &Proc, _args: &[String]) -> CmdResult {
    p.println(&p.t("reboot", &[]));
    p.sleep(300)?;
    p.kernel().request(HostRequest::Reboot);
    Ok(0)
}

fn shutdown(p: &Proc, _args: &[String]) -> CmdResult {
    p.println(&p.t("shutdown", &[]));
    p.sleep(300)?;
    p.kernel().request(HostRequest::Shutdown);
    Ok(0)
}

static LOCKOUT: Mutex<(u32, u64)> = Mutex::new((0, 0));
const MAX_TRIES: u32 = 5;
const LOCK_MS: u64 = 5 * 60 * 1000;

fn parent(p: &Proc, _args: &[String]) -> CmdResult {
    let k = p.kernel();
    let host = k.host();
    let now = host.now_ms();
    {
        let lock = LOCKOUT.lock().unwrap();
        if lock.1 > now {
            let mins = ((lock.1 - now) / 60_000 + 1).to_string();
            p.println(&p.t("parent-locked", &[("minutes", &mins)]));
            return Ok(1);
        }
    }
    if p.is_root() {
        p.println(&p.t("parent-welcome", &[]));
        return Ok(0);
    }
    if host.verify_parent_password("").is_none() {
        // first run: set a password
        p.println(&p.t("parent-set-password", &[]));
        let Some(a) = p.read_secret("> ")? else { return Ok(1) };
        p.println(&p.t("parent-repeat-password", &[]));
        let Some(b) = p.read_secret("> ")? else { return Ok(1) };
        if a != b || a.is_empty() {
            p.println(&p.t("parent-mismatch", &[]));
            return Ok(1);
        }
        if let Err(e) = host.set_parent_password(&a) {
            p.println(&format!("parent: {e}"));
            return Ok(1);
        }
        k.log("parent password set");
    } else {
        p.println(&p.t("parent-enter-password", &[]));
        let Some(pw) = p.read_secret("> ")? else { return Ok(1) };
        if host.verify_parent_password(&pw) != Some(true) {
            let mut lock = LOCKOUT.lock().unwrap();
            lock.0 += 1;
            k.log("wrong parent password");
            if lock.0 >= MAX_TRIES {
                lock.0 = 0;
                lock.1 = now + LOCK_MS;
                p.println(&p.t("parent-locked", &[("minutes", "5")]));
            } else {
                p.println(&p.t("parent-wrong", &[]));
            }
            return Ok(1);
        }
        LOCKOUT.lock().unwrap().0 = 0;
    }
    k.log("parent mode entered");
    p.println(&p.t("parent-welcome", &[]));
    let _ = k.vfs.lock().mkdir_p("/root", &kiddos_vfs::Actor::root());
    let mut s = Spawn::child_of(p, vec!["ksh".into()]);
    s.user = "root".into();
    s.cwd = "/root".into();
    s.env.insert("HOME".into(), "/root".into());
    s.env.insert("USER".into(), "root".into());
    match p.spawn(s) {
        Ok(child) => {
            child.wait();
        }
        Err(e) => p.println(&format!("parent: {e}")),
    }
    k.log("parent mode left");
    p.println(&p.t("bye", &[]));
    Ok(0)
}
