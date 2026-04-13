pub mod boot;
pub mod embed;
pub mod format;
pub mod pack;
pub mod vfs;

pub use boot::{LaunchMode, detect_and_prepare};
pub use format::{MAGIC_BYTES, StandaloneMetadata, VfsEntry};
pub use pack::pack_app;
