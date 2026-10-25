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
            description: "Make an image and save it as a file on an asset (role media). Two shapes, depending on the connection behind it: the built-in renderer takes `source` (SVG, or HTML when a browser is available) and renders it; a generator takes `prompt`. `size` is a channel kind the project publishes to (its network's image size; `brand_kit` lists them), a preset (landscape, square, portrait, og, cover) or WxH. The file lands on the asset at once and returns its name and URL; the user sees the whole asset, image included, in the chat and approves it there or in the library.",
            schema: || obj(
                json!({
                    "asset": asset_ref(),
                    "title": { "type": "string", "description": "Title for a new standalone image asset (when `asset` is omitted)" },
                    "name": { "type": "string", "description": "File name without extension, e.g. cover (default: image)" },
                    "prompt": { "type": "string", "description": "What to generate (generator connections)" },
                    "source": { "type": "string", "description": "SVG or HTML to render (built-in renderer)" },
                    "format": { "type": "string", "enum": ["svg", "html"], "description": "Which `source` is (built-in renderer; default svg)" },
                    "size": { "type": "string", "description": "a channel kind from brand_kit sizes | landscape | square | portrait | og | cover | WxH" },
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

/// The generic presets, pixels. A network's size is not here: it comes
/// from the channel kind that declares it (see [`size_for`]).
pub const PRESETS: &[(&str, u32, u32)] = &[("landscape", 1600, 900), ("square", 1080, 1080), ("portrait", 1080, 1350), ("og", 1200, 630), ("cover", 1600, 900)];

/// A preset or `WxH`, pixels.
pub fn size_preset(name: &str) -> Option<(u32, u32)> {
    let name = name.trim().to_ascii_lowercase();
    if let Some((_, w, h)) = PRESETS.iter().find(|(n, _, _)| *n == name) {
        return Some((*w, *h));
    }
    Some(match name.as_str() {
        other => {
            let (w, h) = other.split_once('x')?;
            (w.trim().parse().ok()?, h.trim().parse().ok()?)
        }
    })
}

/// A size by name: a channel kind's own image size first, then a preset or `WxH`.
pub fn size_for(name: &str, kinds: &[crate::extensions::ChannelKind]) -> Option<(u32, u32)> {
    let n = name.trim().to_ascii_lowercase();
    kinds.iter().find(|k| k.kind == n).and_then(|k| k.image).map(|i| (i.width, i.height)).or_else(|| size_preset(name))
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
        assert_eq!(size_preset("square"), Some((1080, 1080)));
        let network = crate::extensions::ChannelKind { kind: "somenet".into(), image: Some(crate::extensions::ImageSize { width: 1200, height: 627 }), ..Default::default() };
        assert_eq!(size_for("somenet", &[network.clone()]), Some((1200, 627)));
        assert_eq!(size_for("og", &[network]), Some((1200, 630)));
        assert_eq!(size_preset("somenet"), None);
        assert_eq!(size_preset("1024x512"), Some((1024, 512)));
        assert_eq!(size_preset("tall"), None);
    }
}
