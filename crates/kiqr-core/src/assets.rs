//! Assets: what users make with Seemo. Articles, posts, replies, images,
//! memes, videos, songs, ad campaigns, and kinds nobody has thought of yet.
//!
//! An asset type is data, not code: `asset-types/<type>/TYPE.yaml` names it,
//! gives it an icon and says which files it holds and how to preview it.
//! Built-in types are installed on first run; users and Seemo add more.
//!
//! An asset is a folder, `projects/<slug>/assets/<type>/<id>/`, holding
//! `asset.yaml` and its files. Assets are generated content, often large
//! binaries, so the whole `assets/` folder is ignored by git and changes are
//! not committed; they only emit events.

use crate::store::Store;
use anyhow::{bail, Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::PathBuf;

/// The built-in types, in the order the UI lists them.
pub const BUILTIN_TYPES: &[(&str, &str)] = &[
    ("article", include_str!("../../../asset-types/article/TYPE.yaml")),
    ("post", include_str!("../../../asset-types/post/TYPE.yaml")),
    ("reply", include_str!("../../../asset-types/reply/TYPE.yaml")),
    ("image", include_str!("../../../asset-types/image/TYPE.yaml")),
    ("meme", include_str!("../../../asset-types/meme/TYPE.yaml")),
    ("video", include_str!("../../../asset-types/video/TYPE.yaml")),
    ("audio", include_str!("../../../asset-types/audio/TYPE.yaml")),
    ("ad-campaign", include_str!("../../../asset-types/ad-campaign/TYPE.yaml")),
    ("email", include_str!("../../../asset-types/email/TYPE.yaml")),
];

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct AssetType {
    pub id: String,
    pub name: String,
    pub plural: String,
    /// A lucide icon name, kebab-case (`newspaper`, `audio-lines`).
    #[serde(default = "default_icon")]
    pub icon: String,
    #[serde(default)]
    pub description: String,
    /// How the UI shows it: article, post, image, video, audio, campaign or files.
    #[serde(default = "default_preview")]
    pub preview: String,
    #[serde(default)]
    pub files: Vec<FileSlot>,
    /// Whether assets of this type carry an image by default: `always` (an
    /// article's cover), `usually` (most posts) or `none`. Seemo reads it;
    /// images are the default for everything that can show one.
    #[serde(default)]
    pub image: ImageDefault,
    /// Shipped with KIQR (not saved in TYPE.yaml).
    #[serde(default, skip_deserializing)]
    pub builtin: bool,
}

fn default_icon() -> String {
    "file".into()
}
fn default_preview() -> String {
    "files".into()
}

/// A kind of file an asset of this type holds.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct FileSlot {
    pub role: String,
    #[serde(default)]
    pub accept: Vec<String>,
}

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ImageDefault {
    Always,
    Usually,
    #[default]
    None,
}

impl AssetType {
    /// The role an image file lands in: the first slot that accepts the
    /// extension (`cover` on an article, `media` on a post), else `media`.
    pub fn role_for(&self, ext: &str) -> String {
        let ext = ext.trim_start_matches('.').to_ascii_lowercase();
        let same = |a: &str| a.eq_ignore_ascii_case(&ext) || (ext == "jpg" && a == "jpeg") || (ext == "jpeg" && a == "jpg");
        self.files.iter().find(|s| s.accept.iter().any(|a| same(a))).map(|s| s.role.clone()).unwrap_or_else(|| "media".into())
    }
}

/// Where an asset stands. Nobody sets it: it follows from the facts on the
/// asset (`channel`, `publish_at`, `publications`, `archived`), see
/// [`Asset::derived_status`]. Draft until the user approves it for a channel
/// (Ready); Scheduled once a publish time is set; Published when it went
/// out; Archived when put away.
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    #[default]
    Draft,
    Ready,
    Scheduled,
    Published,
    Archived,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Asset {
    pub id: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub title: String,
    /// Derived; written for readers of the file and recomputed on every load and save.
    #[serde(default)]
    pub status: Status,
    #[serde(default)]
    pub tags: Vec<String>,
    /// A line or two for cards and lists.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub summary: String,
    pub created: Origin,
    pub updated_at: DateTime<Utc>,
    #[serde(default)]
    pub files: Vec<AssetFile>,
    /// What it was made from: a content gap, a topic cluster, a brief version… Free-form.
    #[serde(default, skip_serializing_if = "serde_json::Map::is_empty")]
    pub source: serde_json::Map<String, serde_json::Value>,
    /// The channel the user approved it for. Set, the asset is Ready.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub channel: Option<String>,
    /// When the daemon publishes it to that channel. Set, the asset is Scheduled.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub publish_at: Option<DateTime<Utc>>,
    /// Why the last scheduled publish did not happen; cleared by the next publish or schedule.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub publish_error: Option<String>,
    /// Put away. Archived assets live in the Archive and count nowhere.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub archived: bool,
    #[serde(default)]
    pub publications: Vec<Publication>,
}

