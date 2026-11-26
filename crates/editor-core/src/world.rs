//! The world as its maps and the ways between them: what the World view shows. Every map with
//! a small picture (a pixel a tile) and its portals, transitions and edges; which maps nothing
//! leads to; which ways lead nowhere.

use rustpg2d_engine::map::{Edge, Map, ZoneKind};
use rustpg2d_engine::render::Overview;

use crate::workspace::Workspace;

/// How a way leaves a map.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Way {
    /// A portal zone: stepping in arrives on a spawn zone.
    Portal,
    /// A transition zone, one of a pair.
    Transition,
    /// The map's edge: walking off it arrives on the neighbour.
    Edge,
}

/// A way from one map to another.
#[derive(Clone, Debug, PartialEq)]
pub struct Link {
    pub way: Way,
    /// The zone it starts in ("" for an edge) and the middle of that place, in tiles.
    pub zone: String,
    pub from: (f32, f32),
    pub to_map: String,
    /// The spawn or transition zone it arrives on ("" for an edge or when none is named), and
    /// its middle on the other map (`None`: there is no such zone there).
    pub to_zone: String,
    pub to: Option<(f32, f32)>,
    /// What is wrong with it ("" = nothing): no target, a map that isn't there, a transition
    /// whose other side doesn't lead back.
    pub problem: String,
}

/// One map of the world.
pub struct WorldMap {
    pub name: String,
    pub width: u32,
    pub height: u32,
    /// A pixel a tile, top row first (RGBA).
    pub picture: Vec<u8>,
    pub links: Vec<Link>,
    /// Does any other map lead here (or is it where the game starts)?
    pub reached: bool,
    /// How many maps lie between the start map and this one (the start map: 0; `None`: no way).
    pub distance: Option<u32>,
}

fn middle(z: &rustpg2d_engine::map::Zone) -> (f32, f32) {
    (z.x as f32 + z.w as f32 / 2.0, z.y as f32 + z.h as f32 / 2.0)
}

impl Workspace {
    /// Every map of the project as the World view shows it (open maps as they are now, the
    /// others from their files).
    pub fn survey_world(&self) -> Vec<WorldMap> {
        let maps: Vec<Map> = self
            .maps
            .iter()
            .filter_map(|name| match self.open_maps().into_iter().find(|d| d.map.name == *name) {
                Some(doc) => Some(doc.map.clone()),
                None => self.project.load_map(name).ok(),
            })
            .collect();
        let find = |name: &str| maps.iter().find(|m| m.name == name);
        let mut world: Vec<WorldMap> = maps
            .iter()
            .map(|map| {
                let mut links = Vec::new();
                for z in &map.zones {
                    let (way, to_map, to_zone) = match &z.kind {
                        ZoneKind::Portal { target_map, target_spawn } => (Way::Portal, target_map, target_spawn),
                        ZoneKind::Transition { target_map, target_zone } => (Way::Transition, target_map, target_zone),
                        _ => continue,
                    };
                    let other = find(to_map);
                    let there = other.and_then(|o| o.zones.iter().find(|x| x.name == *to_zone));
                    let problem = match (way, other, there) {
                        (_, _, _) if to_map.is_empty() => "it leads nowhere yet".to_string(),
                        (_, None, _) => format!("there is no map \"{to_map}\""),
                        (Way::Portal, Some(_), None) => format!("{to_map} has no spawn zone \"{to_zone}\""),
                        (Way::Portal, Some(_), Some(x)) if x.kind != ZoneKind::Spawn => format!("\"{to_zone}\" on {to_map} isn't a spawn zone"),
                        (Way::Transition, Some(_), None) => format!("{to_map} has no zone \"{to_zone}\""),
                        (Way::Transition, Some(_), Some(x)) if !matches!(&x.kind, ZoneKind::Transition { target_map: back, target_zone: zone } if *back == map.name && *zone == z.name) => {
                            format!("\"{to_zone}\" on {to_map} doesn't lead back here")
                        }
                        _ => String::new(),
                    };
                    links.push(Link { way, zone: z.name.clone(), from: middle(z), to_map: to_map.clone(), to_zone: to_zone.clone(), to: there.map(middle), problem });
                }
                for e in Edge::ALL {
                    let Some(l) = map.edges.get(e) else { continue };
                    let (w, h) = (map.width as f32, map.height as f32);
                    let from = match e {
                        Edge::North => (w / 2.0, 0.0),
                        Edge::South => (w / 2.0, h),
                        Edge::East => (w, h / 2.0),
                        Edge::West => (0.0, h / 2.0),
                    };
                    // (It arrives on the opposite edge of the neighbour.)
                    let to = find(&l.target_map).map(|o| {
                        let (ow, oh) = (o.width as f32, o.height as f32);
                        match e {
                            Edge::North => (ow / 2.0, oh),
                            Edge::South => (ow / 2.0, 0.0),
                            Edge::East => (0.0, oh / 2.0),
                            Edge::West => (ow, oh / 2.0),
                        }
                    });
                    let problem = if to.is_none() { format!("there is no map \"{}\"", l.target_map) } else { String::new() };
                    links.push(Link { way: Way::Edge, zone: String::new(), from, to_map: l.target_map.clone(), to_zone: String::new(), to, problem });
                }
                let picture = Overview::with_decor(map, &self.assets, &self.project.db).image.bytes;
                WorldMap { name: map.name.clone(), width: map.width, height: map.height, picture, links, reached: false, distance: None }
            })
            .collect();
        // Which maps the others lead to, and how far each is from where the game starts.
        let start = self.project.config.start_map.clone();
        let targets: Vec<Vec<String>> = world.iter().map(|m| m.links.iter().map(|l| l.to_map.clone()).collect()).collect();
        for (k, map) in world.iter_mut().enumerate() {
            map.reached = map.name == start || targets.iter().enumerate().any(|(j, t)| j != k && t.contains(&map.name));
        }
        let mut frontier = vec![start];
        let mut depth = 0;
        while !frontier.is_empty() {
            let mut next = Vec::new();
            for name in frontier {
                let Some(k) = world.iter().position(|m| m.name == name && m.distance.is_none()) else { continue };
                world[k].distance = Some(depth);
                next.extend(targets[k].iter().cloned());
            }
            frontier = next;
            depth += 1;
        }
        world
    }

    /// Makes two transition zones each other's other side: `a` and `b` are (map, zone). Both
    /// maps are opened when they aren't; each change is a step of its map.
    pub fn link_transitions(&mut self, a: (&str, &str), b: (&str, &str)) -> Result<(), String> {
        if a.0 == b.0 {
            return Err("a transition joins two different maps (on one map, a portal leads to a spawn zone)".into());
        }
        let back = self.doc.as_ref().map(|d| d.map.name.clone());
        for (here, there) in [(a, b), (b, a)] {
            self.open_map(here.0);
            let Some(doc) = self.doc.as_mut().filter(|d| d.map.name == here.0) else { return Err(format!("{} can't be opened", here.0)) };
            let before = crate::doc::Objects::of(&doc.map);
            if !crate::mapops::set_target(&mut doc.map, here.1, there.0, there.1) {
                return Err(format!("{} has no zone \"{}\"", here.0, here.1));
            }
            doc.record_objects(before, "link");
        }
        if let Some(back) = back {
            self.open_map(&back);
        }
        self.begin_unnamed(&format!("Link {} and {}", a.0, b.0));
        self.stale = true;
        Ok(())
    }
}
