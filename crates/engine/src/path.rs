//! A* pathfinding on the tile grid (used for click-to-move).

use std::cell::Cell;
use std::cmp::Reverse;
use std::collections::{BinaryHeap, HashMap};
use std::hash::{BuildHasherDefault, Hasher};

pub type TilePos = (i32, i32);

/// A hash for tile coordinates (two `i32`s): one multiply instead of SipHash, which dominated
/// the search time of long routes.
#[derive(Default)]
struct TileHash(u64);

impl Hasher for TileHash {
    fn finish(&self) -> u64 {
        self.0
    }
    fn write(&mut self, bytes: &[u8]) {
        for b in bytes {
            self.0 = (self.0 ^ u64::from(*b)).wrapping_mul(0x0100_0000_01b3);
        }
    }
    fn write_i32(&mut self, i: i32) {
        self.0 = (self.0.rotate_left(32) ^ u64::from(i as u32)).wrapping_mul(0x9E37_79B9_7F4A_7C15);
    }
}

type TileMap<V> = HashMap<TilePos, V, BuildHasherDefault<TileHash>>;

/// What a search came to ([`find_path_ex`]).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Search {
    Found(Vec<TilePos>),
    /// Every reachable tile was tried: there is no way. (Trying again with fewer obstacles can help.)
    NoRoute,
    /// The node budget ran out first: the goal may be reachable but far, or walled in on a big
    /// map. (Trying again with the same budget would run out too.)
    OutOfBudget,
}

thread_local! {
    // Searches since the last `take_stats` and the nodes they expanded (performance checks: the
    // game's `status` prints them as `astar=calls/nodes`).
    static CALLS: Cell<u32> = const { Cell::new(0) };
    static NODES: Cell<u32> = const { Cell::new(0) };
}

/// Path searches and the nodes they expanded since the last call (then counts from 0 again).
pub fn take_stats() -> (u32, u32) {
    (CALLS.take(), NODES.take())
}

const STRAIGHT: u32 = 10;
const DIAGONAL: u32 = 14;

fn heuristic(a: TilePos, b: TilePos) -> u32 {
    let (dx, dy) = ((a.0 - b.0).unsigned_abs(), (a.1 - b.1).unsigned_abs());
    STRAIGHT * dx.max(dy) + (DIAGONAL - STRAIGHT) * dx.min(dy)
}

/// Finds a path from `start` to the first tile where `is_goal` holds, heading for `target`.
///
/// `blocked(x, y)` must return true for tiles that can't be entered (including
/// out-of-bounds). Diagonal steps are only taken when both adjacent straight tiles
/// are free, so actors never clip wall corners. Gives up after `max_nodes` expansions.
/// The returned path excludes `start` and ends at the goal tile.
pub fn find_path(
    start: TilePos,
    target: TilePos,
    is_goal: impl Fn(TilePos) -> bool,
    blocked: impl Fn(i32, i32) -> bool,
    max_nodes: usize,
) -> Option<Vec<TilePos>> {
    match find_path_ex(start, target, is_goal, blocked, max_nodes) {
        Search::Found(p) => Some(p),
        Search::NoRoute | Search::OutOfBudget => None,
    }
}

/// [`find_path`] that says *why* no path was found: whether another search with fewer
/// obstacles is worth it ([`Search::NoRoute`]) or would run out of budget again.
pub fn find_path_ex(
    start: TilePos,
    target: TilePos,
    is_goal: impl Fn(TilePos) -> bool,
    blocked: impl Fn(i32, i32) -> bool,
    max_nodes: usize,
) -> Search {
    CALLS.with(|c| c.set(c.get() + 1));
    if is_goal(start) {
        return Search::Found(Vec::new());
    }
    let mut open = BinaryHeap::with_capacity(64);
    let mut came_from: TileMap<TilePos> = TileMap::with_capacity_and_hasher(256, Default::default());
    let mut cost: TileMap<u32> = TileMap::with_capacity_and_hasher(256, Default::default());
    cost.insert(start, 0);
    open.push(Reverse((heuristic(start, target), 0u32, start)));
    let mut expanded = 0;
    while let Some(Reverse((_, g, cur))) = open.pop() {
        if g > cost[&cur] {
            continue; // stale entry
        }
        if is_goal(cur) {
            let mut path = vec![cur];
            let mut p = cur;
            while let Some(&prev) = came_from.get(&p) {
                if prev == start {
                    break;
                }
                path.push(prev);
                p = prev;
            }
            path.reverse();
            return Search::Found(path);
        }
        expanded += 1;
        NODES.with(|n| n.set(n.get() + 1));
        if expanded > max_nodes {
            return Search::OutOfBudget;
        }
        for (dx, dy) in [(1, 0), (-1, 0), (0, 1), (0, -1), (1, 1), (1, -1), (-1, 1), (-1, -1)] {
            let next = (cur.0 + dx, cur.1 + dy);
            if blocked(next.0, next.1) {
                continue;
            }
            let diagonal = dx != 0 && dy != 0;
            if diagonal && (blocked(cur.0 + dx, cur.1) || blocked(cur.0, cur.1 + dy)) {
                continue;
            }
            let ng = g + if diagonal { DIAGONAL } else { STRAIGHT };
            if cost.get(&next).is_none_or(|&c| ng < c) {
                cost.insert(next, ng);
                came_from.insert(next, cur);
                open.push(Reverse((ng + heuristic(next, target), ng, next)));
            }
        }
    }
    Search::NoRoute
}