impl Asset {
    /// The status the facts give. The one place that knows the rule.
    pub fn derived_status(&self) -> Status {
        if self.archived {
            Status::Archived
        } else if !self.publications.is_empty() {
            Status::Published
        } else if self.channel.is_some() && self.publish_at.is_some() {
            Status::Scheduled
        } else if self.channel.is_some() {
            Status::Ready
        } else {
            Status::Draft
        }
    }

    /// Bring a loaded asset to the current rule: an older file that said
    /// `status: archived` meant `archived: true`; everything else is derived.
    fn settle(mut self) -> Self {
        if self.status == Status::Archived {
            self.archived = true;
        }
        self.status = self.derived_status();
        self
    }
}

/// Who made an asset, and when. `by` is `user`, `task:<id>` or `chat:<id>`.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Origin {
    pub at: DateTime<Utc>,
    pub by: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct AssetFile {
    pub role: String,
    /// The file name inside the asset folder.
    pub path: String,
    pub bytes: u64,
    pub sha256: String,
}

/// Where an asset went out.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Publication {
    pub connector: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub account: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    pub at: DateTime<Utc>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub run: Option<String>,
}

/// A new asset. Text files can come along; binary files are added with [`write_file`].
#[derive(Clone, Debug, Default, Deserialize)]
pub struct NewAsset {
    #[serde(rename = "type")]
    pub kind: String,
    pub title: String,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub summary: String,
    #[serde(default)]
    pub by: Option<String>,
    #[serde(default)]
    pub source: serde_json::Map<String, serde_json::Value>,
    #[serde(default)]
    pub files: Vec<NewTextFile>,
    /// Shortcut: the body text, saved as `<type>.md` with role `body`.
    #[serde(default)]
    pub text: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct NewTextFile {
    pub role: String,
    pub name: String,
    pub text: String,
}

/// How many assets a project has of one type.
#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct TypeCount {
    #[serde(rename = "type")]
    pub kind: String,
    pub count: usize,
}

/// Install built-in types that are missing. Returns whether anything was written.
pub fn install_builtin_types(store: &Store) -> Result<bool> {
    let mut wrote = false;
    for (id, body) in BUILTIN_TYPES {
        let path = store.home().asset_types_dir().join(id).join("TYPE.yaml");
        if !path.exists() {
            store.write_text(&path, body)?;
            wrote = true;
        }
    }
    Ok(wrote)
}

/// Every type: built-ins first in their order, then the rest by name. A
/// TYPE.yaml that doesn't parse is skipped (built-ins fall back to the shipped copy).
pub fn types(store: &Store) -> Vec<AssetType> {
    let mut out: Vec<AssetType> = Vec::new();
    for (id, body) in BUILTIN_TYPES {
        let path = store.home().asset_types_dir().join(id).join("TYPE.yaml");
        let t = std::fs::read_to_string(&path).ok().and_then(|s| serde_yaml::from_str::<AssetType>(&s).ok()).or_else(|| serde_yaml::from_str(body).ok());
        if let Some(mut t) = t {
            t.id = id.to_string();
            t.builtin = true;
            out.push(t);
        }
    }
    let mut custom: Vec<AssetType> = std::fs::read_dir(store.home().asset_types_dir())
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| {
            let id = e.file_name().to_string_lossy().to_string();
            if BUILTIN_TYPES.iter().any(|(b, _)| *b == id) || !valid_name(&id) {
                return None;
            }
            let mut t: AssetType = serde_yaml::from_str(&std::fs::read_to_string(e.path().join("TYPE.yaml")).ok()?).ok()?;
            t.id = id;
            Some(t)
        })
        .collect();
    custom.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    out.extend(custom);
    out
}

pub fn find_type(store: &Store, id: &str) -> Option<AssetType> {
    types(store).into_iter().find(|t| t.id == id)
}

