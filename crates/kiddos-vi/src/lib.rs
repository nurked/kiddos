//! vi for KidDOS. The editor starts locked (the plan's progression
//! mechanic): a kid earns `/bin/vi` by finishing vi-quest. Prison Escape
//! teaches the one thing everyone must know first: how to get out.

pub mod engine;
pub mod prison;
pub mod quest;
pub mod render;
pub mod vi;

use kiddos_kernel::{Command, Kernel, Topic};

pub fn register(k: &Kernel) {
    k.register_locked(
        Command::new(
            "vi",
            vi::cmd_vi,
            "the editor the grown-ups use (earned in vi-quest)",
            Topic::Programs,
        )
        .keep_alive(),
    );
    k.register(
        Command::new(
            "vi-quest",
            quest::cmd_vi_quest,
            "learn vi's spells; unlocks vi",
            Topic::Hidden,
        )
        .keep_alive(),
    );
    k.register(
        Command::new(
            "prison-escape",
            prison::cmd_prison,
            "you are locked inside vi; get out",
            Topic::Hidden,
        )
        .keep_alive(),
    );
}
