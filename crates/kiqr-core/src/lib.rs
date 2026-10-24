//! KIQR core: everything the daemon and CLI share.
//!
//! Storage is a plain directory (`~/.kiqr`, or `/data` in Docker) of YAML and
//! Markdown files under a local git repository the daemon commits to after
//! every change. There is no database.

pub mod abilities;
pub mod agent;
pub mod assets;
pub mod brand;
pub mod channels;
pub mod chat;
pub mod config;
pub mod crawl;
pub mod events;
pub mod extensions;
pub mod extract;
pub mod fetch;
pub mod markdown;
pub mod paths;
pub mod profile;
pub mod project;
pub mod provider;
pub mod render;
pub mod store;
pub mod tasks;
pub mod tools;

pub use config::Config;
pub use events::{Event, EventBus};
pub use paths::Home;
pub use store::Store;
