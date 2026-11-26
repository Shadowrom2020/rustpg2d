//! The field schema: what the editor knows about every field of the engine's data.
//!
//! Two sources. The **shape** comes from the engine's own types (`rustpg2d_engine::schema`, JSON
//! Schemas derived from them): a definition's fields in their order, what each holds, an enum's
//! variants, the value a file may leave out, and the field's doc comment. What only an editor
//! cares about is written here, in small tables: the label a creator reads ([`label`]), the
//! sections of a definition's form and which fields are for advanced use ([`sections`]), and
//! which fields name another definition ([`reference`]), so the form shows a picker there.
//!
//! Forms, pickers, field help and (later) table columns and the manual's reference tables are
//! made from this. A test keeps the tables complete: every field of every definition kind has
//! its section, and every field the engine's renames follow is known as a reference.

use std::collections::HashMap;
use std::sync::OnceLock;

use rustpg2d_engine::refactor::RefKind;
use serde_json::Value;

struct Store {
    kinds: HashMap<RefKind, Value>,
    project: Value,
    entity: Value,
    zone: Value,
    trigger: Value,
    map_settings: Value,
}

/// A schema as the forms read it: a plain value that may be left out (`"type": ["string",
/// "null"]`, what an `Option<String>` gives) is written like every other optional thing, as
/// one of two (`anyOf`: the value, or nothing), so its control is the value's own with a way
/// to say "none".
fn tidy_schema(mut v: Value) -> Value {
    fn walk(v: &mut Value) {
        match v {
            Value::Object(m) => {
                for x in m.values_mut() {
                    walk(x);
                }
                let optional = m.get("type").and_then(Value::as_array).filter(|t| t.len() == 2 && t.iter().any(|x| x == "null")).and_then(|t| t.iter().find(|x| *x != "null").cloned());
                if let Some(ty) = optional {
                    let mut inner = std::mem::take(m);
                    inner.insert("type".into(), ty);
                    // (What says something about the field stays with the field.)
                    for key in ["description", "default"] {
                        if let Some(x) = inner.remove(key) {
                            m.insert(key.into(), x);
                        }
                    }
                    m.insert("anyOf".into(), Value::Array(vec![Value::Object(inner), serde_json::json!({"type": "null"})]));
                }
            }
            Value::Array(a) => a.iter_mut().for_each(walk),
            _ => {}
        }
    }
    walk(&mut v);
    v
}

fn store() -> &'static Store {
    static STORE: OnceLock<Store> = OnceLock::new();
    STORE.get_or_init(|| {
        use rustpg2d_engine::schema;
        Store {
            kinds: RefKind::ALL.iter().filter_map(|k| Some((*k, tidy_schema(schema::of_kind(*k)?.to_value())))).collect(),
            project: tidy_schema(schema::project().to_value()),
            entity: tidy_schema(schema::entity().to_value()),
            zone: tidy_schema(schema::zone().to_value()),
            trigger: tidy_schema(schema::trigger().to_value()),
            map_settings: tidy_schema(schema::map_settings().to_value()),
        }
    })
}

fn root(v: &'static Value) -> Node<'static> {
    Node { root: v, at: v, ty: v.get("title").and_then(Value::as_str).unwrap_or(""), variant: "" }
}

/// The schema of a definition of `kind` (`None`: a kind without definitions).
pub fn of_kind(kind: RefKind) -> Option<Node<'static>> {
    store().kinds.get(&kind).map(root)
}

/// The project's settings.
pub fn project() -> Node<'static> {
    root(&store().project)
}

/// A placed object of a map.
pub fn entity() -> Node<'static> {
    root(&store().entity)
}

/// A zone of a map.
pub fn zone() -> Node<'static> {
    root(&store().zone)
}

/// A map's settings.
pub fn map_settings() -> Node<'static> {
    root(&store().map_settings)
}

/// A trigger.
pub fn trigger() -> Node<'static> {
    root(&store().trigger)
}

/// A place in a schema: what a value there looks like.
#[derive(Clone, Copy)]
pub struct Node<'a> {
    root: &'a Value,
    at: &'a Value,
    /// The engine type this place is in (`ItemDef`, `Action`), and inside an enum's variant
    /// with fields, the variant (`GiveItem`; else empty).
    pub ty: &'a str,
    pub variant: &'a str,
}

/// What a value is, for a form.
pub enum Shape<'a> {
    Text,
    /// "#rrggbb" or "#rrggbbaa".
    Color,
    Int { min: Option<i64>, max: Option<i64> },
    Float,
    Bool,
    /// One of several variants (an enum): written as the variant's name, or as an object with
    /// the name as its only key when the variant holds data.
    Choice(Vec<Variant<'a>>),
    /// Nothing (null) or a value.
    Optional(Node<'a>),
    List(Node<'a>),
    /// A list with a fixed number of entries (a size's two numbers, a colour's three).
    Fixed(Vec<Node<'a>>),
    /// Named entries (an object with free keys).
    Map(Node<'a>),
    Struct(Vec<Field<'a>>),
    /// Anything (the schema doesn't say more).
    Any,
}

pub struct Variant<'a> {
    pub name: &'a str,
    doc: &'a str,
    /// What the variant holds; `None` = just its name.
    pub data: Option<Node<'a>>,
}

impl Variant<'_> {
    /// The variant's doc comment, tidied; may be empty.
    pub fn help(&self) -> String {
        tidy(self.doc)
    }

    /// The variant with the values it starts with.
    pub fn blank(&self) -> Value {
        match self.data {
            None => Value::String(self.name.to_string()),
            Some(n) => Value::Object([(self.name.to_string(), n.blank())].into_iter().collect()),
        }
    }
}

pub struct Field<'a> {
    pub key: &'a str,
    pub node: Node<'a>,
    /// Always written to the file (others are left out while they hold their default).
    pub required: bool,
}

impl Field<'_> {
    /// Where the field is: `ItemDef.max_stack`, `Action.GiveItem.item`. The tables are keyed by
    /// this.
    pub fn site(&self) -> String {
        site(self.node.ty, self.node.variant, self.key)
    }

    pub fn label(&self) -> String {
        label(&self.site(), self.key)
    }

    /// The kinds of definition the field names (empty: it holds no id).
    pub fn reference(&self) -> &'static [RefKind] {
        reference(&self.site())
    }
}

fn site(ty: &str, variant: &str, key: &str) -> String {
    if variant.is_empty() {
        format!("{ty}.{key}")
    } else {
        format!("{ty}.{variant}.{key}")
    }
}

/// Which variant of `variants` a value is.
pub fn variant_of(variants: &[Variant], v: &Value) -> Option<usize> {
    let name = match v {
        Value::String(s) => s.as_str(),
        Value::Object(m) if m.len() == 1 => m.keys().next()?.as_str(),
        _ => return None,
    };
    variants.iter().position(|x| x.name == name)
}

