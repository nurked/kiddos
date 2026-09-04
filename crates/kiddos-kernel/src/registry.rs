//! Commands the machine knows. `ls /bin` lists exactly these.

use crate::{CmdResult, Proc};

pub type CmdFn = fn(&Proc, &[String]) -> CmdResult;

/// Where a command shows up in `help`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Topic {
    Files,
    Text,
    System,
    Learning,
    Programs,
    Machine,
    Parent,
    /// Not listed by `help` (internal like `ksh`, `init`).
    Hidden,
}

impl Topic {
    pub fn key(self) -> &'static str {
        match self {
            Topic::Files => "files",
            Topic::Text => "text",
            Topic::System => "system",
            Topic::Learning => "learning",
            Topic::Programs => "programs",
            Topic::Machine => "machine",
            Topic::Parent => "parent",
            Topic::Hidden => "hidden",
        }
    }
    pub fn from_key(k: &str) -> Option<Topic> {
        Some(match k {
            "files" => Topic::Files,
            "text" => Topic::Text,
            "system" => Topic::System,
            "learning" => Topic::Learning,
            "programs" => Topic::Programs,
            "machine" => Topic::Machine,
            "parent" => Topic::Parent,
            _ => return None,
        })
    }
}

#[derive(Clone)]
pub struct Command {
    pub name: &'static str,
    pub run: CmdFn,
    /// One line, kid language. Shown by `help` and `man -k`.
    pub summary: &'static str,
    pub topic: Topic,
    /// Only runs as root (parent mode).
    pub parent_only: bool,
    /// Runs inside the calling process instead of a new one (shell builtins
    /// like `cd`). The shell handles these itself; listed here for `help`.
    pub in_shell: bool,
    /// Full-screen programs that handle Ctrl-C themselves (editors, games):
    /// Ctrl-C arrives as a key instead of killing them.
    pub keep_alive: bool,
}

impl Command {
    pub const fn new(name: &'static str, run: CmdFn, summary: &'static str, topic: Topic) -> Command {
        Command {
            name,
            run,
            summary,
            topic,
            parent_only: false,
            in_shell: false,
            keep_alive: false,
        }
    }
    pub const fn keep_alive(mut self) -> Command {
        self.keep_alive = true;
        self
    }
    pub const fn parent(mut self) -> Command {
        self.parent_only = true;
        self
    }
    pub const fn in_shell(mut self) -> Command {
        self.in_shell = true;
        self
    }
}
