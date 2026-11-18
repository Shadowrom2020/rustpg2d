//! Plugins: shareable add-ons that stay apart from the project's own content and can be updated.
//!
//! A plugin is a folder `plugins/<id>/` of a project (or of the asset library) with a
//! `plugin.ron` ([`PluginDef`]: name, version, author, the script API it needs, typed parameters,
//! script hooks, menu entries, HUD elements), its scripts (`scripts/*.rhai`) and its own
//! definitions and art (`defs/*.ron`, files under its folder). `ProjectConfig::plugins` lists the
//! project's plugins in load order ([`PluginRef`]: on / off and the parameters' values).
//!
//! The game adds the enabled ones when it starts ([`apply`]): their hooks, menu entries and
//! definitions join the project's (definitions the project has keep the project's), their
//! scripts are loaded after the project's and read parameters with `plugin_param(plugin, name)`.
//! The editor never merges them into the project's files: they stay in their folder, and
//! replacing the folder updates a plugin. Shared as a `.r2dplugin` file: the `data.pack` format
//! with the folder's files under `<id>/`.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};

use crate::hooks::HookDef;
use crate::menus::MenuEntry;
use crate::project::Project;
use crate::script_ui::HudElement;

/// A project's (or the library's) plugin folders.
pub const PLUGINS_DIR: &str = "plugins";
/// A plugin's description, in its folder.
pub const PLUGIN_FILE: &str = "plugin.ron";
/// The extension of shared plugin files.
pub const EXTENSION: &str = "r2dplugin";

/// What a parameter holds; the editor shows the matching field.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum ParamKind {
    #[default]
    Number,
    Text,
    /// On / off.
    Flag,
    /// A sprite sheet's id.
    Sprite,
    /// A sound's id.
    Sound,
    /// An item's id.
    Item,
    /// A map's name.
    Map,
    /// A colour: a name the dialogs know or `#rrggbb`.
    Color,
}

impl ParamKind {
    pub const ALL: [ParamKind; 8] = [ParamKind::Number, ParamKind::Text, ParamKind::Flag, ParamKind::Sprite, ParamKind::Sound, ParamKind::Item, ParamKind::Map, ParamKind::Color];

    pub fn label(self) -> &'static str {
        match self {
            ParamKind::Number => "number",
            ParamKind::Text => "text",
            ParamKind::Flag => "on/off",
            ParamKind::Sprite => "sprite",
            ParamKind::Sound => "sound",
            ParamKind::Item => "item",
            ParamKind::Map => "map",
            ParamKind::Color => "colour",
        }
    }

    /// The value a parameter of this kind starts from when its plugin gives none.
    pub fn empty(self) -> ParamValue {
        match self {
            ParamKind::Number => ParamValue::Number(0.0),
            ParamKind::Flag => ParamValue::Flag(false),
            _ => ParamValue::Text(String::new()),
        }
    }

    fn fits(self, v: &ParamValue) -> bool {
        matches!((self, v), (ParamKind::Number, ParamValue::Number(_)) | (ParamKind::Flag, ParamValue::Flag(_)))
            || (!matches!(self, ParamKind::Number | ParamKind::Flag) && matches!(v, ParamValue::Text(_)))
    }
}

/// A parameter's value: a number, on / off or a text (ids, colours).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ParamValue {
    Flag(bool),
    Number(f64),
    Text(String),
}

impl ParamValue {
    /// For `plugin_param()`: whole numbers as integers, others as floats.
    pub fn to_dynamic(&self) -> rhai::Dynamic {
        match self {
            ParamValue::Flag(b) => (*b).into(),
            ParamValue::Number(n) if n.fract() == 0.0 && n.abs() < 9.0e15 => (*n as i64).into(),
            ParamValue::Number(n) => (*n).into(),
            ParamValue::Text(s) => s.clone().into(),
        }
    }