impl<'a> Node<'a> {
    /// Past the references to named types.
    fn resolved(mut self) -> Node<'a> {
        for _ in 0..16 {
            let Some(r) = self.at.get("$ref").and_then(Value::as_str) else { break };
            let name = r.rsplit('/').next().unwrap_or("");
            let Some((key, target)) = self.root.get("$defs").and_then(Value::as_object).and_then(|d| d.get_key_value(name)) else { break };
            self.at = target;
            self.ty = key.as_str();
            self.variant = "";
        }
        self
    }

    /// The place a JSON pointer leads to from here (`/weapon/damage`, `/on_use/0/actions`, a
    /// variant by its name: `/behavior/Chase/sight`); `None` when the data has no such place.
    pub fn at(self, pointer: &str) -> Option<Node<'a>> {
        let mut node = self;
        for part in pointer.split('/').skip(1) {
            while let Shape::Optional(inner) = node.shape() {
                node = inner;
            }
            node = match node.shape() {
                Shape::Struct(fields) => fields.into_iter().find(|f| f.key == part)?.node,
                Shape::List(inner) | Shape::Map(inner) => inner,
                Shape::Fixed(nodes) => *nodes.get(part.parse::<usize>().ok()?)?,
                Shape::Choice(variants) => variants.into_iter().find(|v| v.name == part)?.data?,
                _ => return None,
            };
        }
        Some(node)
    }

    /// The doc comment of the field (else of its type), tidied for reading; may be empty.
    pub fn help(self) -> String {
        let text = self.at.get("description").or_else(|| self.resolved().at.get("description")).and_then(Value::as_str).unwrap_or("");
        tidy(text)
    }

    /// The value a file may leave out here.
    pub fn default(self) -> Option<&'a Value> {
        self.at.get("default")
    }

    /// The name of the type here, when the place is a named type's (`SpriteRef`, `Trigger`,
    /// the definition itself); empty for a plain value or a list.
    pub fn type_name(self) -> &'a str {
        if self.at.get("$ref").is_some() {
            self.resolved().ty
        } else if std::ptr::eq(self.at, self.root) {
            self.ty
        } else {
            ""
        }
    }

    pub fn shape(self) -> Shape<'a> {
        let n = self.resolved();
        let at = n.at;
        let ty = |v: &'a Value| v.get("type").and_then(Value::as_str);
        if let Some(any) = at.get("anyOf").and_then(Value::as_array) {
            let rest: Vec<&Value> = any.iter().filter(|s| ty(s) != Some("null")).collect();
            return match rest[..] {
                [one] if any.len() == 2 => Shape::Optional(Node { at: one, ..n }),
                _ => Shape::Any,
            };
        }
        if let Some(one) = at.get("oneOf").and_then(Value::as_array) {
            let mut variants = Vec::new();
            for alt in one {
                let doc = alt.get("description").and_then(Value::as_str).unwrap_or("");
                if let Some(names) = alt.get("enum").and_then(Value::as_array) {
                    variants.extend(names.iter().filter_map(Value::as_str).map(|name| Variant { name, doc: "", data: None }));
                } else if let Some(name) = alt.get("const").and_then(Value::as_str) {
                    variants.push(Variant { name, doc, data: None });
                } else if let Some((name, data)) = alt.get("properties").and_then(Value::as_object).and_then(|p| p.iter().next()) {
                    variants.push(Variant { name, doc, data: Some(Node { root: n.root, at: data, ty: n.ty, variant: name }) });
                }
            }
            return Shape::Choice(variants);
        }
        if let Some(names) = at.get("enum").and_then(Value::as_array) {
            return Shape::Choice(names.iter().filter_map(Value::as_str).map(|name| Variant { name, doc: "", data: None }).collect());
        }
        match ty(at) {
            Some("object") => {
                if let Some(props) = at.get("properties").and_then(Value::as_object) {
                    let required: Vec<&str> = at.get("required").and_then(Value::as_array).map(|r| r.iter().filter_map(Value::as_str).collect()).unwrap_or_default();
                    Shape::Struct(props.iter().map(|(key, sub)| Field { key, node: Node { at: sub, ..n }, required: required.contains(&key.as_str()) }).collect())
                } else if let Some(values) = at.get("additionalProperties").filter(|a| a.is_object()) {
                    Shape::Map(Node { at: values, ..n })
                } else {
                    Shape::Any
                }
            }
            Some("array") => {
                let fixed = at.get("minItems").and_then(Value::as_u64).filter(|m| at.get("maxItems").and_then(Value::as_u64) == Some(*m));
                if let Some(items) = at.get("prefixItems").and_then(Value::as_array) {
                    Shape::Fixed(items.iter().map(|i| Node { at: i, ..n }).collect())
                } else if let (Some(items), Some(count)) = (at.get("items"), fixed) {
                    Shape::Fixed((0..count).map(|_| Node { at: items, ..n }).collect())
                } else if let Some(items) = at.get("items") {
                    Shape::List(Node { at: items, ..n })
                } else {
                    Shape::Any
                }
            }
            Some("string") if at.get("format").and_then(Value::as_str) == Some("color") => Shape::Color,
            Some("string") => Shape::Text,
            Some("integer") => Shape::Int { min: at.get("minimum").and_then(Value::as_i64), max: at.get("maximum").and_then(Value::as_i64) },
            Some("number") => Shape::Float,
            Some("boolean") => Shape::Bool,
            _ => Shape::Any,
        }
    }

    /// A value to start with here: the default a file may leave out, else the plainest value
    /// of the shape (empty text, 0, off, nothing, an empty list, an enum's first variant).
    pub fn blank(self) -> Value {
        self.blank_at(0)
    }

    fn blank_at(self, depth: usize) -> Value {
        if let Some(d) = self.default() {
            return d.clone();
        }
        if depth > 8 {
            return Value::Null;
        }
        match self.shape() {
            Shape::Text => Value::String(String::new()),
            Shape::Color => Value::String("#ffffff".into()),
            Shape::Int { min, .. } => Value::from(min.unwrap_or(0).max(0)),
            Shape::Float => Value::from(0.0),
            Shape::Bool => Value::Bool(false),
            Shape::Choice(variants) => match variants.first() {
                Some(Variant { name, data: None, .. }) => Value::String(name.to_string()),
                Some(Variant { name, data: Some(n), .. }) => Value::Object([(name.to_string(), n.blank_at(depth + 1))].into_iter().collect()),
                None => Value::Null,
            },
            Shape::Optional(_) | Shape::Any => Value::Null,
            Shape::List(_) => Value::Array(Vec::new()),
            Shape::Fixed(nodes) => Value::Array(nodes.iter().map(|n| n.blank_at(depth + 1)).collect()),
            Shape::Map(_) => Value::Object(Default::default()),
            Shape::Struct(fields) => Value::Object(fields.iter().map(|f| (f.key.to_string(), f.node.blank_at(depth + 1))).collect()),
        }
    }
}

/// A doc comment as a help text: without the code marks and links of Rust docs, its lines
/// joined into paragraphs.
fn tidy(doc: &str) -> String {
    let mut out = String::new();
    for (k, para) in doc.split("\n\n").enumerate() {
        if k > 0 {
            out.push_str("\n\n");
        }
        let line = para.split('\n').map(str::trim).collect::<Vec<_>>().join(" ");
        let mut chars = line.chars().peekable();
        while let Some(c) = chars.next() {
            match c {
                '`' => {}
                // [`Type::field`] and [text](link): the text stays.
                '[' if chars.peek() == Some(&'`') => {}
                ']' if out.ends_with(|p: char| p.is_alphanumeric() || p == ')') && chars.peek() != Some(&'(') => {}
                _ => out.push(c),
            }
        }
    }
    out
}

/// `frame_w` as "Frame w".
pub fn plain_label(key: &str) -> String {
    let mut s = key.replace('_', " ");
    if let Some(f) = s.get(..1) {
        s = format!("{}{}", f.to_uppercase(), &s[1..]);
    }
    s
}

