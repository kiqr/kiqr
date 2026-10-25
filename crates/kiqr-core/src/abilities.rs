//! Abilities: what Seemo can do, fixed by KIQR, each with one tool contract.
//! A connection (an extension manifest, see `extensions`) provides an ability;
//! Seemo sees one `create_image` whatever is behind it, the user sees "Make
//! images" and picks the connection in Settings when several are ready.

use crate::tools::{CallSpec, Group, ToolSpec};
use serde_json::{json, Value};

pub struct Ability {
    pub id: &'static str,
    /// The user's words: "Make images".
    pub label: &'static str,
    pub group: Group,
    /// The tool's name and chat wording.
    pub tool: &'static str,
    pub tool_label: &'static str,
    pub description: &'static str,
    pub schema: fn() -> Value,
    /// The result is a file that lands in an asset.
    pub attach: bool,
    /// The default asset type a new file is saved under when no asset is named.
    pub asset_type: &'static str,
    /// Behind an approval of this kind (sending and publishing).
    pub approval: Option<&'static str>,
}

fn obj(props: Value, required: &[&str]) -> Value {
    json!({ "type": "object", "properties": props, "required": required, "additionalProperties": false })
}

fn asset_ref() -> Value {
    json!({
        "type": "object",
        "description": "The asset the file belongs to. Omit to save a new standalone asset.",
        "properties": { "type": { "type": "string" }, "id": { "type": "string" } },
        "required": ["type", "id"],
    })
}

pub fn all() -> Vec<Ability> {
    vec![
        Ability {
            id: "image",
            label: "Make images",
            group: Group::Images,
            tool: "create_image",
            tool_label: "Making an image",
            description: "Make an image and save it as a file on an asset (role media). Two shapes, depending on the connection behind it: the built-in renderer takes `source` (SVG, or HTML when a browser is available) and renders it; a generator takes `prompt`. `size` is a preset (x, linkedin, instagram, instagram_portrait, og, cover) or WxH. The file lands on the asset at once and returns its name and URL; the user sees the whole asset, image included, in the chat and approves it there or in the library.",
            schema: || obj(
                json!({
                    "asset": asset_ref(),
                    "title": { "type": "string", "description": "Title for a new standalone image asset (when `asset` is omitted)" },
                    "name": { "type": "string", "description": "File name without extension, e.g. cover (default: image)" },
                    "prompt": { "type": "string", "description": "What to generate (generator connections)" },
                    "source": { "type": "string", "description": "SVG or HTML to render (built-in renderer)" },
                    "format": { "type": "string", "enum": ["svg", "html"], "description": "Which `source` is (built-in renderer; default svg)" },
                    "size": { "type": "string", "description": "x | linkedin | instagram | instagram_portrait | og | cover | WxH" },
                    "width": { "type": "integer" },
                    "height": { "type": "integer" },
                }),
                &[],
            ),
            attach: true,
            asset_type: "image",
            approval: None,
        },
        Ability {
            id: "video",
            label: "Make video",
            group: Group::Media,
            tool: "create_video",
            tool_label: "Making a video",
            description: "Generate a short video from a prompt through the connected generator and save it on an asset (role media). Slow and paid; say what it will cost before a batch.",
            schema: || obj(
                json!({
                    "asset": asset_ref(),
                    "title": { "type": "string" },
                    "name": { "type": "string", "description": "File name without extension" },
                    "prompt": { "type": "string" },
                    "seconds": { "type": "integer", "description": "Length in seconds, if the generator allows it" },
                    "size": { "type": "string", "description": "landscape | portrait | square, or WxH" },
                }),
                &["prompt"],
            ),
            attach: true,
            asset_type: "video",
            approval: None,
        },
        Ability {
            id: "music",
            label: "Make music",
            group: Group::Media,
            tool: "create_music",
            tool_label: "Making music",
            description: "Generate music or a sound bed from a prompt through the connected generator and save it on an asset (role media).",
            schema: || obj(
                json!({
                    "asset": asset_ref(),
                    "title": { "type": "string" },
                    "name": { "type": "string" },
                    "prompt": { "type": "string" },
                    "seconds": { "type": "integer" },
                }),
                &["prompt"],
            ),
            attach: true,
            asset_type: "audio",
            approval: None,
        },
        Ability {
            id: "email",
            label: "Send email",
            group: Group::Publish,
            tool: "send_email",
            tool_label: "Sending an email",
            description: "Send an email through the connected mail account. The user sees the recipient, subject and text on a card and confirms first; this call waits for that answer. Attach an asset's files with `asset`.",
            schema: || obj(
                json!({
                    "to": { "type": "string", "description": "Recipient address, or several separated by commas" },
                    "subject": { "type": "string" },
                    "text": { "type": "string", "description": "The body, Markdown" },
                    "asset": asset_ref(),
                }),
                &["to", "subject", "text"],
            ),
            attach: false,
            asset_type: "email",
            approval: Some("email"),
        },
    ]
}

pub fn find(id: &str) -> Option<Ability> {
    all().into_iter().find(|a| a.id == id)
}

impl Ability {
    /// The tool as the model sees it, naming the connection behind it.
    pub fn spec(&self, connection_name: &str, cost: Option<&str>) -> ToolSpec {
        let call = match self.approval {
            Some(k) => CallSpec::Approval { approval: k.to_string() },
            None => CallSpec::Ability { ability: self.id.to_string() },
        };
        ToolSpec {
            name: self.tool.to_string(),
            group: self.group.clone(),
            label: self.tool_label.to_string(),
            description: format!("{} Connection: {connection_name}.", self.description),
            input_schema: (self.schema)(),
            call,
            cost: cost.map(str::to_string),
        }
    }
}

/// Preset sizes, pixels.
pub fn size_preset(name: &str) -> Option<(u32, u32)> {
    Some(match name.trim().to_ascii_lowercase().as_str() {
        "x" | "twitter" => (1600, 900),
        "linkedin" => (1200, 627),
        "instagram" | "square" => (1080, 1080),
        "instagram_portrait" | "portrait" => (1080, 1350),
        "og" | "facebook" => (1200, 630),
        "cover" | "landscape" => (1600, 900),
        other => {
            let (w, h) = other.split_once('x')?;
            (w.trim().parse().ok()?, h.trim().parse().ok()?)
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn abilities_have_schemas_and_sizes() {
        for a in all() {
            assert_eq!((a.schema)()["type"], "object", "{}", a.id);
            let s = a.spec("Built-in renderer", Some("free"));
            assert!(s.description.contains("Built-in renderer"));
        }
        assert_eq!(size_preset("linkedin"), Some((1200, 627)));
        assert_eq!(size_preset("1024x512"), Some((1024, 512)));
        assert_eq!(size_preset("tall"), None);
    }
}