/// Add or change a type. Returns the saved type.
pub fn save_type(store: &Store, t: &AssetType) -> Result<AssetType> {
    if !valid_name(&t.id) {
        bail!("a type id is lowercase letters, digits and dashes, like `christmas-song`");
    }
    if t.name.trim().is_empty() {
        bail!("a type needs a name");
    }
    let mut t = t.clone();
    if t.plural.trim().is_empty() {
        t.plural = format!("{}s", t.name.trim());
    }
    t.builtin = false;
    let path = store.home().asset_types_dir().join(&t.id).join("TYPE.yaml");
    store.write_yaml(&path, &t)?;
    t.builtin = BUILTIN_TYPES.iter().any(|(b, _)| *b == t.id);
    Ok(t)
}

/// All assets of a project, newest change first.
pub fn list(store: &Store, slug: &str) -> Result<Vec<Asset>> {
    let root = store.home().assets_dir(slug);
    let mut out = Vec::new();
    for kind in std::fs::read_dir(&root).into_iter().flatten().flatten() {
        if !kind.path().is_dir() {
            continue;
        }
        for dir in std::fs::read_dir(kind.path()).into_iter().flatten().flatten() {
            let meta = dir.path().join("asset.yaml");
            match store.read_yaml_opt::<Asset>(&meta) {
                Ok(Some(a)) => out.push(a.settle()),
                Ok(None) => {}
                Err(e) => tracing::warn!(path = %meta.display(), error = %e, "skipping an asset that doesn't parse"),
            }
        }
    }
    out.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
    Ok(out)
}

/// Asset counts per type, most first. Archived assets are not counted: they
/// live in the Archive, not in the library's lists.
pub fn summary(store: &Store, slug: &str) -> Result<Vec<TypeCount>> {
    let mut counts: Vec<TypeCount> = Vec::new();
    for a in list(store, slug)?.into_iter().filter(|a| a.status != Status::Archived) {
        match counts.iter_mut().find(|c| c.kind == a.kind) {
            Some(c) => c.count += 1,
            None => counts.push(TypeCount { kind: a.kind, count: 1 }),
        }
    }
    counts.sort_by(|a, b| b.count.cmp(&a.count).then(a.kind.cmp(&b.kind)));
    Ok(counts)
}

pub fn load(store: &Store, slug: &str, kind: &str, id: &str) -> Result<Asset> {
    let dir = asset_dir(store, slug, kind, id)?;
    store.read_yaml_opt::<Asset>(&dir.join("asset.yaml"))?.with_context(|| format!("no asset {kind}/{id}")).map(Asset::settle)
}

pub fn create(store: &Store, slug: &str, new: NewAsset) -> Result<Asset> {
    if find_type(store, &new.kind).is_none() {
        bail!("no asset type {}; add it first", new.kind);
    }
    let title = new.title.trim();
    if title.is_empty() {
        bail!("an asset needs a title");
    }
    let now = Utc::now();
    let base = slugify(title);
    let mut id = base.clone();
    let mut n = 2;
    while store.home().assets_dir(slug).join(&new.kind).join(&id).exists() {
        id = format!("{base}-{n}");
        n += 1;
    }
    let mut asset = Asset {
        id,
        kind: new.kind,
        title: title.to_string(),
        status: Status::Draft,
        tags: new.tags,
        summary: new.summary,
        created: Origin { at: now, by: new.by.unwrap_or_else(|| "user".into()) },
        updated_at: now,
        files: Vec::new(),
        source: new.source,
        channel: None,
        publish_at: None,
        publish_error: None,
        archived: false,
        publications: Vec::new(),
    };
    save(store, slug, &asset)?;
    let mut files = new.files;
    if let Some(text) = new.text.filter(|t| !t.trim().is_empty()) {
        if !files.iter().any(|f| f.role == "body") {
            files.push(NewTextFile { role: "body".into(), name: format!("{}.md", asset.kind), text: text.trim().to_string() + "\n" });
        }
    }
    for f in files {
        asset = write_file(store, slug, &asset.kind, &asset.id, &f.role, &f.name, f.text.as_bytes())?;
    }
    Ok(asset)
}

/// The name of an asset's body file, if it has one.
pub fn body_file(asset: &Asset) -> Option<&str> {
    asset.files.iter().find(|f| f.role == "body").map(|f| f.path.as_str())
}