    /// As shown in lists and `status`.
    pub fn show(&self) -> String {
        match self {
            ParamValue::Flag(b) => if *b { "on" } else { "off" }.into(),
            ParamValue::Number(n) => format!("{n}"),
            ParamValue::Text(s) => s.clone(),
        }
    }
}

/// A parameter a plugin offers (filled in the editor's *Plugins* window).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ParamDef {
    /// What `plugin_param(plugin, name)` asks for.
    pub name: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub label: String,
    #[serde(default)]
    pub kind: ParamKind,
    /// None = the kind's empty value.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default: Option<ParamValue>,
    /// Numbers: the range the editor offers (both 0 = any).
    #[serde(default, skip_serializing_if = "is_zero")]
    pub min: f64,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub max: f64,
    /// A sentence for the editor's tooltip.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub help: String,
}

fn is_zero(v: &f64) -> bool {
    *v == 0.0
}

impl ParamDef {
    pub fn default_value(&self) -> ParamValue {
        self.default.clone().filter(|v| self.kind.fits(v)).unwrap_or_else(|| self.kind.empty())
    }

    /// The value a project set, when it suits the kind, else the default.
    pub fn value(&self, set: Option<&ParamValue>) -> ParamValue {
        match set {
            Some(v) if self.kind.fits(v) => v.clone(),
            _ => self.default_value(),
        }
    }

    pub fn shown_label(&self) -> &str {
        if self.label.is_empty() { &self.name } else { &self.label }
    }
}

/// `plugin.ron`: what a plugin is and brings.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct PluginDef {
    pub name: String,
    /// `1.2.0`: a newer version replaces an older one when added again.
    #[serde(default)]
    pub version: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub author: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub description: String,
    /// The script API version it needs (`0.27`); "" = any. A newer one than the engine's
    /// ([`crate::script_api::API_VERSION`]) keeps the plugin out.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub api: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub parameters: Vec<ParamDef>,
    /// Script hooks its functions are called at (with the project's, by priority).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub hooks: Vec<HookDef>,
    /// Entries added to the title screen (before *Exit*) and the pause menu (before *Save and
    /// quit*): usually `Custom` ones running a function of the plugin.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub title_entries: Vec<MenuEntry>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub pause_entries: Vec<MenuEntry>,
    /// HUD elements shown while it is on (like scripts' `hud_text`, but not saved), by id.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub hud: BTreeMap<String, HudElement>,
}

/// A plugin of a project: its folder's id, whether it is on, the parameters' values.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PluginRef {
    pub id: String,
    #[serde(default = "yes")]
    pub enabled: bool,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub parameters: BTreeMap<String, ParamValue>,
}

fn yes() -> bool {
    true
}

impl PluginRef {
    pub fn new(id: &str) -> Self {
        PluginRef { id: id.to_string(), enabled: true, parameters: BTreeMap::new() }
    }
}

/// `plugins/<id>` of a project or library folder.
pub fn plugin_dir(root: &Path, id: &str) -> PathBuf {
    root.join(PLUGINS_DIR).join(id)
}

/// Whether `id` can name a plugin folder (letters, digits, `_`, `-`).
pub fn valid_id(id: &str) -> bool {
    !id.is_empty() && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

/// Reads `plugins/<id>/plugin.ron` (also from an exported game's pack).
pub fn read_def(root: &Path, id: &str) -> Result<PluginDef> {
    let path = plugin_dir(root, id).join(PLUGIN_FILE);
    let text = crate::sys::read_to_string(&path).with_context(|| format!("reading {}", path.display()))?;
    ron::from_str(&text).with_context(|| format!("reading {}", path.display()))
}

/// Writes `plugins/<id>/plugin.ron`.
pub fn write_def(root: &Path, id: &str, def: &PluginDef) -> Result<()> {
    let dir = plugin_dir(root, id);
    std::fs::create_dir_all(&dir)?;
    std::fs::write(dir.join(PLUGIN_FILE), crate::project::ron_text(def)?)?;
    Ok(())
}

/// The plugin folders of a project or library (with a `plugin.ron`), sorted.
pub fn installed(root: &Path) -> Vec<String> {
    let mut ids: Vec<String> = std::fs::read_dir(root.join(PLUGINS_DIR))
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| e.path().join(PLUGIN_FILE).is_file())
        .filter_map(|e| e.file_name().to_str().map(str::to_string))
        .filter(|id| valid_id(id))
        .collect();
    ids.sort();
    ids
}