/// `GiveItem` as "Give item".
pub fn variant_label(name: &str) -> String {
    let mut out = String::new();
    for (k, c) in name.chars().enumerate() {
        if c.is_uppercase() && k > 0 {
            out.push(' ');
            out.extend(c.to_lowercase());
        } else {
            out.push(c);
        }
    }
    out
}

/// The label of the field at `site` (see [`Field::site`]): the table's, else the key in words.
pub fn label(site: &str, key: &str) -> String {
    let found = LABELS.iter().find(|(s, _)| *s == site).or_else(|| LABELS.iter().find(|(s, _)| s.strip_prefix("*.") == Some(key)));
    found.map_or_else(|| plain_label(key), |(_, l)| l.to_string())
}

/// Labels that say more than the field's name (`*.key` = that key anywhere).
const LABELS: &[(&str, &str)] = &[
    ("*.id", "Id"),
    ("*.hp", "Hit points"),
    ("*.max_hp", "Hit points"),
    ("*.xp", "Experience"),
    ("*.dc", "Difficulty (DC)"),
    ("*.fps", "Frames per second"),
    ("*.frame_w", "Frame width"),
    ("*.frame_h", "Frame height"),
    ("*.w", "Width"),
    ("*.h", "Height"),
    ("*.srd", "SRD name"),
    ("*.cost_cp", "Cost (copper)"),
    ("*.coin_cp", "Coin value (copper)"),
    ("*.walk_ft", "Walking speed (feet)"),
    ("*.darkvision_ft", "Darkvision (feet)"),
    ("*.offset_y", "Line offset"),
    ("ItemDef.max_stack", "Stack size"),
    ("ItemDef.on_use", "When used"),
    ("ItemDef.use_on", "Used on"),
    ("ItemDef.use_anim", "Animation when used"),
    ("ItemDef.wear", "Shown as"),
    ("ItemDef.held", "Picture in the hand"),
    ("ItemDef.model", "3D model in the hand"),
    ("ItemDef.anims", "Animations"),
    ("ItemDef.bag_slots", "Extra bag slots"),
    ("PropDef.sprite", "Picture"),
    ("PropDef.default_state", "First state"),
    ("PropDef.use_spot", "Sitting / sleeping spot"),
    ("PropDef.climb_dc", "Climbing difficulty (DC)"),
    ("PropDef.anim", "Animation"),
    ("PropDef.model", "3D model"),
    ("CharacterDef.max_hp", "Hit points (old)"),
    ("CharacterDef.stats", "Stat block"),
    ("CharacterDef.gear_locked", "Gear can't be changed"),
    ("QuestDef.giver_name", "Giver's name"),
    ("SpellDef.parts", "What it does"),
    ("ModelDef.clip_files", "More animation files"),
    ("ModelDef.shared_clips", "Animations from other models"),
    ("ModelDef.female", "Offered as female"),
    ("ModelDef.playable", "Offered in the character creator"),
    ("ProjectConfig.name", "The game's name"),
    ("ProjectConfig.start_map", "It starts on"),
    ("ProjectConfig.start_spawn", "At the spawn point"),
    ("ProjectConfig.scale", "Zoom"),
    ("ProjectConfig.player", "The player's character"),
    ("ProjectConfig.player_portrait", "The player's portrait"),
    ("ProjectConfig.creation", "Character creator"),
    ("ProjectConfig.inventory_size", "Bag slots"),
    ("ProjectConfig.companion_down", "A companion at 0 hit points"),
    ("ProjectConfig.progression", "Experience and levels"),
    ("ProjectConfig.rules", "The SRD's options"),
    ("ProjectConfig.skills", "Magic"),
    ("ProjectConfig.srd", "The SRD's content in the game"),
    ("ProjectConfig.day_minutes", "A day lasts (minutes)"),
    ("ProjectConfig.start_hour", "The game starts at (hour)"),
    ("ProjectConfig.map_titles", "Maps' titles"),
    ("ProjectConfig.skin", "UI skin"),
    ("ProjectConfig.fonts", "Fonts"),
    ("ProjectConfig.dialog_portraits", "Portraits in dialogs"),
    ("ProjectConfig.auto_emotions", "Emotions by themselves"),
    ("ProjectConfig.eight_facings", "Characters face eight ways"),
    ("ProjectConfig.title", "Title screen"),
    ("ProjectConfig.menu", "Pause menu"),
    ("ProjectConfig.sfx", "The game's sounds"),
    ("ProjectConfig.battle_music", "Music of fights"),
    ("ProjectConfig.language", "Written in"),
    ("ProjectConfig.languages", "Translated into"),
    ("ProjectConfig.hooks", "Script hooks"),
    ("ProjectConfig.model_atlas_mb", "Memory for drawn figures (MB)"),
    ("Trigger.on", "When"),
    ("Trigger.conditions", "If"),
    ("Trigger.actions", "Do"),
    ("Trigger.once", "Only once"),
];

/// The sections of a definition's form: `(title, fields)`, the fields separated by spaces, one
/// with `~` before it for advanced use (shown with *Show all fields*). A type that isn't listed
/// has no sections (its fields in their own order).
pub fn sections(ty: &str) -> &'static [(&'static str, &'static str)] {
    SECTIONS.iter().find(|(t, _)| *t == ty).map_or(&[], |(_, s)| s)
}

pub type Sections = &'static [(&'static str, &'static str)];

/// The Inspector's sections of a placed thing, by what it is (`Prop`, `Character`, `Item`): the
/// fields that mean something for it. The section without a title holds what the Inspector
/// shows itself or what means nothing for that kind.
pub fn entity_sections(kind: &str) -> Sections {
    match kind {
        "Prop" => &[
            ("Placed here", "name facing state active ~frame"),
            ("Its own, instead of the definition's", "container lock loot light"),
            ("Events", "triggers"),
            ("", "uid kind def x y count patrol schedule shop appearance equipment inventory portrait activity faction stats"),
        ],
        "Character" => &[
            ("Placed here", "name facing active faction"),
            ("Daily life", "patrol schedule activity"),
            ("Their own, instead of the definition's", "appearance equipment inventory portrait shop loot stats light"),
            ("Events", "triggers"),
            ("", "uid kind def x y count state frame container lock"),
        ],
        _ => &[("Placed here", "name count active light"), ("Events", "triggers"), ("", "uid kind def x y facing state frame container lock loot patrol schedule shop appearance equipment inventory portrait activity faction stats")],
    }
}

/// The sections of a map's settings.
pub const MAP_SECTIONS: Sections = &[("Light and weather", "lighting weather fog background"), ("Sound", "music battle_music"), ("Beyond the edges", "edges parallax"), ("Events", "triggers")];

/// The Inspector's sections of a zone.
pub const ZONE_SECTIONS: Sections = &[("Zone", "name kind"), ("Place (tiles)", "x y w h"), ("Events", "triggers"), ("Spawned characters", "~loot"), ("", "uid")];

