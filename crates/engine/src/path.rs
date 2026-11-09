//! A* pathfinding on the tile grid (used for click-to-move).

use std::cell::Cell;
use std::cmp::Reverse;
use std::collections::{BinaryHeap, HashMap};

pub type TilePos = (i32, i32);

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
    CALLS.with(|c| c.set(c.get() + 1));
    if is_goal(start) {
        return Some(Vec::new());
    }
    let mut open = BinaryHeap::new();
    let mut came_from: HashMap<TilePos, TilePos> = HashMap::new();
    let mut cost: HashMap<TilePos, u32> = HashMap::new();
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
            return Some(path);
        }
        expanded += 1;
        NODES.with(|n| n.set(n.get() + 1));
        if expanded > max_nodes {
            return None;
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
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn grid<'a>(rows: &'a [&'a str]) -> impl Fn(i32, i32) -> bool + 'a {
        move |x, y| {
            y < 0 || x < 0 || y as usize >= rows.len() || x as usize >= rows[0].len() || rows[y as usize].as_bytes()[x as usize] == b'#'
        }
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
