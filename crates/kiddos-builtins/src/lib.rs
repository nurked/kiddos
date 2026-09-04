//! Every command the machine knows. One file per group; `register_all`
//! puts them all in the kernel's registry.

pub mod edit;
pub mod files;
pub mod fun;
pub mod machine;
pub mod parent;
pub mod system;
pub mod text;
pub mod util;

use kiddos_kernel::Kernel;

pub fn register_all(k: &Kernel) {
    files::register(k);
    edit::register(k);
    text::register(k);
    system::register(k);
    machine::register(k);
    parent::register(k);
    fun::register(k);
}