/// Save an asset's metadata as given, stamping `updated_at` and the status
/// its facts give.
pub fn save(store: &Store, slug: &str, asset: &Asset) -> Result<Asset> {
    let dir = asset_dir(store, slug, &asset.kind, &asset.id)?;
    let mut a = asset.clone();
    a.updated_at = Utc::now();
    a.status = a.derived_status();
    store.write_yaml(&dir.join("asset.yaml"), &a)?;
    Ok(a)
}

/// Write one file into an asset and record it (replacing a file of the same name).
pub fn write_file(store: &Store, slug: &str, kind: &str, id: &str, role: &str, name: &str, bytes: &[u8]) -> Result<Asset> {
    if !valid_file_name(name) {
        bail!("a file name is letters, digits, dots, dashes and underscores");
    }
    let mut a = load(store, slug, kind, id)?;
    let dir = asset_dir(store, slug, kind, id)?;
    std::fs::write(dir.join(name), bytes)?;
    let sha256 = Sha256::digest(bytes).iter().map(|b| format!("{b:02x}")).collect();
    a.files.retain(|f| f.path != name);
    a.files.push(AssetFile { role: role.to_string(), path: name.to_string(), bytes: bytes.len() as u64, sha256 });
    save(store, slug, &a)
}

/// The path of one of an asset's files, if the asset lists it.
pub fn file_path(store: &Store, slug: &str, kind: &str, id: &str, name: &str) -> Result<PathBuf> {
    let a = load(store, slug, kind, id)?;
    if !a.files.iter().any(|f| f.path == name) {
        bail!("no asset file {name}");
    }
    Ok(asset_dir(store, slug, kind, id)?.join(name))
}

pub fn delete(store: &Store, slug: &str, kind: &str, id: &str) -> Result<()> {
    let dir = asset_dir(store, slug, kind, id)?;
    if !dir.join("asset.yaml").exists() {
        bail!("no asset {kind}/{id}");
    }
    std::fs::remove_dir_all(dir)?;
    Ok(())
}

fn asset_dir(store: &Store, slug: &str, kind: &str, id: &str) -> Result<PathBuf> {
    if !valid_name(kind) || !valid_name(id) {
        bail!("no asset {kind}/{id}");
    }
    Ok(store.home().assets_dir(slug).join(kind).join(id))
}

/// Type ids and asset ids: lowercase letters, digits and dashes.
pub fn valid_name(s: &str) -> bool {
    !s.is_empty() && s.len() <= 80 && s.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-') && !s.starts_with('-')
}

fn valid_file_name(s: &str) -> bool {
    !s.is_empty() && s.len() <= 120 && !s.starts_with('.') && s != "asset.yaml" && s.bytes().all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b))
}