/// "0.27" → (0, 27); missing parts are 0.
fn parts(v: &str) -> Vec<u64> {
    v.trim().split('.').map(|p| p.trim().parse::<u64>().unwrap_or(0)).collect()
}

/// Compares dotted versions ("1.10" is newer than "1.9").
pub fn compare_versions(a: &str, b: &str) -> std::cmp::Ordering {
    let (mut x, mut y) = (parts(a), parts(b));
    let n = x.len().max(y.len());
    x.resize(n, 0);
    y.resize(n, 0);
    x.cmp(&y)
}

/// Whether the engine offers the script API a plugin needs ("" = any).
pub fn api_fits(api: &str) -> bool {
    api.trim().is_empty() || compare_versions(api, crate::script_api::API_VERSION) != std::cmp::Ordering::Greater
}

/// The enabled plugins of a project in load order with their definitions; plugins that are
/// missing, unreadable or need a newer script API are left out with a warning.
pub fn enabled(project: &Project) -> (Vec<(String, PluginDef)>, Vec<String>) {
    let mut out = Vec::new();
    let mut warnings = Vec::new();
    for r in project.config.plugins.iter().filter(|r| r.enabled) {
        match read_def(&project.root, &r.id) {
            Ok(d) if !api_fits(&d.api) => warnings.push(format!("the plugin '{}' needs script API {} (this engine has {}): left out", r.id, d.api, crate::script_api::API_VERSION)),
            Ok(d) => out.push((r.id.clone(), d)),
            Err(e) => warnings.push(format!("the plugin '{}' can't be read: {e:#}", r.id)),
        }
    }
    (out, warnings)
}

/// What [`apply`] added that the game keeps apart.
#[derive(Debug, Default)]
pub struct Applied {
    /// Enabled plugins in load order (`status` `plugins=`).
    pub ids: Vec<String>,
    /// Their script folders, loaded after the project's.
    pub script_dirs: Vec<PathBuf>,
    /// The parameters' values per plugin (`plugin_param`).
    pub params: BTreeMap<String, BTreeMap<String, ParamValue>>,
    /// Their HUD elements (by "<plugin>/<id>").
    pub hud: BTreeMap<String, HudElement>,
    pub warnings: Vec<String>,
}

/// Adds the enabled plugins to a project the game is about to play: hooks, menu entries,
/// definitions the project doesn't have (their files are found in the plugin's folder). The
/// project on disk is not touched.
pub fn apply(project: &mut Project) -> Applied {
    let (list, warnings) = enabled(project);
    let mut applied = Applied { warnings, ..Default::default() };
    for (id, def) in list {
        let dir = plugin_dir(&project.root, &id);
        project.config.hooks.extend(def.hooks.iter().cloned());
        add_entries(&mut project.config.title.entries, &def.title_entries, crate::menus::TitleScreen::DEFAULT.as_slice(), &[MenuEntry::Exit]);
        add_entries(&mut project.config.menu.entries, &def.pause_entries, crate::menus::PauseMenu::DEFAULT.as_slice(), &[MenuEntry::SaveAndQuit, MenuEntry::QuitToTitle]);
        match plugin_defs(&dir, &format!("{PLUGINS_DIR}/{id}")) {
            Ok(db) => merge_missing(&mut project.db, db),
            Err(e) => applied.warnings.push(format!("the plugin '{id}''s definitions can't be read: {e:#}")),
        }
        let set = project.config.plugins.iter().find(|r| r.id == id).map(|r| r.parameters.clone()).unwrap_or_default();
        applied.params.insert(id.clone(), def.parameters.iter().map(|p| (p.name.clone(), p.value(set.get(&p.name)))).collect());
        for (k, e) in &def.hud {
            applied.hud.insert(format!("{id}/{k}"), e.clone());
        }
        applied.script_dirs.push(dir.join("scripts"));
        applied.ids.push(id);
    }
    project.db.reindex();
    for w in &applied.warnings {
        eprintln!("[plugins] {w}");
    }
    applied
}

