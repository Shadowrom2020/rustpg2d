//! Entities placed on a map: props, characters and item pickups.

use serde::{Deserialize, Serialize};

use crate::events::Trigger;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum EntityKind {
    Prop,
    Character,
    Item,
}

impl EntityKind {
    pub const ALL: [EntityKind; 3] = [EntityKind::Prop, EntityKind::Character, EntityKind::Item];
    pub fn label(self) -> &'static str {
        match self {
            EntityKind::Prop => "Prop",
            EntityKind::Character => "Character",
            EntityKind::Item => "Item",
        }
    }
}

/// Facing direction. The discriminant is the row in character sprite sheets.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Facing {
    #[default]
    Down = 0,
    Left = 1,
    Right = 2,
    Up = 3,
}

impl Facing {
    pub const ALL: [Facing; 4] = [Facing::Down, Facing::Left, Facing::Right, Facing::Up];
    pub fn from_vec(dx: f32, dy: f32) -> Option<Facing> {
        if dx == 0.0 && dy == 0.0 {
            None
        } else if dx.abs() > dy.abs() {
            Some(if dx < 0.0 { Facing::Left } else { Facing::Right })
        } else {
            Some(if dy < 0.0 { Facing::Up } else { Facing::Down })
        }
    }
    pub fn dir(self) -> (f32, f32) {
        match self {
            Facing::Down => (0.0, 1.0),
            Facing::Left => (-1.0, 0.0),
            Facing::Right => (1.0, 0.0),
            Facing::Up => (0.0, -1.0),
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            Facing::Down => "Down",
            Facing::Left => "Left",
            Facing::Right => "Right",
            Facing::Up => "Up",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct EntityInstance {
    pub uid: u32,
    pub kind: EntityKind,
    /// Id of the PropDef / CharacterDef / ItemDef.
    pub def: String,
    /// Optional name, used to address the entity from actions and scripts.
    #[serde(default)]
    pub name: String,
    /// Feet position in pixels (bottom-center of the sprite).
    pub x: f32,
    pub y: f32,
    #[serde(default)]
    pub facing: Facing,
    /// Inactive entities are hidden and not interactable (e.g. spawned later by an event).
    #[serde(default = "yes")]
    pub active: bool,
    /// Legacy prop frame offset (converted to `state` when the map is loaded).
    #[serde(default)]
    pub frame: u32,
    /// State a prop starts in ("" = its definition's default state).
    #[serde(default)]
    pub state: String,
    /// Stack size for item pickups.
    #[serde(default = "one")]
    pub count: u32,
    /// Instance triggers, run in addition to the definition's triggers.
    #[serde(default)]
    pub triggers: Vec<Trigger>,
    /// Overrides the prop definition's container (size, contents, lock).
    #[serde(default)]
    pub container: Option<crate::defs::ContainerSpec>,
    /// Overrides the prop definition's key lock.
    #[serde(default)]
    pub lock: Option<crate::defs::LockSpec>,
    /// Characters: loot table used instead of the definition's (None = definition's).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub loot: Option<Vec<crate::defs::LootEntry>>,
    /// Characters: a route the NPC walks along (instead of its definition's behaviour).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub patrol: Option<PatrolRoute>,
    /// Characters: a daily routine of places to be at (instead of the patrol route; the
    /// definition's behaviour applies at each stop).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub schedule: Vec<ScheduleStop>,
    /// Overrides the character definition's merchant spec (stock, prices).
    #[serde(default)]
    pub shop: Option<crate::defs::ShopSpec>,
    /// Light of this placed entity, instead of its definition's (props: of its current state).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub light: Option<crate::defs::LightSpec>,
    /// Characters: layered look instead of the definition's.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub appearance: Option<crate::defs::Appearance>,
    /// Characters: equipment instead of the definition's.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub equipment: Option<crate::state::Equipment>,
    /// Characters: carried items instead of the definition's.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub inventory: Option<Vec<crate::inventory::ItemStack>>,
    /// The placed character's own portrait (None = its definition's).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub portrait: Option<crate::defs::SpriteRef>,
}

fn yes() -> bool {
    true
}
fn one() -> u32 {
    1
}

/// How a patrol continues after its last waypoint.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum PatrolMode {
    /// Back to the first waypoint and around again.
    #[default]
    Loop,
    /// Turn around and walk the waypoints in reverse.
    PingPong,
}

impl PatrolMode {
    pub fn label(self) -> &'static str {
        match self {
            PatrolMode::Loop => "Loop",
            PatrolMode::PingPong => "Back and forth",
        }
    }
}

/// A patrol route: tile waypoints a character walks between (finding its way around
/// obstacles), pausing at each one.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PatrolRoute {
    /// Waypoints in tile coordinates.
    pub points: Vec<(i32, i32)>,
    #[serde(default)]
    pub mode: PatrolMode,
    /// Seconds to wait at each waypoint.
    #[serde(default = "default_pause")]
    pub pause: f32,
    /// Walking speed in pixels per second (0 = the character's speed).
    #[serde(default)]
    pub speed: f32,
}

fn default_pause() -> f32 {
    1.0
}

