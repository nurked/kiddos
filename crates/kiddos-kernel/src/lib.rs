//! The KidDOS kernel: processes, streams and pipes, capabilities, the shared
//! screen and key queue, the command registry, and the bridge to the host.
//!
//! Processes are OS threads that only ever talk to the outside world through
//! [`Proc`], which implements the [`Console`] contract and exposes a jailed,
//! user-aware filesystem view ([`Fs`]). Nothing below this crate can see the
//! host: the only door is the [`HostCaps`] trait.

pub mod fs;
pub mod host;
pub mod kernel;
pub mod proc;
pub mod registry;
pub mod stream;

pub use fs::Fs;
pub use host::{HostCaps, HostRequest, MachineConfig, NullHost};
pub use kernel::{Child, Event, Kernel, Pid, ProcInfo, ProcState, Spawn, SpawnError};
pub use kiddos_console::{Console, Interrupted, Key, KeyEvent, Pixels, Rgb, Screen};
pub use kiddos_i18n::Lang;
pub use kiddos_vfs::{Actor, Stat, Vfs, VfsError};
pub use proc::{CapSet, Proc};
pub use registry::{CmdFn, Command, Topic};
pub use stream::{Input, Output, Pipe};

/// Exit status of a process.
pub type ExitCode = i32;
/// What every command returns.
pub type CmdResult = Result<ExitCode, Interrupted>;

pub const KID_USER: &str = "kid";
pub const KID_HOME: &str = "/home/kid";
pub const ROOT_HOME: &str = "/root";
pub const HOSTNAME: &str = "kiddos";
pub const RELEASE: &str = env!("CARGO_PKG_VERSION");