const SECTIONS: &[(&str, Sections)] = &[
    ("SheetDef", &[("Picture", "id image frame_w frame_h scale"), ("Use", "kind ~tags")]),
    (
        "ItemDef",
        &[
            ("Basics", "id name description icon max_stack value"),
            ("Using it", "consumable on_use use_on use_anim uses teaches food"),
            ("Equipment", "weapon armor tool bag_slots light wear"),
            ("In the hand", "held model anims"),
            ("Rules", "srd cost_cp weight coin_cp magic silvered poison"),
            ("Farming and homes", "crop furniture"),
        ],
    ),
    (
        "PropDef",
        &[
            ("Basics", "id name sprite model anim"),
            ("Shape", "hitbox solid low flat block counter turns shadow ~snap ~pieces"),
            ("States and events", "default_state states triggers"),
            ("Behaviour", "container lock door light use_spot carry"),
            ("Terrain", "difficult obscures climb climb_dc"),
            ("Gathering and farming", "gather regrow plot"),
            ("Rules", "object trap"),
            ("Building", "building"),
        ],
    ),
    (
        "CharacterDef",
        &[
            ("Basics", "id name description race faction behavior hostile speed"),
            ("Look", "appearance portrait moods light hitbox"),
            ("Rules", "stats class level xp"),
            ("Gear", "equipment inventory gear_locked loot shop"),
            ("Events", "triggers"),
            ("Values from before the SRD rules", "~max_hp ~damage ~weapon ~defense ~touch_effect ~windup ~max_mana ~mana_regen"),
        ],
    ),
    ("SoundDef", &[("Sound", "id file volume caption")]),
    (
        "QuestDef",
        &[
            ("Basics", "id title description xp"),
            ("Giver", "giver giver_name requires offer"),
            ("Steps", "steps objects ~stages ~targets"),
            ("Rewards", "rewards"),
        ],
    ),
    ("StructureDef", &[("Basics", "id name category w h"), ("Contents (edited on the map)", "~materials ~ground ~decor_defs ~decor ~clear ~collision ~entities ~tilesets ~layers")]),
    (
        "SpellDef",
        &[
            ("Basics", "id name description icon level school classes"),
            ("Casting", "casting range area components material costly ritual concentration duration_rounds duration"),
            ("Effect", "parts visual story_only"),
            ("Values from before the SRD rules", "~mana ~cooldown ~skill ~effect"),
        ],
    ),
    ("RecipeDef", &[("Basics", "id result count ingredients"), ("Needs", "station station_state tools skill level"), ("Quality", "quality")]),
    ("AchievementDef", &[("Basics", "id name description icon hidden"), ("Unlocked when", "conditions")]),
    (
        "RaceDef",
        &[
            ("Basics", "id name description parent playable faction"),
            ("Look", "bodies_male bodies_female size colors"),
            ("Rules", "ability_bonuses walk_ft size_category darkvision_ft hp_per_level languages resistances skills traits choices"),
            ("Values from before the SRD rules", "~max_hp ~speed ~defense"),
        ],
    ),
    (
        "ModelDef",
        &[
            ("Basics", "id name file height stand"),
            ("Animations", "clips clip_files shared_clips rig"),
            ("Colours", "features"),
            ("Character creator", "playable female"),
            ("Pieces", "part parts"),
        ],
    ),
    (
        "MaterialDef",
        &[("Basics", "id name texture priority"), ("Edges", "edge edge_noise edge_pixel rim"), ("Walking", "walk difficult step"), ("Movement", "scroll frames fps")],
    ),
    ("ScatterDef", &[("Scatter set", "id name props density spacing flip layer")]),
    ("FontDef", &[("Font", "id name file pixel scale offset_y"), ("Licence", "license credit")]),
    (
        "SkinDef",
        &[
            ("Basics", "id name"),
            ("Windows", "frame background border dialog name_box hud"),
            ("Buttons", "button button_hover button_fill button_hover_fill cursor cursor_fill"),
            ("Toasts", "toast toast_fill toast_text"),
            ("Text and bars", "text hp mana xp colors"),
        ],
    ),
    ("CommonEventDef", &[("Basics", "id name trigger"), ("Actions", "actions")]),
    ("BackgroundDef", &[("Basics", "id name description"), ("Gives", "proficiencies language_choices equipment feature feature_text")]),
    (
        "ClassDef",
        &[
            ("Basics", "id name description hit_die ability_order"),
            ("Training", "saves proficiencies skill_choices skill_options multiclass"),
            ("Starting gear", "equipment starting_gold"),
            ("Levels", "levels features spellcasting"),
            ("Subclasses", "subclass_level subclass_title subclasses"),
        ],
    ),
    (
        "DiseaseDef",
        &[
            ("Basics", "id name description"),
            ("Catching it", "save dc onset onset_days"),
            ("While sick", "exhaustion keeps_exhaustion no_natural_healing no_rest_healing half_hit_dice max_hp_loss sight condition when_hurt"),
            ("Course", "course"),
        ],
    ),
    ("CutsceneDef", &[("Basics", "id name map"), ("Timeline", "tracks")]),
    ("AnimationDef", &[("Basics", "id name"), ("Parts", "cels emitters flashes sounds")]),
    ("VisualScript", &[("Basics", "id name description params returns"), ("Graph (edited in the node editor)", "~nodes ~links")]),
];

/// The pages of the game's settings (the *Game* group of the Project browser): `(title, what
/// is on it, fields)`, the fields of the project's settings (`ProjectConfig`) separated by
/// spaces, one with `~` before it for advanced use. Every field is on one page (a test).
pub const PAGES: &[(&str, &str, &str)] = &[
    ("Game", "What the game is called and where it starts.", "name start_map start_spawn scale credits ~tile_size ~model_atlas_mb"),
    ("Hero & party", "The player's character, the character creator, the bag, companions, levels.", "player player_portrait creation inventory_size companion_down progression"),
    ("Rules", "The SRD's options, magic, and what the game says about the rules it uses.", "rules skills srd ~rules_version"),
    ("World", "The day, the factions and how they stand to each other, the maps' titles.", "day_minutes start_hour factions map_titles"),
    ("Look & feel", "The UI's skin and fonts, portraits and emotions in dialogs, how characters face.", "skin fonts dialog_portraits auto_emotions eight_facings"),
    ("Title & menus", "The title screen, the pause menu, the action bar.", "title menu action_bar"),
    ("Sound", "The sounds the game itself plays, and the music of fights.", "sfx battle_music"),
    ("Languages", "The language the game is written in and the ones it is translated into.", "language languages"),
    ("Scripts & plugins", "Script functions the game calls at its own moments, and plugins.", "hooks plugins"),
];

/// A page's name in scripts and tab names: `Hero & party` -> `hero-party`.
pub fn page_slug(title: &str) -> String {
    title.to_lowercase().split(|c: char| !c.is_ascii_alphanumeric()).filter(|p| !p.is_empty()).collect::<Vec<_>>().join("-")
}

/// The kinds of definition (or `Map`) the field at `site` names, when it holds an id: the form
/// shows a picker there. Empty: no id.
pub fn reference(site: &str) -> &'static [RefKind] {
    REFERENCES.iter().chain(NOT_FOLLOWED).find(|(s, _)| *s == site).map_or(&[], |(_, kinds)| kinds)
}

/// Ids the engine's renames don't follow (yet): the form still shows a picker there.
#[rustfmt::skip]
const NOT_FOLLOWED: &[(&str, &[RefKind])] = {
    use RefKind::*;
    &[
        ("ActEffect.Spell.spell", &[Spell]),
        ("Action.Carry.prop", &[Prop]),
        ("Action.LearnSpell.spell", &[Spell]),
        ("Action.Quest.quest", &[Quest]),
        ("Action.UnlockAchievement.id", &[Achievement]),
        ("Buff.SkillBonus.skill", &[Skill]),
        ("Cond.KnowsSpell.spell", &[Spell]),
        ("Cond.Quest.quest", &[Quest]),
        ("FeatureEffect.AlwaysPrepared.spells", &[Spell]),
        ("FeatureEffect.CheckAdvantage.skill", &[Skill]),
        ("FeatureEffect.SpellDamageBonus.spell", &[Spell]),
        ("RaceDef.parent", &[Race]),
        ("RouteStep.Sound.sound", &[Sound]),
        ("RulesSettings.camp_item", &[Item]),
        ("ScheduleStop.map", &[Map]),
    ]
};

