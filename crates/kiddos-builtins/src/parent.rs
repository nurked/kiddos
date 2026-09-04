//! Parent-only commands.

use kiddos_kernel::{CmdResult, Command, Console, HostRequest, Kernel, Lang, Proc, Topic};

pub fn register(k: &Kernel) {
    use Topic::Parent as P;
    for c in [
        Command::new(
            "exit-fullscreen",
            exit_fullscreen,
            "let the window be a normal window",
            P,
        )
        .parent(),
        Command::new("fullscreen", fullscreen, "go back to full screen", P).parent(),
        Command::new(
            "reset-drive",
            reset_drive,
            "wipe the drive back to factory (asks first)",
            P,
        )
        .parent(),
        Command::new("log", log, "show what happened on this machine", P).parent(),
        Command::new("set-lang", set_lang, "set the machine language", P).parent(),
        Command::new("set-name", set_name, "set the kid's name", P).parent(),
        Command::new("passwd", passwd, "change the parent password", P).parent(),
    ] {
        k.register(c);
    }
}

fn exit_fullscreen(p: &Proc, _args: &[String]) -> CmdResult {
    p.kernel().request(HostRequest::ExitFullscreen);
    Ok(0)
}

fn fullscreen(p: &Proc, _args: &[String]) -> CmdResult {
    p.kernel().request(HostRequest::EnterFullscreen);
    Ok(0)
}

fn reset_drive(p: &Proc, _args: &[String]) -> CmdResult {
    p.println("This erases EVERYTHING the kid made and puts the drive back to new.");
    let Some(a) = p.readline("Type yes to do it: ")? else {
        return Ok(1);
    };
    if a.trim() != "yes" {
        p.println("OK, nothing changed.");
        return Ok(1);
    }
    p.kernel().log("drive reset requested");
    p.kernel().request(HostRequest::ResetDrive);
    p.println(&p.t("reboot", &[]));
    p.sleep(300)?;
    p.kernel().request(HostRequest::Reboot);
    Ok(0)
}

fn log(p: &Proc, args: &[String]) -> CmdResult {
    let n: usize = args.first().and_then(|s| s.parse().ok()).unwrap_or(40);
    let lines = p.kernel().host().read_log(n);
    if lines.is_empty() {
        p.println("(the log is empty)");
    }
    let text = lines.join("\n") + "\n";
    kiddos_man::page(p, &text)?;
    Ok(0)
}

fn set_lang(p: &Proc, args: &[String]) -> CmdResult {
    match args.first().and_then(|c| Lang::from_code(c)) {
        Some(l) => {
            p.kernel().set_lang(l);
            p.println(&p.t("lang-set", &[]));
            Ok(0)
        }
        None => {
            p.println("set-lang en   or   set-lang ru");
            Ok(1)
        }
    }
}

fn set_name(p: &Proc, args: &[String]) -> CmdResult {
    let name = args.join(" ");
    p.kernel().update_config(|c| c.kid_name = name.trim().to_string());
    p.println(&format!(
        "Name: {}",
        if name.trim().is_empty() {
            "(none, hi will ask)"
        } else {
            name.trim()
        }
    ));
    Ok(0)
}

fn passwd(p: &Proc, _args: &[String]) -> CmdResult {
    p.println(&p.t("parent-set-password", &[]));
    let Some(a) = p.read_secret("> ")? else { return Ok(1) };
    p.println(&p.t("parent-repeat-password", &[]));
    let Some(b) = p.read_secret("> ")? else { return Ok(1) };
    if a != b || a.is_empty() {
        p.println(&p.t("parent-mismatch", &[]));
        return Ok(1);
    }
    match p.kernel().host().set_parent_password(&a) {
        Ok(()) => {
            p.kernel().log("parent password changed");
            p.println("Done.");
            Ok(0)
        }
        Err(e) => {
            p.println(&format!("passwd: {e}"));
            Ok(1)
        }
    }
}