/// Inserts `new` entries into a menu (its built-in entries first when it has none of its own),
/// before the first of `before` (*Exit*, *Save and quit*), else at the end.
fn add_entries(entries: &mut Vec<MenuEntry>, new: &[MenuEntry], builtin: &[MenuEntry], before: &[MenuEntry]) {
    if new.is_empty() {
        return;
    }
    if entries.is_empty() {
        *entries = builtin.to_vec();
    }
    let at = entries.iter().position(|e| before.contains(e)).unwrap_or(entries.len());
    for (k, e) in new.iter().enumerate() {
        entries.insert(at + k, e.clone());
    }
}

/// A plugin folder's definitions, with the files they name made relative to the project
/// (`prefix` = `plugins/<id>`).
pub fn plugin_defs(dir: &Path, prefix: &str) -> Result<crate::defs::Database> {
    let defs = dir.join("defs");
    let mut db = crate::project::parse_defs(|name| {
        let p = defs.join(format!("{name}.ron"));
        if crate::sys::exists(&p) {
            Ok(Some(crate::sys::read_to_string(&p).with_context(|| format!("reading {}", p.display()))?))
        } else {
            Ok(None)
        }
    })?;
    let fix = |f: &mut String| {
        if !f.is_empty() {
            *f = format!("{prefix}/{f}");
        }
    };
    db.sheets.iter_mut().for_each(|d| fix(&mut d.image));
    db.sounds.iter_mut().for_each(|d| fix(&mut d.file));
    db.materials.iter_mut().for_each(|d| fix(&mut d.texture));
    for d in &mut db.fonts {
        fix(&mut d.file);
        fix(&mut d.license);
    }
    Ok(db)
}

/// Adds the definitions of `from` that `db` doesn't have (the project's own win).
pub fn merge_missing(db: &mut crate::defs::Database, from: crate::defs::Database) {
    use crate::defs::Def;
    for kind in crate::refactor::RefKind::ALL {
        crate::with_def_kind!(
            kind,
            |T| {
                for d in T::list(&from) {
                    if db.get::<T>(d.id()).is_none() {
                        T::list_mut(db).push(d.clone());
                    }
                }
            },
            else => {}
        );
    }
}

/// Every file of a plugin folder: paths relative to the folder (`/` separators), sorted.
pub fn folder_files(root: &Path, id: &str) -> Result<Vec<String>> {
    crate::pack::dir_files(&plugin_dir(root, id))
}

/// A `.r2dplugin` of `plugins/<id>`: the folder's files under `<id>/`.
pub fn pack(root: &Path, id: &str) -> Result<Vec<u8>> {
    let dir = plugin_dir(root, id);
    if !dir.join(PLUGIN_FILE).is_file() {
        bail!("{} has no {PLUGIN_FILE}", dir.display());
    }
    let files: Vec<(String, Vec<u8>)> = crate::pack::read_dir_files(&dir)?.into_iter().map(|(rel, b)| (format!("{id}/{rel}"), b)).collect();
    crate::pack::write(&files)
}

/// Files with their paths relative to a folder.
pub type Files = Vec<(String, Vec<u8>)>;

