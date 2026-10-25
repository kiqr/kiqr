//! Connections implemented in Rust inside KIQR (`transport: builtin`): only
//! the core renderer behind "Make images". Everything a manifest can express
//! stays in yaml, and a new need gets a generic mechanism, not a branch here.

use crate::ext::ExecOut;
use crate::App;
use anyhow::{bail, Result};
use kiqr_core::abilities::Ability;
use kiqr_core::extensions::Manifest;
use serde_json::Value;

pub async fn run(app: &App, slug: &str, m: &Manifest, ability: Option<&Ability>, tool: Option<&str>, args: &Value) -> Result<ExecOut> {
    match (m.id.as_str(), ability.map(|a| a.id), tool) {
        (id, Some("image"), _) if kiqr_core::extensions::is_core_id(id) => crate::images::render_args(app, slug, args).await,
        (id, ab, t) => bail!("{} ({id}) cannot do {} yet", m.name, ab.or(t).unwrap_or("that")),
    }
}
