//! The machine itself: help hi man apropos whoami hostname date cal uptime
//! ps kill sleep clear env true false games

use crate::util::{datetime, days_from_civil, days_in_month, tz, wants_help, Args, DAYS, MONTHS};
use kiddos_kernel::{CmdResult, Command, Console, Kernel, Proc, Topic};

pub fn register(k: &Kernel) {
    use Topic::System as S;
    for c in [
        Command::new("help", help, "show what I can do", S),
        Command::new("hi", hi, "say hello to the machine", S),
        Command::new("man", man, "read the manual for a command", S),
        Command::new("apropos", apropos, "search the manual for a word", S),
        Command::new("whoami", whoami, "who is logged in?", S),
        Command::new("hostname", hostname, "what is this machine called?", S),
        Command::new("date", date, "what day and time is it?", S),
        Command::new("cal", cal, "show a calendar", S),
        Command::new("uptime", uptime, "how long has the machine been on?", S),
        Command::new("ps", ps, "list running programs", S),
        Command::new("kill", kill, "stop a running program by its number", S),
        Command::new("sleep", sleep, "wait for some seconds", S),
        Command::new("clear", clear, "wipe the screen clean", S),
        Command::new("env", env, "show all variables", S),
        Command::new("true", |_, _| Ok(0), "do nothing, successfully", S),
        Command::new("false", |_, _| Ok(1), "do nothing, unsuccessfully", S),
        Command::new("exit", |_, _| Ok(0), "leave (there is nowhere to go)", S).in_shell(),
        Command::new("history", |_, _| Ok(0), "show what you typed before", S).in_shell(),
        Command::new("export", |_, _| Ok(0), "set a variable, like export NAME=Sam", S).in_shell(),
        Command::new("games", games, "list the games on this machine", Topic::Programs),
        Command::new("play", play, "start a game: play adventure", Topic::Programs),
    ] {
        k.register(c);
    }
}

fn help(p: &Proc, args: &[String]) -> CmdResult {
    let topics = [
        (Topic::Files, "help-topic-files"),
        (Topic::Text, "help-topic-text"),
        (Topic::System, "help-topic-system"),
        (Topic::Learning, "help-topic-learning"),
        (Topic::Programs, "help-topic-programs"),
        (Topic::Machine, "help-topic-machine"),
        (Topic::Parent, "parent-welcome"),
    ];
    let cmds = p.kernel().commands();
    let mut out = String::new();
    let filter = args.first().map(|s| s.as_str());
    if let Some(f) = filter {
        if Topic::from_key(f).is_none() {
            if let Some(c) = cmds.iter().find(|c| c.name == f) {
                p.println(&format!("\x1b[1m{}\x1b[0m: {}", c.name, c.summary));
                p.println(&format!("Type man {} to learn more.", c.name));
                return Ok(0);
            }
            p.println(&p.t("unknown-command", &[("cmd", f)]));
            return Ok(1);
        }
    }
    if filter.is_none() {
        out.push_str(&format!("{}\n\n", p.t("help-intro", &[])));
    }
    for (topic, key) in topics {
        if topic == Topic::Parent && !p.is_root() {
            continue;
        }
        if let Some(f) = filter {
            if Topic::from_key(f) != Some(topic) {
                continue;
            }
        }
        let mut list: Vec<_> = cmds
            .iter()
            .filter(|c| c.topic == topic && (!c.parent_only || p.is_root()))
            .collect();
        if list.is_empty() {
            continue;
        }
        list.sort_by_key(|c| c.name);
        out.push_str(&format!("\x1b[1;33m{}\x1b[0m\n", p.t(key, &[])));
        for c in list {
            out.push_str(&format!("  \x1b[1;32m{:<10}\x1b[0m {}\n", c.name, c.summary));
        }
        out.push('\n');
    }
    if filter.is_none() {
        out.push_str(&format!("{}\n", p.t("help-more", &[])));
    }
    kiddos_man::page(p, &out)?;
    Ok(0)
}

