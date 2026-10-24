//! Connections implemented in Rust inside KIQR (`transport: builtin`): the
//! renderer behind "Make images" and SMTP behind "Send email". Everything a
//! manifest can express stays in yaml; this is only what yaml cannot do.

use crate::ext::ExecOut;
use crate::App;
use anyhow::{bail, Result};
use kiqr_core::abilities::Ability;
use kiqr_core::extensions::Manifest;
use serde_json::Value;

pub async fn run(app: &App, slug: &str, m: &Manifest, ability: Option<&Ability>, tool: Option<&str>, args: &Value) -> Result<ExecOut> {
    match (m.id.as_str(), ability.map(|a| a.id), tool) {
        ("kiqr-render", Some("image"), _) => crate::images::render_args(app, slug, args).await,
        (id, ab, t) => bail!("{} ({id}) cannot do {} yet", m.name, ab.or(t).unwrap_or("that")),
    }
}