/// Fields that hold an id, with the kinds it may name: exactly the fields the engine's renames
/// follow (the test `references_agree_with_renames` renames through made-up values with
/// everything in them and prints the lines this table should have).
#[rustfmt::skip]
const REFERENCES: &[(&str, &[RefKind])] = {
    use RefKind::*;
    &[
        ("Action.CallCommon.id", &[CommonEvent]),
        ("Action.CastSpell.spell", &[Spell]),
        ("Action.ChangeShape.form", &[Character]),
        ("Action.CureDisease.disease", &[Disease]),
        ("Action.Cutscene.function", &[Cutscene]),
        ("Action.Equip.item", &[Item]),
        ("Action.ExposeDisease.disease", &[Disease]),
        ("Action.GiveItem.item", &[Item]),
        ("Action.GiveItemTo.item", &[Item]),
        ("Action.PlayMusic.sound", &[Sound]),
        ("Action.PlayMusicEffect.sound", &[Sound]),
        ("Action.PlaySound.sound", &[Sound]),
        ("Action.Poison.item", &[Item]),
        ("Action.SetClass.class", &[Class]),
        ("Action.SetSkill.skill", &[Skill]),
        ("Action.SetSpawners.map", &[Map]),
        ("Action.SetWeather.map", &[Map]),
        ("Action.ShopStock.item", &[Item]),
        ("Action.ShowImage.sheet", &[Sheet]),
        ("Action.ShowPicture.sheet", &[Sheet]),
        ("Action.TakeItem.item", &[Item]),
        ("Action.TakeItemFrom.item", &[Item]),
        ("Action.TeachSpell.spell", &[Spell]),
        ("Action.Teleport.map", &[Map]),
        ("AnimSound.sound", &[Sound]),
        ("Appearance.body", &[Model]),
        ("Appearance.model", &[Model]),
        ("Appearance.parts", &[Model]),
        ("Cel.sheet", &[Sheet]),
        ("CharacterCreation.body_female", &[Model]),
        ("CharacterCreation.body_male", &[Model]),
        ("CharacterCreation.store", &[Character]),
        ("CharacterDef.race", &[Race]),
        ("Cond.Attuned.item", &[Item]),
        ("Cond.Class.class", &[Class]),
        ("Cond.Defeated.character", &[Character]),
        ("Cond.HasDisease.disease", &[Disease]),
        ("Cond.HasItem.item", &[Item]),
        ("Cond.KnockedOut.name", &[Character]),
        ("Cond.PlayerRace.race", &[Race]),
        ("Cond.Skill.skill", &[Skill]),
        ("Costly.item", &[Item]),
        ("CutsceneDef.map", &[Map]),
        ("EdgeLink.target_map", &[Map]),
        ("EncounterEntry.character", &[Character]),
        ("EntityInstance.def", &[Item, Prop, Character]),
        ("FeatureEffect.Act.anim", &[Animation]),
        ("FontRoles.dialog", &[Font]),
        ("FontRoles.fallback", &[Font]),
        ("FontRoles.hud", &[Font]),
        ("FontRoles.title", &[Font]),
        ("FontRoles.ui", &[Font]),
        ("FxSprite.sheet", &[Sheet, Animation]),
        ("GameSfx.attack", &[Sound]),
        ("GameSfx.defeat", &[Sound]),
        ("GameSfx.dialog", &[Sound]),
        ("GameSfx.hit", &[Sound]),
        ("GameSfx.hurt", &[Sound]),
        ("GameSfx.level_up", &[Sound]),
        ("GameSfx.open_container", &[Sound]),
        ("GameSfx.pickup", &[Sound]),
        ("GameSfx.portal", &[Sound]),
        ("GameSfx.unlock", &[Sound]),
        ("GameSfx.victory", &[Sound]),
        ("HeroClass.class", &[Class]),
        ("HitRider.disease", &[Disease]),
        ("ItemDef.furniture", &[Prop]),
        ("ItemDef.teaches", &[Spell]),
        ("ItemDef.use_anim", &[Animation]),
        ("ItemSpell.spell", &[Spell]),
        ("ItemStack.item", &[Item]),
        ("Join.prop", &[Prop]),
        ("LockSpec.key", &[Item]),
        ("LootEntry.item", &[Item]),
        ("MagicItem.attune_by", &[Race, Class]),
        ("MapSettings.battle_music", &[Sound]),
        ("MapSettings.music", &[Sound]),
        ("MaterialDef.step", &[Sound]),
        ("ModelDef.parts", &[Model]),
        ("ModelPart.fits", &[Model]),
        ("NineSlice.sheet", &[Sheet]),
        ("NodeKind.Call.function", &[VisualScript]),
        ("ParallaxLayer.sheet", &[Sheet]),
        ("ProjectConfig.battle_music", &[Sound]),
        ("ProjectConfig.map_titles#key", &[Map]),
        ("ProjectConfig.player", &[Character]),
        ("ProjectConfig.skin", &[Skin]),
        ("ProjectConfig.start_map", &[Map]),
        ("PropDef.model", &[Model]),
        ("PropDef.pieces", &[Prop]),
        ("PropState.model", &[Model]),
        ("QualityStep.result", &[Item]),
        ("QuestDef.giver", &[Character]),
        ("QuestTarget.map", &[Map]),
        ("RaceDef.bodies_female", &[Model]),
        ("RaceDef.bodies_male", &[Model]),
        ("RaceSkill.skill", &[Skill]),
        ("Ranged.ammo", &[Item]),
        ("RecipeDef.result", &[Item]),
        ("RecipeDef.skill", &[Skill]),
        ("RewardItem.item", &[Item]),
        ("RigFit.template", &[Model]),
        ("ScatterDef.props.0", &[Prop]),
        ("ShopEntry.item", &[Item]),
        ("ShopSpec.currency", &[Item]),
        ("SpellDef.classes", &[Class]),
        ("SpellDef.skill", &[Skill]),
        ("SpellPart.Summon.creature", &[Character]),
        ("SpellVisual.impact", &[Sheet, Animation]),
        ("SpellVisual.sound", &[Sound]),
        ("SpriteRef.sheet", &[Sheet]),
        ("StatAttack.anim", &[Animation]),
        ("StatSpell.spell", &[Spell]),
        ("StepGoal.Talk.character", &[Character]),
        ("StructureDef.decor_defs", &[Prop]),
        ("StructureDef.materials", &[Material]),
        ("TitleBackground.Map.map", &[Map]),
        ("TitleBackground.Picture.sheet", &[Sheet]),
        ("TitleLogo.sheet", &[Sheet]),
        ("TitleScreen.music", &[Sound]),
        ("TriggerOn.UseItem.item", &[Item]),
        ("Weapon.hit_anim", &[Animation]),
        ("ZoneKind.Ambience.sound", &[Sound]),
        ("ZoneKind.Drop.target_map", &[Map]),
        ("ZoneKind.FishingSpot.rod", &[Item]),
        ("ZoneKind.Portal.target_map", &[Map]),
        ("ZoneKind.Spawner.character", &[Character]),
        ("ZoneKind.Transition.target_map", &[Map]),
    ]
};