fn hi(p: &Proc, _args: &[String]) -> CmdResult {
    let name = p.kernel().kid_name.lock().clone();
    if name.is_empty() {
        p.println(&p.t("hi-first", &[]));
        let answer = p.readline("> ")?.unwrap_or_default();
        let answer = answer.trim().to_string();
        if answer.is_empty() {
            return Ok(0);
        }
        p.kernel().update_config(|c| c.kid_name = answer.clone());
        let msg = p.t("hi-named", &[("name", &answer)]);
        p.println(&msg);
        p.speak(&msg);
    } else {
        let msg = p.t("hi", &[("name", &name)]);
        p.println(&msg);
        p.speak(msg.lines().next().unwrap_or(""));
    }
    Ok(0)
}

fn man(p: &Proc, args: &[String]) -> CmdResult {
    let a = Args::parse(args, &["k"]);
    if let Some(q) = a.value("k") {
        return apropos(p, &[q.to_string()]);
    }
    let Some(name) = a.positional.first() else {
        p.println(&p.t("usage", &[("usage", "man <command>, for example: man ls")]));
        return Ok(1);
    };
    match kiddos_man::find_page(p, name) {
        Some(md) => {
            let (cols, _) = p.size();
            let text = kiddos_man::render(&md, cols as usize);
            kiddos_man::page(p, &text)?;
            Ok(0)
        }
        None => {
            if let Some(c) = p.kernel().command(name) {
                p.println(&format!("{}: {}", c.name, c.summary));
                p.println("(That's all I know about it so far.)");
                return Ok(0);
            }
            p.println(&p.t("man-no-page", &[("cmd", name)]));
            Ok(1)
        }
    }
}

fn apropos(p: &Proc, args: &[String]) -> CmdResult {
    let Some(q) = args.first() else {
        p.println(&p.t("usage", &[("usage", "apropos <word>")]));
        return Ok(1);
    };
    let mut hits = kiddos_man::search(p, q);
    // also search command summaries
    let ql = q.to_lowercase();
    for c in p.kernel().commands() {
        if c.topic == Topic::Hidden || (c.parent_only && !p.is_root()) {
            continue;
        }
        if (c.name.contains(&ql) || c.summary.to_lowercase().contains(&ql)) && !hits.iter().any(|(n, _)| n == c.name) {
            hits.push((c.name.to_string(), c.summary.to_string()));
        }
    }
    hits.sort();
    if hits.is_empty() {
        p.println(&p.t("man-search-none", &[("q", q)]));
        return Ok(1);
    }
    for (n, s) in hits {
        p.println(&format!("\x1b[1;32m{n:<12}\x1b[0m {s}"));
    }
    Ok(0)
}

fn whoami(p: &Proc, _args: &[String]) -> CmdResult {
    p.println(&p.user);
    Ok(0)
}

fn hostname(p: &Proc, _args: &[String]) -> CmdResult {
    p.println(&p.kernel().config().hostname);
    Ok(0)
}

fn date(p: &Proc, _args: &[String]) -> CmdResult {
    let d = datetime(p.kernel().host().unix_time(), tz(p));
    p.println(&format!(
        "{} {} {:>2} {:02}:{:02}:{:02} {}",
        DAYS[d.weekday as usize],
        MONTHS[(d.month - 1) as usize],
        d.day,
        d.hour,
        d.minute,
        d.second,
        d.year
    ));
    Ok(0)
}

fn cal(p: &Proc, args: &[String]) -> CmdResult {
    if wants_help(p, args) {
        return Ok(0);
    }
    let now = datetime(p.kernel().host().unix_time(), tz(p));
    let (month, year) = match args {
        [m, y] => (m.parse().unwrap_or(now.month), y.parse().unwrap_or(now.year)),
        [m] => (m.parse().unwrap_or(now.month), now.year),
        _ => (now.month, now.year),
    };
    if !(1..=12).contains(&month) {
        p.println("cal: months go from 1 to 12.");
        return Ok(1);
    }
    let title = format!("{} {}", MONTHS[(month - 1) as usize], year);
    p.println(&format!("{:^20}", title));
    p.println("Su Mo Tu We Th Fr Sa");
    let first = days_from_civil(year, month, 1);
    let start = ((first + 4).rem_euclid(7)) as u32;
    let mut line = "   ".repeat(start as usize);
    let mut col = start;
    for d in 1..=days_in_month(year, month) {
        let cell = if year == now.year && month == now.month && d == now.day {
            format!("\x1b[7m{d:>2}\x1b[0m")
        } else {
            format!("{d:>2}")
        };
        line.push_str(&cell);
        col += 1;
        if col == 7 {
            p.println(&line);
            line.clear();
            col = 0;
        } else {
            line.push(' ');
        }
    }
    if !line.trim().is_empty() {
        p.println(line.trim_end());
    }
    Ok(0)
}