/// A plugin's id, definition and files (relative to its folder) from a `.r2dplugin`.
pub fn unpack(bytes: &[u8]) -> Result<(String, PluginDef, Files)> {
    let files = crate::pack::read_all(bytes).context("not a plugin file")?;
    let id = files.iter().find_map(|(p, _)| p.strip_suffix(&format!("/{PLUGIN_FILE}")).filter(|id| !id.contains('/')).map(str::to_string)).context("the file holds no plugin.ron")?;
    if !valid_id(&id) {
        bail!("'{id}' is no plugin id");
    }
    let mut out = Vec::new();
    for (p, b) in files {
        let Some(rel) = p.strip_prefix(&format!("{id}/")) else { bail!("{p} is outside the plugin's folder") };
        if !crate::project::safe_rel(rel) {
            bail!("unsafe path in the plugin: {p}");
        }
        out.push((rel.to_string(), b));
    }
    let def_text = out.iter().find(|(p, _)| p == PLUGIN_FILE).map(|(_, b)| String::from_utf8_lossy(b).into_owned()).unwrap_or_default();
    let def: PluginDef = ron::from_str(&def_text).context("reading its plugin.ron")?;
    Ok((id, def, out))
}

/// What installing a plugin did.
#[derive(Debug, PartialEq)]
pub struct Installed {
    pub id: String,
    pub name: String,
    pub version: String,
    /// The version it replaced (None: new).
    pub replaced: Option<String>,
}

/// Puts a plugin's files into `root/plugins/<id>` (replacing the folder) and lists it in the
/// project (keeping its order, on / off and parameter values when it was there).
pub fn install(project: &mut Project, id: &str, def: &PluginDef, files: &[(String, Vec<u8>)]) -> Result<Installed> {
    let dir = plugin_dir(&project.root, id);
    let replaced = read_def(&project.root, id).ok().map(|d| d.version);
    if dir.exists() {
        std::fs::remove_dir_all(&dir).with_context(|| format!("removing {}", dir.display()))?;
    }
    for (rel, b) in files {
        let p = dir.join(rel);
        if let Some(parent) = p.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&p, b).with_context(|| format!("writing {}", p.display()))?;
    }
    if !project.config.plugins.iter().any(|r| r.id == id) {
        project.config.plugins.push(PluginRef::new(id));
    }
    Ok(Installed { id: id.to_string(), name: def.name.clone(), version: def.version.clone(), replaced })
}

/// Installs the plugin of another folder (the asset library's `plugins/<id>`).
pub fn install_from(project: &mut Project, from_root: &Path, id: &str) -> Result<Installed> {
    let def = read_def(from_root, id)?;
    let files = crate::pack::read_dir_files(&plugin_dir(from_root, id))?;
    install(project, id, &def, &files)
}

/// Removes a plugin: its folder and its entry.
pub fn remove(project: &mut Project, id: &str) -> Result<()> {
    let dir = plugin_dir(&project.root, id);
    if dir.exists() {
        std::fs::remove_dir_all(&dir).with_context(|| format!("removing {}", dir.display()))?;
    }
    project.config.plugins.retain(|r| r.id != id);
    Ok(())
}