impl Default for PatrolRoute {
    fn default() -> Self {
        PatrolRoute { points: Vec::new(), mode: PatrolMode::Loop, pause: default_pause(), speed: 0.0 }
    }
}

impl PatrolRoute {
    /// The waypoint after `idx` (and the new direction for back-and-forth routes).
    pub fn next(&self, idx: usize, back: bool) -> (usize, bool) {
        let n = self.points.len();
        if n < 2 {
            return (0, false);
        }
        match self.mode {
            PatrolMode::Loop => ((idx + 1) % n, false),
            PatrolMode::PingPong if back && idx == 0 => (1, false),
            PatrolMode::PingPong if back => (idx - 1, true),
            PatrolMode::PingPong if idx + 1 >= n => (n - 2, true),
            PatrolMode::PingPong => (idx + 1, false),
        }
    }
}

/// One stop of a character's daily routine: from `hour` on, the character walks to the tile
/// and stays there until the next stop's hour (the last stop lasts over midnight until the
/// first one).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ScheduleStop {
    /// In-game hour the stop starts (0..24; 7.5 = 07:30).
    pub hour: f32,
    /// Tile the character goes to.
    pub x: i32,
    pub y: i32,
    /// Direction it faces once there (None = as it arrived).
    #[serde(default)]
    pub facing: Option<Facing>,
    /// It goes indoors there: out of sight (and out of reach) until the next stop.
    #[serde(default)]
    pub inside: bool,
    /// The map of the stop ("" = the character's own map): it walks to the door / portal of
    /// its map leading there and turns up on that map as a visitor.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub map: String,
    /// Furniture to use there: a prop's name on the stop's map (sits / lies on it); "" = none.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub prop: String,
}

impl ScheduleStop {
    pub fn new(hour: f32, x: i32, y: i32) -> Self {
        ScheduleStop { hour, x, y, facing: None, inside: false, map: String::new(), prop: String::new() }
    }

    /// "07:30".
    pub fn time_text(&self) -> String {
        hour_text(self.hour)
    }
}

/// "07:30" of an hour of the day.
pub fn hour_text(hour: f32) -> String {
    let m = (hour.rem_euclid(24.0) * 60.0).round() as u32 % (24 * 60);
    format!("{:02}:{:02}", m / 60, m % 60)
}

/// Index of the stop that is current at `hour`: the last one started (stops in any order; the
/// latest of the day carries over midnight). None without stops.
pub fn current_stop(stops: &[ScheduleStop], hour: f32) -> Option<usize> {
    let h = |k: usize| stops[k].hour.rem_euclid(24.0);
    let now = hour.rem_euclid(24.0);
    let latest = |it: &mut dyn Iterator<Item = usize>| it.max_by(|&a, &b| h(a).total_cmp(&h(b)));
    latest(&mut (0..stops.len()).filter(|&k| h(k) <= now)).or_else(|| latest(&mut (0..stops.len())))
}

impl EntityInstance {
    pub fn new(uid: u32, kind: EntityKind, def: &str, x: f32, y: f32) -> Self {
        EntityInstance {
            uid,
            kind,
            def: def.to_string(),
            name: String::new(),
            x,
            y,
            facing: Facing::Down,
            active: true,
            frame: 0,
            state: String::new(),
            count: 1,
            triggers: Vec::new(),
            container: None,
            lock: None,
            shop: None,
            light: None,
            appearance: None,
            equipment: None,
            inventory: None,
            patrol: None,
            schedule: Vec::new(),
            portrait: None,
            loot: None,
        }
    }

    /// Display label for editor lists.
    pub fn label(&self) -> String {
        if self.name.is_empty() {
            format!("#{} {}", self.uid, self.def)
        } else {
            format!("#{} {} ({})", self.uid, self.name, self.def)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schedule_stop_of_the_hour() {
        let stops = vec![ScheduleStop::new(20.0, 1, 1), ScheduleStop::new(7.5, 2, 2), ScheduleStop::new(12.0, 3, 3)];
        assert_eq!(current_stop(&stops, 8.0), Some(1));
        assert_eq!(current_stop(&stops, 7.5), Some(1), "starts on the hour");
        assert_eq!(current_stop(&stops, 13.0), Some(2));
        assert_eq!(current_stop(&stops, 23.9), Some(0));
        assert_eq!(current_stop(&stops, 3.0), Some(0), "the evening stop lasts over midnight");
        assert_eq!(current_stop(&[], 3.0), None);
        assert_eq!(hour_text(7.5), "07:30");
        assert_eq!(hour_text(23.999), "00:00");
    }

    #[test]
    fn old_entities_have_no_schedule() {
        let e: EntityInstance = ron::from_str("(uid: 3, kind: Character, def: \"villager\", x: 1.0, y: 2.0)").unwrap();
        assert!(e.schedule.is_empty());
        let mut with = e.clone();
        with.schedule = vec![ScheduleStop { inside: true, ..ScheduleStop::new(21.0, 4, 5) }];
        let back: EntityInstance = ron::from_str(&ron::to_string(&with).unwrap()).unwrap();
        assert_eq!(back, with);
        assert!(!ron::to_string(&e).unwrap().contains("schedule"), "no schedule, nothing written");
    }
}
