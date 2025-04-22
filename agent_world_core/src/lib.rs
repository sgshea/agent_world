use serde::{Deserialize, Serialize};

pub mod agent;
pub mod environment;
pub mod map;

/// Unique identifier for entities (agents, items, etc.).
pub type EntityId = usize;

/// Represents a 2D coordinate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Position {
    pub x: usize,
    pub y: usize,
}

/// Represents the specific type (color) of a door or key.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DoorKeyType {
    Red,
    Green,
    Blue,
    Yellow,
}

/// Represents items that can exist in the environment or agent inventories.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Item {
    Key { key_type: DoorKeyType },
    Chip,
    Goal,
}