/// `Day 3 — Santa's whitening kit!` → `day-3-santas-whitening-kit`. Non-ASCII
/// letters are folded where common (å→a, ö→o) and dropped otherwise.
pub fn slugify(s: &str) -> String {
    let mut out = String::new();
    for c in s.to_lowercase().chars() {
        let c = match c {
            'å' | 'ä' | 'à' | 'á' | 'â' => 'a',
            'ö' | 'ø' | 'ó' | 'ò' | 'ô' => 'o',
            'é' | 'è' | 'ê' | 'ë' => 'e',
            'ü' | 'ú' | 'ù' => 'u',
            'í' | 'ì' | 'ï' => 'i',
            'ñ' => 'n',
            'ç' => 'c',
            c => c,
        };
        if c.is_ascii_alphanumeric() {
            out.push(c);
        } else if c == '\'' || c == '’' {
            continue;
        } else if !out.ends_with('-') && !out.is_empty() {
            out.push('-');
        }
    }
    let out: String = out.trim_end_matches('-').chars().take(60).collect();
    let out = out.trim_end_matches('-').to_string();
    if out.is_empty() {
        "untitled".into()
    } else {
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::paths::Home;

    fn store() -> (Store, PathBuf) {
        let dir = std::env::temp_dir().join(format!("kiqr-assets-{}", uuid::Uuid::new_v4()));
        let store = Store::open(Home::at(&dir)).unwrap();
        install_builtin_types(&store).unwrap();
        (store, dir)
    }

    #[test]
    fn builtin_types_parse() {
        for (id, body) in BUILTIN_TYPES {
            let t: AssetType = serde_yaml::from_str(body).unwrap_or_else(|e| panic!("{id}: {e}"));
            assert_eq!(&t.id, id);
            assert!(!t.files.is_empty(), "{id} lists its files");
        }
    }

    #[test]
    fn slugs() {
        assert_eq!(slugify("Day 3 — Santa's whitening kit!"), "day-3-santas-whitening-kit");
        assert_eq!(slugify("Vita tänder på 5 minuter"), "vita-tander-pa-5-minuter");
        assert_eq!(slugify("!!!"), "untitled");
    }

    #[test]
    fn round_trip_and_custom_types() {
        let (store, dir) = store();
        let song = AssetType {
            id: "christmas-song".into(),
            name: "Christmas song".into(),
            plural: String::new(),
            icon: "music".into(),
            description: String::new(),
            preview: "audio".into(),
            files: vec![FileSlot { role: "audio".into(), accept: vec!["mp3".into()] }],
            image: ImageDefault::None,
            builtin: false,
        };
        save_type(&store, &song).unwrap();
        let all = types(&store);
        assert_eq!(all[0].id, "article");
        // Images are the default: an article's cover, a post's media, and the type says so.
        assert_eq!(all[0].image, ImageDefault::Always);
        assert_eq!(all[0].role_for("png"), "cover");
        let post = all.iter().find(|t| t.id == "post").unwrap();
        assert_eq!(post.image, ImageDefault::Always);
        assert_eq!(post.role_for("jpeg"), "media");
        assert_eq!(all.last().unwrap().role_for("png"), "media");
        assert_eq!(all.last().unwrap().plural, "Christmas songs");

        let a = create(
            &store,
            "acme",
            NewAsset {
                kind: "article".into(),
                title: "Whitening at home".into(),
                files: vec![NewTextFile { role: "body".into(), name: "article.md".into(), text: "# Hi".into() }],
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(a.id, "whitening-at-home");
        assert_eq!(a.files[0].bytes, 4);
        let b = create(&store, "acme", NewAsset { kind: "article".into(), title: "Whitening at home".into(), ..Default::default() }).unwrap();
        assert_eq!(b.id, "whitening-at-home-2");
        write_file(&store, "acme", "christmas-song", "x", "audio", "a.mp3", b"x").unwrap_err();
        create(&store, "acme", NewAsset { kind: "christmas-song".into(), title: "Jingle".into(), ..Default::default() }).unwrap();

        assert_eq!(list(&store, "acme").unwrap().len(), 3);
        assert_eq!(summary(&store, "acme").unwrap()[0], TypeCount { kind: "article".into(), count: 2 });
        assert_eq!(load(&store, "acme", "article", "whitening-at-home").unwrap().files.len(), 1);
        assert!(file_path(&store, "acme", "article", "whitening-at-home", "../../project.yaml").is_err());
        assert!(load(&store, "acme", "..", "x").is_err());
        delete(&store, "acme", "article", "whitening-at-home-2").unwrap();
        assert_eq!(list(&store, "acme").unwrap().len(), 2);

        // Status follows the facts; nobody sets it.
        let mut a = load(&store, "acme", "article", "whitening-at-home").unwrap();
        assert_eq!(a.status, Status::Draft);
        a.channel = Some("blog".into());
        let a = save(&store, "acme", &a).unwrap();
        assert_eq!(a.status, Status::Ready);
        let mut a = a;
        a.publish_at = Some(Utc::now() + chrono::Duration::hours(1));
        assert_eq!(save(&store, "acme", &a).unwrap().status, Status::Scheduled);
        a.publications.push(Publication { connector: "blog".into(), account: None, url: None, at: Utc::now(), run: None });
        assert_eq!(save(&store, "acme", &a).unwrap().status, Status::Published);
        a.archived = true;
        assert_eq!(save(&store, "acme", &a).unwrap().status, Status::Archived);
        assert_eq!(summary(&store, "acme").unwrap().iter().find(|c| c.kind == "article"), None);
        // A file from before `archived` existed said so through the status.
        let path = store.home().assets_dir("acme").join("article").join("whitening-at-home").join("asset.yaml");
        let text = std::fs::read_to_string(&path).unwrap().replace("archived: true\n", "").replace("status: archived", "status: archived");
        std::fs::write(&path, text).unwrap();
        let a = load(&store, "acme", "article", "whitening-at-home").unwrap();
        assert!(a.archived);
        assert_eq!(a.status, Status::Archived);

        // Generated content stays out of the home's git history.
        let ignored = std::process::Command::new("git").args(["check-ignore", "-q", "projects/acme/assets/article/whitening-at-home/article.md"]).current_dir(&dir).status().unwrap();
        assert!(ignored.success());
        std::fs::remove_dir_all(dir).ok();
    }
}
