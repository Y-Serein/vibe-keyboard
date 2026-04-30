//! vk-daemon library — session monitoring, permission handling, focus management, config.

pub(crate) mod cesp;
pub mod config;
pub mod discovery;
pub mod focus;
pub mod keystroke;
pub(crate) mod local_speaker;
pub mod notification;
pub mod permission;
// platform.rs removed (P1-6: all dead code, 3 identical impls)
pub mod server;
pub mod session;
pub mod setup;
pub(crate) mod terminal;
pub(crate) mod transcript;