/// Texts the engine's renames follow by what stands around them (a script call's argument),
/// which no picker can know from the field alone.
#[cfg(test)]
const BY_CONTEXT: &[&str] = &["Value.Text"];

/// Names a project makes up as it goes, which a text field takes one of: the form offers the
/// ones in use, and a new one is typed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Names {
    /// A script function.
    Function,
    Flag,
    Variable,
    Faction,
    /// A prop's state.
    State,
    /// Who an action is about: `self`, `player`, a named object of the map.
    Target,
    /// A crafting station.
    Station,
    Emotion,
}

/// The kind of name the text field at `site` holds (see [`Names`]); `None`: free text, or a
/// definition's id ([`reference`], which goes first).
pub fn names(site: &str, key: &str) -> Option<Names> {
    if !reference(site).is_empty() {
        return None;
    }
    NAMES.iter().find(|(s, _)| *s == site).or_else(|| NAMES.iter().find(|(s, _)| s.strip_prefix("*.") == Some(key))).map(|(_, n)| *n)
}

/// (`*.key` = that key anywhere.)
const NAMES: &[(&str, Names)] = &[
    ("*.function", Names::Function),
    ("*.flag", Names::Flag),
    ("Action.SetFlag.name", Names::Flag),
    ("Cond.Flag.name", Names::Flag),
    ("*.var", Names::Variable),
    ("Action.SetVar.name", Names::Variable),
    ("Cond.Var.name", Names::Variable),
    ("Cond.TextVar.name", Names::Variable),
    ("*.faction", Names::Faction),
    ("Relation.a", Names::Faction),
    ("Relation.b", Names::Faction),
    ("*.state", Names::State),
    ("PropDef.default_state", Names::State),
    ("ContainerSpec.open_state", Names::State),
    ("Gather.then_state", Names::State),
    ("RecipeDef.station_state", Names::State),
    ("*.target", Names::Target),
    ("Action.ChangeShape.who", Names::Target),
    ("*.station", Names::Station),
    ("Action.Emote.emotion", Names::Emotion),
];

/// Calls `f(site, pointer, text)` for every text in `value`, walking it with its schema: the
/// site is the nearest field's ([`Field::site`]; `#key` after it for a map's key), the pointer
/// the JSON pointer of the text.
pub fn texts(node: Node, value: &Value, f: &mut dyn FnMut(&str, &str, &str)) {
    walk(node, value, &mut String::new(), &mut String::new(), f);
}

