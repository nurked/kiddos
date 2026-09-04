//! `kiddos-headless script.txt` runs a script and prints the screen.
//! `kiddos-headless -` reads lines from stdin, one per command, printing the
//! screen after each (handy for poking at the machine from a terminal).

use kiddos_headless::Machine;
use std::io::BufRead;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let m = Machine::boot();
    match args.first().map(|s| s.as_str()) {
        Some("-") => {
            println!("{}\n{}", m.screen(), "-".repeat(80));
            for line in std::io::stdin().lock().lines() {
                let line = line.unwrap_or_default();
                m.feed(&line);
                m.kernel.push_key(kiddos_kernel::Key::Enter);
                m.settle();
                println!("{}\n{}", m.screen(), "-".repeat(80));
            }
        }
        Some(path) => {
            let script = std::fs::read_to_string(path).expect("read script");
            m.run_script(&script);
            println!("{}", m.screen());
        }
        None => {
            eprintln!("usage: kiddos-headless <script.txt> | -");
            std::process::exit(2);
        }
    }
    let spoken = m.spoken();
    if !spoken.is_empty() {
        eprintln!("[spoken] {}", spoken.join(" / "));
    }
}
