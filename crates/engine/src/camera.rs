//! Camera moves (*Camera* action, `camera_to` / `camera_follow` / `camera_reset`): what the camera
//! looks at during an event or cut-scene. `runtime/camera.rs` pans and zooms to it and hands the
//! camera back to the player when the event is over.

use serde::{Deserialize, Serialize};

/// What the camera looks at.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub enum CameraAim {
    /// The player (as usual).
    #[default]
    Player,
    /// The middle of a tile of the map.
    Tile { x: i32, y: i32 },
    /// A character or prop ("self", a name), following it as it moves.
    Follow { target: String },
}

impl CameraAim {
    pub fn label(&self) -> &'static str {
        match self {
            CameraAim::Player => "The player",
            CameraAim::Tile { .. } => "A tile",
            CameraAim::Follow { .. } => "Follow a character",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn camera_aims_read_back() {
        for a in [CameraAim::Player, CameraAim::Tile { x: 3, y: -1 }, CameraAim::Follow { target: "guard".into() }] {
            assert_eq!(ron::from_str::<CameraAim>(&ron::to_string(&a).unwrap()).unwrap(), a);
        }
    }
}