fn walk(node: Node, value: &Value, site: &mut String, pointer: &mut String, f: &mut dyn FnMut(&str, &str, &str)) {
    let at = |pointer: &mut String, key: &str, inner: &mut dyn FnMut(&mut String)| {
        let len = pointer.len();
        pointer.push('/');
        pointer.push_str(&key.replace('~', "~0").replace('/', "~1"));
        inner(pointer);
        pointer.truncate(len);
    };
    match (node.shape(), value) {
        (Shape::Optional(inner), v) if !v.is_null() => walk(inner, v, site, pointer, f),
        // (An enum's variant without data is its name, not a text.)
        (Shape::Text | Shape::Any, Value::String(s)) => f(site, pointer, s),
        (Shape::Choice(variants), Value::Object(m)) => {
            if let Some((v, (name, data))) = variant_of(&variants, value).and_then(|k| Some((&variants[k], m.iter().next()?))) {
                if let Some(n) = v.data {
                    // A variant that holds one value (not fields): the value's site is the variant.
                    let old = std::mem::replace(site, format!("{}.{}", n.ty, name));
                    at(pointer, name, &mut |p| walk(n, data, site, p, f));
                    *site = old;
                }
            }
        }
        (Shape::Struct(fields), Value::Object(m)) => {
            for field in &fields {
                if let Some(v) = m.get(field.key) {
                    let old = std::mem::replace(site, field.site());
                    at(pointer, field.key, &mut |p| walk(field.node, v, site, p, f));
                    *site = old;
                }
            }
        }
        (Shape::Map(inner), Value::Object(m)) => {
            for (key, v) in m {
                at(pointer, key, &mut |p| {
                    f(&format!("{site}#key"), &format!("{p}#key"), key);
                    walk(inner, v, site, p, f);
                });
            }
        }
        (Shape::List(inner), Value::Array(list)) => {
            for (k, v) in list.iter().enumerate() {
                at(pointer, &k.to_string(), &mut |p| walk(inner, v, site, p, f));
            }
        }
        (Shape::Fixed(nodes), Value::Array(list)) => {
            for (k, (n, v)) in nodes.iter().zip(list).enumerate() {
                let old = site.clone();
                site.push_str(&format!(".{k}"));
                at(pointer, &k.to_string(), &mut |p| walk(*n, v, site, p, f));
                *site = old;
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rustpg2d_engine::defs::Database;
    use rustpg2d_engine::project::ProjectConfig;
    use std::collections::{BTreeMap, BTreeSet};

    #[test]
    fn labels_and_help() {
        assert_eq!(plain_label("frame_w"), "Frame w");
        assert_eq!(variant_label("GiveItem"), "Give item");
        assert_eq!(label("ItemDef.max_stack", "max_stack"), "Stack size");
        assert_eq!(label("Whatever.frame_w", "frame_w"), "Frame width");
        assert_eq!(label("CharacterDef.max_hp", "max_hp"), "Hit points (old)", "a field's own label goes before the general one");
        assert_eq!(tidy("A seed: what grows\non a farm plot (`PropDef::plot`)."), "A seed: what grows on a farm plot (PropDef::plot).");
        assert_eq!(tidy("names that models map to clips ([`ItemAnims`])."), "names that models map to clips (ItemAnims).");
    }

    #[test]
    fn shapes_of_an_item() {
        let item = of_kind(RefKind::Item).unwrap();
        let Shape::Struct(fields) = item.shape() else { panic!("an item has fields") };
        assert_eq!(fields[0].key, "id", "fields come in the definition's order");
        let get = |k: &str| fields.iter().find(|f| f.key == k).unwrap();
        assert!(matches!(get("max_stack").node.shape(), Shape::Int { min: Some(0), .. }));
        assert_eq!(get("max_stack").node.default(), Some(&Value::from(99)));
        assert!(get("max_stack").node.help().contains("inventory slot"));
        assert_eq!(get("icon").node.type_name(), "SpriteRef");
        assert_eq!(get("max_stack").node.type_name(), "");
        let Shape::Optional(weapon) = get("weapon").node.shape() else { panic!("a weapon or none") };
        assert_eq!(weapon.type_name(), "Weapon");
        assert!(matches!(weapon.shape(), Shape::Struct(_)));
        let Shape::Choice(targets) = get("use_on").node.shape() else { panic!("an enum") };
        assert_eq!(targets.iter().map(|v| v.name).collect::<Vec<_>>(), ["Player", "Ally", "Choose"]);
        assert!(!targets[1].help().is_empty());
        assert!(matches!(item.at("/weapon/ranged/ammo").map(|n| n.shape()), Some(Shape::Text)));
        assert!(item.at("/teaches").is_some() && item.at("/nonsense").is_none() && item.at("/weapon/nonsense").is_none());
        assert!(item.at("/on_use/0/actions/3/GiveItem/count").is_some());
        // A blank weapon is one the engine reads.
        let blank = weapon.blank();
        serde_json::from_value::<rustpg2d_engine::defs::Weapon>(blank.clone()).unwrap_or_else(|e| panic!("{e}: {blank}"));
    }

    #[test]
    fn every_setting_is_on_one_page() {
        let Shape::Struct(fields) = project().shape() else { panic!("the settings have fields") };
        let listed: Vec<&str> = PAGES.iter().flat_map(|(_, _, f)| f.split_whitespace()).map(|f| f.trim_start_matches('~')).collect();
        for f in &fields {
            assert_eq!(listed.iter().filter(|l| **l == f.key).count(), 1, "ProjectConfig.{} must be on exactly one page of PAGES", f.key);
        }
        for l in &listed {
            assert!(fields.iter().any(|f| f.key == *l), "PAGES names {l}, which isn't a setting");
        }
        assert_eq!(page_slug("Hero & party"), "hero-party");
        let mut slugs: Vec<String> = PAGES.iter().map(|(t, ..)| page_slug(t)).collect();
        slugs.dedup();
        assert_eq!(slugs.len(), PAGES.len());
    }

    #[test]
    fn made_up_names_are_known_where_they_are_typed() {
        assert_eq!(names("Action.RunScript.function", "function"), Some(Names::Function));
        assert_eq!(names("Action.Cutscene.function", "function"), None, "a cut-scene's id: a picker of definitions");
        assert_eq!(names("Cond.Flag.name", "name"), Some(Names::Flag));
        assert_eq!(names("ItemDef.name", "name"), None);
        assert_eq!(names("CharacterDef.faction", "faction"), Some(Names::Faction));
        // Every exact site of the table is a text field of some schema.
        let mut sites = std::collections::BTreeSet::new();
        for sample in made_up() {
            texts(sample.node(), sample.value(), &mut |site, _, _| {
                sites.insert(site.to_string());
            });
        }
        for (site, _) in NAMES.iter().filter(|(s, _)| !s.starts_with("*.")) {
            assert!(sites.contains(*site), "{site} isn't a text field (any more)");
        }
    }

    #[test]
    fn every_field_has_its_section() {
        let mut problems = Vec::new();
        for kind in RefKind::ALL {
            let Some(node) = of_kind(kind) else { continue };
            let Shape::Struct(fields) = node.shape() else { panic!("{kind:?} has fields") };
            let listed: Vec<&str> = sections(node.ty).iter().flat_map(|(_, f)| f.split_whitespace()).map(|f| f.trim_start_matches('~')).collect();
            for f in &fields {
                match listed.iter().filter(|l| **l == f.key).count() {
                    1 => {}
                    0 => problems.push(format!("{}.{} is in no section", node.ty, f.key)),
                    _ => problems.push(format!("{}.{} is in two sections", node.ty, f.key)),
                }
            }
            for l in &listed {
                if !fields.iter().any(|f| f.key == *l) {
                    problems.push(format!("{}: the sections name {l}, which isn't a field", node.ty));
                }
            }
        }
        // The Inspector's tables: every field of a placed thing and of a zone, once.
        let mut tables: Vec<(String, Node, Sections)> = ["Prop", "Character", "Item"].iter().map(|k| (format!("entity_sections({k})"), entity(), entity_sections(k))).collect();
        tables.push(("ZONE_SECTIONS".into(), zone(), ZONE_SECTIONS));
        for (name, node, table) in tables {
            let Shape::Struct(fields) = node.shape() else { panic!("{name} has fields") };
            let listed: Vec<&str> = table.iter().flat_map(|(_, f)| f.split_whitespace()).map(|f| f.trim_start_matches('~')).collect();
            for f in &fields {
                if listed.iter().filter(|l| **l == f.key).count() != 1 {
                    problems.push(format!("{name}: {} must be in one section (the one without a title: not shown)", f.key));
                }
            }
            for l in &listed {
                if !fields.iter().any(|f| f.key == *l) {
                    problems.push(format!("{name}: {l} isn't a field"));
                }
            }
        }
        assert!(problems.is_empty(), "crates/editor-core/src/schema.rs, SECTIONS:\n{}", problems.join("\n"));
    }

    /// Something the engine's renames reach: a definition, or a placed object, zone or
    /// trigger of a map, or the project's settings.
    #[derive(Clone)]
    enum Sample {
        Def(RefKind, Value),
        Entity(Value),
        Zone(Value),
        Trigger(Value),
        Settings(Value),
        Project(Value),
    }

    impl Sample {
        fn node(&self) -> Node<'static> {
            match self {
                Sample::Def(kind, _) => of_kind(*kind).unwrap(),
                Sample::Entity(_) => entity(),
                Sample::Zone(_) => zone(),
                Sample::Trigger(_) => trigger(),
                Sample::Settings(_) => map_settings(),
                Sample::Project(_) => project(),
            }
        }

        fn value(&self) -> &Value {
            match self {
                Sample::Def(_, v) | Sample::Entity(v) | Sample::Zone(v) | Sample::Trigger(v) | Sample::Settings(v) | Sample::Project(v) => v,
            }
        }

        /// The sample after each kind's id `old` was renamed, for the kinds that changed it.
        fn renamed(&self, old: &str) -> Vec<(RefKind, Value)> {
            use rustpg2d_engine::defs::Def;
            use rustpg2d_engine::map::Map;
            use rustpg2d_engine::refactor::{rename_in_db, rename_in_map};
            let new = "probe__";
            let mut out = Vec::new();
            let (mut db, mut config, mut map) = (Database::default(), ProjectConfig::new("probe"), Map::new("probe", 4, 4));
            let v = self.value();
            let bad = |e: serde_json::Error| -> String { format!("a made-up value the engine doesn't read: {e}\n{v}") };
            match self {
                Sample::Def(of, _) => rustpg2d_engine::with_def_kind!(*of, |T| T::list_mut(&mut db).push(serde_json::from_value::<T>(v.clone()).unwrap_or_else(|e| panic!("{}", bad(e)))), else => unreachable!()),
                Sample::Project(_) => config = serde_json::from_value(v.clone()).unwrap_or_else(|e| panic!("{}", bad(e))),
                Sample::Entity(_) => map.entities.push(serde_json::from_value(v.clone()).unwrap_or_else(|e| panic!("{}", bad(e)))),
                Sample::Zone(_) => map.zones.push(serde_json::from_value(v.clone()).unwrap_or_else(|e| panic!("{}", bad(e)))),
                Sample::Trigger(_) => map.triggers.push(serde_json::from_value(v.clone()).unwrap_or_else(|e| panic!("{}", bad(e)))),
                Sample::Settings(_) => serde_json::from_value::<rustpg2d_engine::map::MapSettings>(v.clone()).unwrap_or_else(|e| panic!("{}", bad(e))).apply(&mut map),
            }
            for kind in RefKind::ALL {
                match self {
                    Sample::Def(of, _) => {
                        let mut db = db.clone();
                        if rename_in_db(&mut db, &mut config.clone(), kind, old, new) > 0 {
                            out.push((kind, rustpg2d_engine::with_def_kind!(*of, |T| serde_json::to_value(&T::list(&db)[0]).unwrap(), else => unreachable!())));
                        }
                    }
                    Sample::Project(_) => {
                        let mut config = config.clone();
                        if rename_in_db(&mut db.clone(), &mut config, kind, old, new) > 0 {
                            out.push((kind, serde_json::to_value(&config).unwrap()));
                        }
                    }
                    Sample::Entity(_) | Sample::Zone(_) | Sample::Trigger(_) | Sample::Settings(_) => {
                        let mut map = map.clone();
                        if rename_in_map(&mut map, kind, old, new) > 0 {
                            out.push((
                                kind,
                                match self {
                                    Sample::Entity(_) => serde_json::to_value(&map.entities[0]),
                                    Sample::Zone(_) => serde_json::to_value(&map.zones[0]),
                                    Sample::Settings(_) => serde_json::to_value(rustpg2d_engine::map::MapSettings::of(&map)),
                                    _ => serde_json::to_value(&map.triggers[0]),
                                }
                                .unwrap(),
                            ));
                        }
                    }
                }
            }
            out
        }
    }

    /// A value with everything in it: every field set, every list with an entry (a list of an
    /// enum: one per variant), every text "p". Enums that aren't in a list take variant `round`
    /// (modulo their number), so some round shows each. An enum inside itself stays empty.
    fn full(node: Node, round: usize, inside: &mut Vec<String>) -> Value {
        let ty = node.resolved().ty.to_string();
        match node.shape() {
            Shape::Text => Value::String("p".into()),
            Shape::Color => Value::String("#ffffff".into()),
            Shape::Int { min, max } => Value::from(1.clamp(min.unwrap_or(1).min(1), max.unwrap_or(1).max(1))),
            Shape::Float => Value::from(1.0),
            Shape::Bool => Value::Bool(true),
            // One of several forms (an untagged enum): the first.
            Shape::Any => {
                let n = node.resolved();
                n.at.get("anyOf").and_then(Value::as_array).and_then(|a| a.first()).map_or(Value::Null, |first| full(Node { at: first, ..n }, round, inside))
            }
            Shape::Optional(inner) => full(inner, round, inside),
            Shape::Fixed(nodes) => Value::Array(nodes.iter().map(|n| full(*n, round, inside)).collect()),
            Shape::Map(inner) => Value::Object([("p".to_string(), full(inner, round, inside))].into_iter().collect()),
            Shape::Struct(fields) => Value::Object(fields.iter().map(|f| (f.key.to_string(), full(f.node, round, inside))).collect()),
            Shape::Choice(variants) => {
                if inside.contains(&ty) {
                    return node.blank();
                }
                inside.push(ty);
                LONE.set(LONE.get().max(variants.len()));
                let v = &variants[round % variants.len()];
                let out = match v.data {
                    None => Value::String(v.name.to_string()),
                    Some(n) => Value::Object([(v.name.to_string(), full(n, round, inside))].into_iter().collect()),
                };
                inside.pop();
                out
            }
            Shape::List(inner) => {
                let of = inner.resolved().ty.to_string();
                match inner.shape() {
                    Shape::Choice(_) if inside.contains(&of) => Value::Array(Vec::new()),
                    Shape::Choice(variants) => {
                        inside.push(of);
                        let all = variants.iter().map(|v| match v.data {
                            None => Value::String(v.name.to_string()),
                            Some(n) => Value::Object([(v.name.to_string(), full(n, round, inside))].into_iter().collect()),
                        });
                        let all = Value::Array(all.collect());
                        inside.pop();
                        all
                    }
                    _ => Value::Array(vec![full(inner, round, inside)]),
                }
            }
        }
    }

    thread_local! {
        /// The most variants an enum had that [`full`] met standing alone (not in a list).
        static LONE: std::cell::Cell<usize> = const { std::cell::Cell::new(1) };
    }

    /// Made-up samples that have every field and every variant of everything ([`full`]): as
    /// many of each kind as its biggest lone enum has variants.
    fn made_up() -> Vec<Sample> {
        let mut out = Vec::new();
        let mut all = |node: Node<'static>, make: &dyn Fn(Value) -> Sample| {
            LONE.set(1);
            let mut round = 0;
            while round < LONE.get() {
                out.push(make(full(node, round, &mut Vec::new())));
                round += 1;
            }
        };
        for kind in RefKind::ALL {
            if let Some(node) = of_kind(kind) {
                all(node, &|v| Sample::Def(kind, v));
            }
        }
        all(project(), &Sample::Project);
        all(entity(), &Sample::Entity);
        all(zone(), &Sample::Zone);
        all(trigger(), &Sample::Trigger);
        all(map_settings(), &Sample::Settings);
        out
    }

    /// The texts of a sample that are ids the engine's renames follow: (site, kind). Found by
    /// renaming: a text that changes when `kind`'s id of that name is renamed names it.
    fn renamed_sites(sample: &Sample) -> BTreeSet<(String, RefKind)> {
        let node = sample.node();
        let mut before: Vec<(String, String, String)> = Vec::new();
        texts(node, sample.value(), &mut |site, pointer, text| before.push((site.to_string(), pointer.to_string(), text.to_string())));
        let ids: BTreeSet<&str> = before.iter().map(|(_, _, t)| t.as_str()).filter(|t| crate::rename::valid_name(t)).collect();
        let mut found = BTreeSet::new();
        for id in ids {
            for (kind, now) in sample.renamed(id) {
                for (site, pointer, text) in &before {
                    // (A text that is gone lies under a renamed key: the key is what changed.)
                    let still = match pointer.strip_suffix("#key") {
                        Some(entry) => now.pointer(entry).is_some(),
                        None => now.pointer(pointer).is_none_or(|t| t.as_str() == Some(id)),
                    };
                    if text == id && !still {
                        found.insert((site.clone(), kind));
                    }
                }
            }
        }
        found
    }

    #[test]
    fn references_agree_with_renames() {
        let mut found: BTreeMap<String, BTreeSet<RefKind>> = BTreeMap::new();
        let mut seen: BTreeSet<String> = BTreeSet::new();
        for sample in made_up() {
            for (site, kind) in renamed_sites(&sample) {
                found.entry(site).or_default().insert(kind);
            }
            texts(sample.node(), sample.value(), &mut |site, _, text| {
                if !text.is_empty() {
                    seen.insert(site.to_string());
                }
            });
        }
        let mut problems = Vec::new();
        for site in seen.iter().filter(|s| !BY_CONTEXT.contains(&s.as_str())) {
            let table: BTreeSet<RefKind> = REFERENCES.iter().find(|(s, _)| s == site).map(|(_, k)| k.iter().copied().collect()).unwrap_or_default();
            let renames = found.get(site).cloned().unwrap_or_default();
            if renames != table {
                let kinds: Vec<String> = renames.iter().map(|k| format!("{k:?}")).collect();
                problems.push(format!("        (\"{site}\", &[{}]),{}", kinds.join(", "), if table.is_empty() { String::new() } else { format!(" // the table says {table:?}") }));
            }
            if !renames.is_empty() && NOT_FOLLOWED.iter().any(|(s, _)| s == site) {
                problems.push(format!("        {site} is followed by renames now: take it out of NOT_FOLLOWED"));
            }
        }
        for (site, _) in REFERENCES.iter().chain(NOT_FOLLOWED) {
            if !seen.contains(*site) {
                problems.push(format!("        {site} isn't a text field (any more)"));
            }
        }
        assert!(problems.is_empty(), "crates/editor-core/src/schema.rs, REFERENCES should have:\n{}", problems.join("\n"));
    }
}
