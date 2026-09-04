//! Shared helpers: argument parsing, dates, sizes.

use kiddos_kernel::Proc;

/// Tiny getopt: single-dash flag clusters (`-la`), `--word` flags, and
/// `-n 5` / `-n5` style values for flags listed in `with_value`.
pub struct Args {
    pub flags: Vec<String>,
    pub values: Vec<(String, String)>,
    pub positional: Vec<String>,
}

impl Args {
    pub fn parse(args: &[String], with_value: &[&str]) -> Args {
        let mut a = Args {
            flags: Vec::new(),
            values: Vec::new(),
            positional: Vec::new(),
        };
        let mut i = 0;
        let mut only_positional = false;
        while i < args.len() {
            let s = &args[i];
            if only_positional || s == "-" || !s.starts_with('-') {
                a.positional.push(s.clone());
            } else if s == "--" {
                only_positional = true;
            } else if let Some(long) = s.strip_prefix("--") {
                if let Some((k, v)) = long.split_once('=') {
                    a.values.push((k.to_string(), v.to_string()));
                } else if with_value.contains(&long) && i + 1 < args.len() {
                    i += 1;
                    a.values.push((long.to_string(), args[i].clone()));
                } else {
                    a.flags.push(long.to_string());
                }
            } else {
                let body = &s[1..];
                // `-5` (head/tail style) -> value for "n"
                if body.chars().all(|c| c.is_ascii_digit()) {
                    a.values.push(("n".to_string(), body.to_string()));
                    i += 1;
                    continue;
                }
                let chars: Vec<char> = body.chars().collect();
                let mut j = 0;
                while j < chars.len() {
                    let f = chars[j].to_string();
                    if with_value.contains(&f.as_str()) {
                        let rest: String = chars[j + 1..].iter().collect();
                        if !rest.is_empty() {
                            a.values.push((f, rest));
                        } else if i + 1 < args.len() {
                            i += 1;
                            a.values.push((f, args[i].clone()));
                        } else {
                            a.flags.push(f);
                        }
                        break;
                    }
                    a.flags.push(f);
                    j += 1;
                }
            }
            i += 1;
        }
        a
    }

    pub fn has(&self, f: &str) -> bool {
        self.flags.iter().any(|x| x == f)
    }

    pub fn value(&self, k: &str) -> Option<&str> {
        self.values.iter().rev().find(|(x, _)| x == k).map(|(_, v)| v.as_str())
    }

    pub fn num(&self, k: &str) -> Option<usize> {
        self.value(k).and_then(|v| v.parse().ok())
    }
}

/// `--help` on any command: summary + pointer to man.
pub fn wants_help(p: &Proc, args: &[String]) -> bool {
    if args.iter().any(|a| a == "--help") {
        if let Some(c) = p.kernel().command(&p.name) {
            p.println(&format!("{}: {}", c.name, c.summary));
        }
        p.println(&format!("Type man {} to learn more.", p.name));
        return true;
    }
    false
}

/// Print `cmd: I need something...` when there are no operands.
pub fn need_operand(p: &Proc) -> i32 {
    p.eprintln(&p.t("missing-operand", &[("cmd", &p.name)]));
    1
}

pub fn human_size(n: u64) -> String {
    if n < 1024 {
        format!("{n}")
    } else if n < 1024 * 1024 {
        format!("{:.1}K", n as f64 / 1024.0)
    } else {
        format!("{:.1}M", n as f64 / (1024.0 * 1024.0))
    }
}

pub const MONTHS: [&str; 12] = [
    "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
];
pub const DAYS: [&str; 7] = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DateTime {
    pub year: i64,
    pub month: u32,
    pub day: u32,
    pub hour: u32,
    pub minute: u32,
    pub second: u32,
    /// 0 = Sunday
    pub weekday: u32,
}

/// Howard Hinnant's civil-from-days.
pub fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}

pub fn days_from_civil(y: i64, m: u32, d: u32) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let mp = if m > 2 { m - 3 } else { m + 9 } as i64;
    let doy = (153 * mp + 2) / 5 + d as i64 - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

pub fn datetime(unix: u64, tz_offset_secs: i32) -> DateTime {
    let t = unix as i64 + tz_offset_secs as i64;
    let days = t.div_euclid(86_400);
    let secs = t.rem_euclid(86_400) as u32;
    let (year, month, day) = civil_from_days(days);
    DateTime {
        year,
        month,
        day,
        hour: secs / 3600,
        minute: (secs / 60) % 60,
        second: secs % 60,
        weekday: ((days + 4).rem_euclid(7)) as u32,
    }
}

pub fn days_in_month(year: i64, month: u32) -> u32 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        _ => {
            if (year % 4 == 0 && year % 100 != 0) || year % 400 == 0 {
                29
            } else {
                28
            }
        }
    }
}

/// `Sep  3 20:16` like `ls -l`.
pub fn short_date(unix: u64, tz: i32) -> String {
    let d = datetime(unix, tz);
    format!(
        "{} {:>2} {:02}:{:02}",
        MONTHS[(d.month - 1) as usize],
        d.day,
        d.hour,
        d.minute
    )
}

pub fn tz(p: &Proc) -> i32 {
    p.kernel().host().tz_offset_secs()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dates() {
        let d = datetime(0, 0);
        assert_eq!((d.year, d.month, d.day, d.weekday), (1970, 1, 1, 4));
        let d = datetime(1_756_929_600, 0); // 2025-09-03 20:00:00 UTC
        assert_eq!((d.year, d.month, d.day, d.hour), (2025, 9, 3, 20));
        assert_eq!(days_from_civil(2025, 9, 3), 1_756_929_600 / 86_400);
    }

    #[test]
    fn args() {
        let a = Args::parse(
            &["-la".into(), "-n".into(), "5".into(), "x".into(), "--all".into()],
            &["n"],
        );
        assert!(a.has("l") && a.has("a") && a.has("all"));
        assert_eq!(a.num("n"), Some(5));
        assert_eq!(a.positional, vec!["x"]);
        let a = Args::parse(&["-5".into()], &["n"]);
        assert_eq!(a.num("n"), Some(5));
    }
}