/// Copies the starter set's plugins into a library folder: those it lacks and newer versions of
/// those it has (an edited plugin of the same version stays).
pub fn sync_from_files(files: &crate::library::LibraryFiles, root: &Path) -> Result<Vec<String>> {
    let mut by_id: BTreeMap<&str, Vec<(&str, &[u8])>> = BTreeMap::new();
    for (path, bytes) in files {
        if let Some(rest) = path.strip_prefix(&format!("{PLUGINS_DIR}/")) {
            if let Some((id, rel)) = rest.split_once('/') {
                by_id.entry(id).or_default().push((rel, bytes));
            }
        }
    }
    let mut changed = Vec::new();
    for (id, list) in by_id {
        let Some(def) = list.iter().find(|(r, _)| *r == PLUGIN_FILE).and_then(|(_, b)| ron::from_str::<PluginDef>(&String::from_utf8_lossy(b)).ok()) else { continue };
        let have = read_def(root, id).ok();
        if have.as_ref().is_some_and(|h| compare_versions(&def.version, &h.version) != std::cmp::Ordering::Greater) {
            continue;
        }
        let dir = plugin_dir(root, id);
        if dir.exists() {
            std::fs::remove_dir_all(&dir)?;
        }
        for (rel, b) in list {
            if !crate::project::safe_rel(rel) {
                continue;
            }
            let p = dir.join(rel);
            if let Some(parent) = p.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::write(&p, b)?;
        }
        changed.push(id.to_string());
    }
    Ok(changed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn versions_and_api() {
        use std::cmp::Ordering::*;
        assert_eq!(compare_versions("1.10", "1.9"), Greater);
        assert_eq!(compare_versions("1.0", "1"), Equal);
        assert_eq!(compare_versions("0.9.1", "0.10"), Less);
        assert!(api_fits("") && api_fits("0.1") && api_fits(crate::script_api::API_VERSION));
        assert!(!api_fits("99.0"));
    }

    #[test]
    fn parameters_fall_back_to_their_defaults() {
        let p = ParamDef { name: "times".into(), label: String::new(), kind: ParamKind::Number, default: Some(ParamValue::Number(2.0)), min: 0.0, max: 0.0, help: String::new() };
        assert_eq!(p.value(None), ParamValue::Number(2.0));
        assert_eq!(p.value(Some(&ParamValue::Number(3.5))), ParamValue::Number(3.5));
        assert_eq!(p.value(Some(&ParamValue::Text("x".into()))), ParamValue::Number(2.0));
        assert_eq!(ParamValue::Number(3.0).to_dynamic().as_int().ok(), Some(3));
        assert!(ParamValue::Number(1.5).to_dynamic().is_float());
        let f = ParamDef { kind: ParamKind::Flag, default: None, ..p };
        assert_eq!(f.value(None), ParamValue::Flag(false));
        // The untagged values read back as they were written.
        let v: BTreeMap<String, ParamValue> = ron::from_str(r#"{"a": true, "b": 2.5, "c": "gold"}"#).unwrap();
        assert_eq!(v["a"], ParamValue::Flag(true));
        assert_eq!(v["b"], ParamValue::Number(2.5));
        assert_eq!(v["c"], ParamValue::Text("gold".into()));
    }

    #[test]
    fn menu_entries_go_before_exit() {
        let custom = MenuEntry::Custom { label: "Calendar".into(), function: "cal".into(), detail: String::new() };
        let mut pause = Vec::new();
        add_entries(&mut pause, std::slice::from_ref(&custom), crate::menus::PauseMenu::DEFAULT.as_slice(), &[MenuEntry::SaveAndQuit, MenuEntry::QuitToTitle]);
        let at = pause.iter().position(|e| *e == custom).unwrap();
        assert_eq!(pause[at + 1], MenuEntry::SaveAndQuit);
        assert_eq!(pause.len(), crate::menus::PauseMenu::DEFAULT.len() + 1);
    }

    #[test]
    fn starter_plugins_reach_libraries_by_version() {
        let root = std::env::temp_dir().join(format!("rustpg2d_plugin_sync_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let v = |n: &str| format!("(name: \"Calendar\", version: \"{n}\")").into_bytes();
        let (one, two) = (v("1.0"), v("1.1"));
        let files: Vec<(&str, &[u8])> = vec![("plugins/calendar/plugin.ron", &one), ("plugins/calendar/scripts/c.rhai", b"fn c() {}"), ("defs/items.ron", b"[]")];
        assert_eq!(sync_from_files(&files, &root).unwrap(), ["calendar"]);
        assert!(plugin_dir(&root, "calendar").join("scripts/c.rhai").is_file());
        // The same version again: the library's (maybe edited) copy stays.
        std::fs::write(plugin_dir(&root, "calendar").join("scripts/c.rhai"), "fn c() { 1 }").unwrap();
        assert!(sync_from_files(&files, &root).unwrap().is_empty());
        assert_eq!(std::fs::read_to_string(plugin_dir(&root, "calendar").join("scripts/c.rhai")).unwrap(), "fn c() { 1 }");
        // A newer one replaces it.
        let files: Vec<(&str, &[u8])> = vec![("plugins/calendar/plugin.ron", &two), ("plugins/calendar/scripts/c.rhai", b"fn c() {}")];
        assert_eq!(sync_from_files(&files, &root).unwrap(), ["calendar"]);
        assert_eq!(read_def(&root, "calendar").unwrap().version, "1.1");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_plugin_packs_installs_and_applies() {
        let root = std::env::temp_dir().join(format!("rustpg2d_plugin_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let src = root.join("src");
        let def = PluginDef {
            name: "Test".into(),
            version: "1.0".into(),
            api: "0.27".into(),
            parameters: vec![ParamDef { name: "times".into(), label: String::new(), kind: ParamKind::Number, default: Some(ParamValue::Number(2.0)), min: 0.0, max: 0.0, help: String::new() }],
            hooks: vec![HookDef { hook: "damage".into(), function: "test_times".into(), priority: 0 }],
            ..Default::default()
        };
        write_def(&src, "test", &def).unwrap();
        std::fs::create_dir_all(plugin_dir(&src, "test").join("scripts")).unwrap();
        std::fs::write(plugin_dir(&src, "test").join("scripts/test.rhai"), "fn test_times(m) { m }").unwrap();
        std::fs::create_dir_all(plugin_dir(&src, "test").join("defs")).unwrap();
        std::fs::write(plugin_dir(&src, "test").join("defs/sounds.ron"), r#"[(id: "ding", file: "sounds/ding.wav")]"#).unwrap();
        let bytes = pack(&src, "test").unwrap();
        let (id, back, files) = unpack(&bytes).unwrap();
        assert_eq!((id.as_str(), &back), ("test", &def));
        assert!(files.iter().any(|(p, _)| p == "scripts/test.rhai"));
        let proj_root = root.join("proj");
        std::fs::create_dir_all(&proj_root).unwrap();
        let mut project = Project { root: proj_root.clone(), config: crate::project::ProjectConfig::new("P"), db: Default::default() };
        let i = install(&mut project, &id, &back, &files).unwrap();
        assert_eq!(i.replaced, None);
        assert_eq!(project.config.plugins, vec![PluginRef::new("test")]);
        project.config.plugins[0].parameters.insert("times".into(), ParamValue::Number(3.0));
        let applied = apply(&mut project);
        assert!(applied.warnings.is_empty(), "{:?}", applied.warnings);
        assert_eq!(applied.ids, ["test"]);
        assert_eq!(applied.params["test"]["times"], ParamValue::Number(3.0));
        assert_eq!(project.config.hooks.len(), 1);
        assert_eq!(project.db.sound("ding").map(|s| s.file.as_str()), Some("plugins/test/sounds/ding.wav"));
        // A newer version replaces it; the project's settings stay.
        let newer = PluginDef { version: "1.1".into(), ..def.clone() };
        let i = install(&mut project, "test", &newer, &files).unwrap();
        assert_eq!(i.replaced.as_deref(), Some("1.0"));
        assert_eq!(project.config.plugins.len(), 1);
        // Too new for this engine: left out.
        write_def(&proj_root, "test", &PluginDef { api: "99.0".into(), ..def }).unwrap();
        let mut again = Project { root: proj_root.clone(), config: project.config.clone(), db: Default::default() };
        let applied = apply(&mut again);
        assert!(applied.ids.is_empty() && !applied.warnings.is_empty());
        remove(&mut project, "test").unwrap();
        assert!(project.config.plugins.is_empty() && !plugin_dir(&proj_root, "test").exists());
        let _ = std::fs::remove_dir_all(&root);
    }
}