fn uptime(p: &Proc, _args: &[String]) -> CmdResult {
    let ms = p.tick();
    let secs = ms / 1000;
    let (h, m, s) = (secs / 3600, (secs / 60) % 60, secs % 60);
    let n = p.kernel().processes().len();
    p.println(&format!("up {h:02}:{m:02}:{s:02}, {n} programs running"));
    Ok(0)
}

fn ps(p: &Proc, _args: &[String]) -> CmdResult {
    p.println(&format!(
        "{:>5} {:>5} {:<6} {:<8} {}",
        "PID", "PPID", "USER", "STATE", "NAME"
    ));
    for i in p.kernel().processes() {
        let state = match i.state {
            kiddos_kernel::ProcState::Running => "running",
            kiddos_kernel::ProcState::Waiting => "waiting",
        };
        p.println(&format!(
            "{:>5} {:>5} {:<6} {:<8} {}",
            i.pid, i.ppid, i.user, state, i.name
        ));
    }
    Ok(0)
}

fn kill(p: &Proc, args: &[String]) -> CmdResult {
    let pids: Vec<u32> = args
        .iter()
        .filter(|a| !a.starts_with('-'))
        .filter_map(|a| a.parse().ok())
        .collect();
    if pids.is_empty() {
        p.println(&p.t("usage", &[("usage", "kill <PID>   (see PIDs with ps)")]));
        return Ok(1);
    }
    let mut status = 0;
    for pid in pids {
        if pid == 1 {
            p.println("kill: Not PID 1. That one is me!");
            status = 1;
            continue;
        }
        if !p.kernel().kill(pid) {
            p.println(&format!("kill: there is no program number {pid}."));
            status = 1;
        }
    }
    Ok(status)
}

fn sleep(p: &Proc, args: &[String]) -> CmdResult {
    let secs: f64 = args.first().and_then(|s| s.parse().ok()).unwrap_or(1.0);
    p.sleep((secs.clamp(0.0, 3600.0) * 1000.0) as u64)?;
    Ok(0)
}

fn clear(p: &Proc, _args: &[String]) -> CmdResult {
    p.clear(kiddos_console::colors::DEFAULT_BG);
    Ok(0)
}

fn env(p: &Proc, _args: &[String]) -> CmdResult {
    for (k, v) in p.env_all() {
        p.println(&format!("{k}={v}"));
    }
    Ok(0)
}

fn games(p: &Proc, _args: &[String]) -> CmdResult {
    let carts = kiddos_cart::list(p);
    if carts.is_empty() {
        p.println("No games installed yet. A parent can add them with install.");
        return Ok(0);
    }
    for m in carts {
        p.println(&format!(
            "  \x1b[1;32m{:<12}\x1b[0m {} — {}",
            m.name, m.title, m.description
        ));
    }
    p.println("Type play <name> to start one, play <name> --about to read about it.");
    Ok(0)
}

fn play(p: &Proc, args: &[String]) -> CmdResult {
    let Some(name) = args.first() else {
        return games(p, &[]);
    };
    if args.iter().any(|a| a == "--about") {
        let readme = format!("{}/README.md", kiddos_cart::dir_of(name));
        return match p.fs().read_string(&readme) {
            Ok(md) => {
                let (cols, _) = p.size();
                kiddos_man::page(p, &kiddos_man::render(&md, cols as usize))?;
                Ok(0)
            }
            Err(_) => {
                p.println(&format!("{name} has no README."));
                Ok(1)
            }
        };
    }
    match kiddos_cart::launch(p, name, &args[1..]) {
        Ok(status) => Ok(status),
        Err(e) => {
            p.println(&e);
            Ok(1)
        }
    }
}
