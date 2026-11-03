//! KIQR core: everything the daemon and CLI share.
//!
//! Storage is a plain directory (`~/.kiqr`, or `/data` in Docker) of YAML and
//! Markdown files under a local git repository the daemon commits to after
//! every change. There is no database.

pub mod abilities;
pub mod access;
pub mod agent;
pub mod assets;
pub mod brand;
pub mod publishing;
pub mod channels;
pub mod chat;
pub mod config;
pub mod connect;
pub mod conn;
pub mod crawl;
pub mod events;
pub mod intake;
pub mod lists;
pub mod extensions;
pub mod extract;
pub mod fetch;
pub mod marketing;
pub mod memory;
pub mod markdown;
pub mod media;
pub mod metrics;
pub mod oauth;
pub mod paths;
pub mod process;
pub mod profile;
pub mod project;
pub mod provider;
pub mod render;
pub mod secrets;
pub mod sources;
pub mod store;
pub mod tasks;
pub mod tools;
pub mod totp;
pub mod vault;

pub use config::Config;
pub use events::{Event, EventBus};
pub use paths::Home;
pub use store::Store;
