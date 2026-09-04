//! A KidDOS machine with a null host. Tests type at it and read the screen.

use kiddos_kernel::{Child, Kernel, Key, MachineConfig, NullHost, Vfs};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

pub struct Machine {
    pub kernel: Arc<Kernel>,
    pub host: Arc<NullHost>,
    _init: Child,
}

/// The factory drive directory in the source tree.
pub fn factory_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../content/factory-drive")
}

impl Machine {
    /// Boot from the factory drive with a frozen clock and no kid name.
    pub fn boot() -> Machine {
        let vfs = Vfs::from_dir(&factory_dir()).expect("factory drive");
        Machine::boot_with(vfs, NullHost::frozen(1_788_811_200), MachineConfig::default())
    }

    pub fn boot_with(vfs: Vfs, host: NullHost, config: MachineConfig) -> Machine {
        let host = Arc::new(host);
        let kernel = Kernel::new(vfs, host.clone(), config, 80, 25);
        kiddos_builtins::register_all(&kernel);
        kiddos_shell::register(&kernel);
        kiddos_basic::register(&kernel);
        kiddos_cart::register(&kernel);
        kiddos_tutor::Tutor::install(&kernel);
        let init = kernel.boot();
        let m = Machine {
            kernel,
            host,
            _init: init,
        };
        m.settle();
        m
    }

    /// Wait until the machine is blocked on keyboard input (or 5 s pass).
    pub fn settle(&self) {
        let start = Instant::now();
        let mut idle_streak = 0;
        while start.elapsed() < Duration::from_secs(5) {
            if self.kernel.is_idle() {
                idle_streak += 1;
                if idle_streak >= 3 {
                    return;
                }
            } else {
                idle_streak = 0;
            }
            std::thread::sleep(Duration::from_millis(5));
        }
    }

    pub fn key(&self, k: Key) {
        self.kernel.push_key(k);
        self.settle();
    }

    /// Type text (no Enter) and wait.
    pub fn type_text(&self, s: &str) {
        self.kernel.push_text(s);
        self.settle();
    }

    /// Type a line and press Enter.
    pub fn run(&self, line: &str) {
        self.kernel.push_text(line);
        self.kernel.push_key(Key::Enter);
        self.settle();
    }

    pub fn screen(&self) -> String {
        self.kernel.screen_text()
    }

    pub fn line(&self, y: u16) -> String {
        self.kernel.screen.lock().line(y)
    }

    /// One cell, copied out (never hold the screen lock across an assert:
    /// the failure message would take it again and deadlock).
    pub fn cell(&self, x: u16, y: u16) -> kiddos_console::Cell {
        self.kernel.screen.lock().cell(x, y)
    }

    /// The screen lines *after* the last prompt that echoed `line`.
    pub fn output_of(&self, line: &str) -> String {
        let screen = self.screen();
        let lines: Vec<&str> = screen.lines().collect();
        let idx = lines
            .iter()
            .rposition(|l| l.ends_with(line) && (l.contains("$ ") || l.contains("# ")));
        match idx {
            Some(i) => {
                let rest: Vec<&str> = lines[i + 1..]
                    .iter()
                    .copied()
                    .take_while(|l| !l.starts_with("kid@") && !l.starts_with("root@"))
                    .collect();
                rest.join("\n")
            }
            None => String::new(),
        }
    }

    pub fn clear_screen(&self) {
        self.run("clear");
    }

    pub fn spoken(&self) -> Vec<String> {
        self.host.spoken.lock().clone()
    }

    /// Run a script in the headless DSL:
    /// * a plain line is typed followed by Enter
    /// * `{tab}`, `{up}`, `{ctrl-c}`, `{enter}` … inside a line are keys
    /// * lines starting with `#` are comments
    /// * `@type text` types without Enter
    /// * `@expect text` panics unless the screen contains text
    /// * `@absent text` panics if the screen contains text
    /// * `@clear` clears the screen
    pub fn run_script(&self, script: &str) {
        for (n, raw) in script.lines().enumerate() {
            let line = raw.trim_end();
            if line.trim().is_empty() || line.trim_start().starts_with('#') {
                continue;
            }
            if let Some(rest) = line.strip_prefix("@expect ") {
                assert!(
                    self.screen().contains(rest),
                    "line {}: expected {:?} on screen:\n{}",
                    n + 1,
                    rest,
                    self.screen()
                );
            } else if let Some(rest) = line.strip_prefix("@absent ") {
                assert!(
                    !self.screen().contains(rest),
                    "line {}: did not expect {:?} on screen:\n{}",
                    n + 1,
                    rest,
                    self.screen()
                );
            } else if line == "@clear" {
                self.clear_screen();
            } else if let Some(rest) = line.strip_prefix("@type ") {
                self.feed(rest);
                self.settle();
            } else {
                self.feed(line);
                self.kernel.push_key(Key::Enter);
                self.settle();
            }
        }
    }

    /// Feed text with `{key}` tokens.
    pub fn feed(&self, s: &str) {
        let mut rest = s;
        while let Some(i) = rest.find('{') {
            self.kernel.push_text(&rest[..i]);
            let after = &rest[i + 1..];
            match after.find('}') {
                Some(j) if Key::parse_name(&after[..j]).is_some() => {
                    self.kernel.push_key(Key::parse_name(&after[..j]).unwrap());
                    rest = &after[j + 1..];
                }
                _ => {
                    self.kernel.push_text("{");
                    rest = after;
                }
            }
        }
        self.kernel.push_text(rest);
    }
}