#[cfg(test)]
mod tests {
    use super::*;

    fn grid<'a>(rows: &'a [&'a str]) -> impl Fn(i32, i32) -> bool + 'a {
        move |x, y| {
            y < 0 || x < 0 || y as usize >= rows.len() || x as usize >= rows[0].len() || rows[y as usize].as_bytes()[x as usize] == b'#'
        }
    }

    /// No route and no budget are told apart: a second search with fewer obstacles only makes
    /// sense when the first really tried everything.
    #[test]
    fn no_route_or_out_of_budget() {
        let rows = ["....", ".###", ".#X#", ".###"]; // X is walled in
        let blocked = grid(&rows);
        assert_eq!(find_path_ex((0, 0), (2, 2), |t| t == (2, 2), &blocked, 1000), Search::NoRoute);
        let open = grid(&["...........", "...........", "..........."]);
        assert_eq!(find_path_ex((0, 0), (10, 2), |t| t == (10, 2), &open, 2), Search::OutOfBudget);
        assert!(matches!(find_path_ex((0, 0), (10, 2), |t| t == (10, 2), &open, 1000), Search::Found(p) if p.len() == 10));
        assert_eq!(find_path_ex((0, 0), (0, 0), |t| t == (0, 0), &open, 1), Search::Found(vec![]));
    }

    #[test]
    fn walks_around_walls_without_cutting_corners() {
        let rows = ["....#....", "....#....", "....#....", ".........", "........."];
        let path = find_path((0, 0), (8, 0), |p| p == (8, 0), grid(&rows), 1000).unwrap();
        assert_eq!(*path.last().unwrap(), (8, 0));
        assert!(path.iter().all(|&(x, y)| !grid(&rows)(x, y)));
        // Must pass below the wall (row >= 3).
        assert!(path.iter().any(|&(x, y)| x == 4 && y >= 3));
        // Consecutive steps are neighbours.
        let mut prev = (0, 0);
        for &p in &path {
            assert!((p.0 - prev.0).abs() <= 1 && (p.1 - prev.1).abs() <= 1);
            prev = p;
        }
    }

    #[test]
    fn goal_predicate_and_unreachable() {
        let rows = ["..#..", "..#..", "..#.."];
        // Unreachable target.
        assert!(find_path((0, 0), (4, 0), |p| p == (4, 0), grid(&rows), 1000).is_none());
        // "Next to (2,1)" is reachable even though (2,1) itself is a wall.
        let adj = |p: TilePos| (p.0 - 2).abs() <= 1 && (p.1 - 1).abs() <= 1 && p != (2, 1);
        let path = find_path((0, 2), (2, 1), adj, grid(&rows), 1000).unwrap();
        assert!(adj(*path.last().unwrap()));
        assert!(find_path((1, 1), (2, 1), adj, grid(&rows), 1000).unwrap().is_empty());
    }

    #[test]
    fn node_budget_limits_search() {
        let open_field = |x: i32, y: i32| x < 0 || y < 0 || x >= 1024 || y >= 1024;
        assert!(find_path((0, 0), (1000, 1000), |p| p == (1000, 1000), open_field, 50).is_none());
        let p = find_path((0, 0), (300, 200), |p| p == (300, 200), open_field, 100_000).unwrap();
        assert_eq!(p.len(), 300);
    }
}
