use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::{DoorKeyType, EntityId, Item, Position, agent::Agent, map::Grid};

/// Represents the static type of a cell in the environment grid.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum CellType {
    Floor,
    Wall,
    Door {
        open: bool,
        /// The type of key required, if any.
        door_type: Option<DoorKeyType>,
    },
}

impl Default for CellType {
    fn default() -> Self {
        CellType::Floor
    }
}

/// Represents actions an agent can decide to take.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Action {
    Wait,
    Move { dx: isize, dy: isize },
}

/// Represents the outcome of processing an agent's action.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ActionResult {
    Success,
    Failure(String),
    Win,
}

/// Holds the state of an agent within the environment.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentState {
    pub id: EntityId,
    pub position: Position,
    pub inventory: Vec<Item>,
}

/// Provides a read-only view of the environment relevant to an agent.
#[derive(Debug)]
pub struct EnvironmentView<'a> {
    pub agent_state: &'a AgentState,
    pub location: Position,
    pub terrain_grid: &'a Grid<CellType>,
    pub item_grid: &'a Grid<Option<Item>>,
    pub agent_location_grid: &'a Grid<Option<EntityId>>,
}

/// Manages the simulation environment.
pub struct Environment {
    pub terrain: Grid<CellType>,
    pub items: Grid<Option<Item>>,
    pub agent_locations: Grid<Option<EntityId>>,
    pub agents: HashMap<EntityId, AgentState>,
    pub agent_behaviors: HashMap<EntityId, Box<dyn Agent>>,
    pub next_entity_id: EntityId,
}

impl Environment {
    /// Creates a new, empty environment.
    pub fn new(width: usize, height: usize) -> Self {
        Environment {
            terrain: Grid::new(width, height),
            items: Grid::new(width, height),
            agent_locations: Grid::new(width, height),
            agents: HashMap::new(),
            agent_behaviors: HashMap::new(),
            next_entity_id: 0,
        }
    }

    /// Generates a unique entity ID for agents.
    pub fn reserve_entity_id(&mut self) -> EntityId {
        let id = self.next_entity_id;
        self.next_entity_id += 1;
        id
    }

    /// Adds an item to the environment grid.
    pub fn add_item(&mut self, position: Position, item: Item) -> Result<(), String> {
        if !self.terrain.is_valid(position.x, position.y) {
            return Err(format!("Position {:?} is out of bounds.", position));
        }
        if self.items[position].is_some() {
            return Err(format!("Position {:?} already contains an item.", position));
        }
        if self.agent_locations[position].is_some() {
            return Err(format!("Position {:?} is occupied by an agent.", position));
        }
        match self.terrain[position] {
            CellType::Wall => {
                return Err(format!(
                    "Cannot place item inside a Wall at {:?}.",
                    position
                ));
            }
            _ => {}
        }
        self.items[position] = Some(item);
        Ok(())
    }

    /// Adds an agent to the environment.
    pub fn add_agent(
        &mut self,
        position: Position,
        behavior: Box<dyn Agent>,
        initial_inventory: Vec<Item>,
    ) -> Result<EntityId, String> {
        let agent_id = behavior.id();

        if !self.terrain.is_valid(position.x, position.y) {
            return Err(format!("Position {:?} is out of bounds.", position));
        }
        if self.agent_locations[position].is_some() {
            return Err(format!(
                "Position {:?} is already occupied by an agent.",
                position
            ));
        }
        if self.items[position].is_some() {
            eprintln!(
                "Warning: Placing agent {} on top of item at {:?}",
                agent_id, position
            );
        }
        match self.terrain[position] {
            CellType::Wall => {
                return Err(format!(
                    "Cannot place agent inside a Wall at {:?}.",
                    position
                ));
            }
            CellType::Door { open: false, .. } => {
                return Err(format!(
                    "Cannot place agent inside a closed Door at {:?}.",
                    position
                ));
            }
            _ => {}
        }

        if self.agents.contains_key(&agent_id) {
            return Err(format!("Agent ID {} is already in use.", agent_id));
        }

        let agent_state = AgentState {
            id: agent_id,
            position,
            inventory: initial_inventory,
        };

        self.agent_locations[position] = Some(agent_id);
        self.agents.insert(agent_id, agent_state.clone());
        self.agent_behaviors.insert(agent_id, behavior);

        self.next_entity_id = self.next_entity_id.max(agent_id + 1);

        Ok(agent_id)
    }

    /// Processes one turn for all agents.
    pub fn process_turn(&mut self) -> ActionResult {
        let agent_ids: Vec<EntityId> = self.agents.keys().cloned().collect();

        for agent_id in agent_ids {
            // Clone agent state to avoid borrowing issues when calling get_action & process_action
            if let Some(agent_state) = self.agents.get(&agent_id).cloned() {
                // Get mutable access to behavior
                if let Some(behavior) = self.agent_behaviors.get_mut(&agent_id) {
                    // Construct the view using the cloned state
                    let view = EnvironmentView {
                        agent_state: &agent_state, // Pass reference to cloned state
                        location: agent_state.position,
                        terrain_grid: &self.terrain,
                        item_grid: &self.items,
                        agent_location_grid: &self.agent_locations,
                    };
                    // Get action from agent
                    let action = behavior.get_action(&view);
                    let result = self.process_action(agent_id, action);
                    match result {
                        ActionResult::Success => {}
                        ActionResult::Win => {
                            return ActionResult::Win;
                        }
                        ActionResult::Failure(_reason) => {
                            // eprintln!("Agent {} action {:?} failed: {}", agent_id, action, reason);
                        }
                    }
                }
            }
        }
        ActionResult::Success
    }

    /// Processes a single action for a given agent.
    pub fn process_action(&mut self, agent_id: EntityId, action: Action) -> ActionResult {
        // Get mutable access to the agent's state
        let agent_state = match self.agents.get_mut(&agent_id) {
            Some(state) => state,
            None => return ActionResult::Failure(format!("Agent {} not found.", agent_id)),
        };

        match action {
            Action::Wait => ActionResult::Success,
            Action::Move { dx, dy } => {
                let current_pos = agent_state.position;
                // Calculate target position
                let target_x = current_pos.x.wrapping_add_signed(dx);
                let target_y = current_pos.y.wrapping_add_signed(dy);

                // Check bounds
                if !self.terrain.is_valid(target_x, target_y) {
                    return ActionResult::Failure("Target position is out of bounds.".to_string());
                }
                let target_pos = Position {
                    x: target_x,
                    y: target_y,
                };

                // Check target cell for items
                if let Some(item_pos) = self.items.get_mut(target_x, target_y) {
                    if let Some(item) = item_pos {
                        match item {
                            Item::Goal => {
                                // Goal found, goto then end game
                                self.agent_locations[current_pos] = None;
                                self.agent_locations[target_pos] = Some(agent_id);
                                agent_state.position = target_pos;
                                return ActionResult::Win;
                            }
                            Item::Chip => {
                                // Chip found, collect it and remove it from the grid
                                agent_state.inventory.push(item.clone());
                                self.items[target_pos] = None;
                            }
                            Item::Key { key_type: key } => {
                                // Key found, check if agent has the key type
                                let has_key = agent_state.inventory.iter().find(
                                    |i| matches!(i, Item::Key { key_type } if *key_type == *key),
                                );
                                // do nothing if agent has the key already
                                if has_key.is_none() {
                                    // pick up key and remove it from the grid
                                    agent_state.inventory.push(item.clone());
                                    self.items[target_pos] = None;
                                }
                            }
                        }
                    }
                }

                // Check target cell terrain and handle interactions (doors)
                match self.terrain.get(target_x, target_y).cloned() {
                    Some(CellType::Wall) => {
                        ActionResult::Failure("Cannot move into a wall.".to_string())
                    }
                    Some(CellType::Door { open: true, .. }) => {
                        // Door is already open, check only for agent occupancy
                        if self.agent_locations[target_pos].is_some() {
                            ActionResult::Failure(
                                "Target position is occupied by another agent.".to_string(),
                            )
                        } else {
                            // Move succeeds: Update agent_locations and agent's state
                            self.agent_locations[current_pos] = None;
                            self.agent_locations[target_pos] = Some(agent_id);
                            agent_state.position = target_pos; // Update the mutable agent state
                            ActionResult::Success
                        }
                    }
                    Some(CellType::Door {
                        open: false,
                        door_type: None,
                    }) => {
                        // Door is closed but needs no key (unlocked)
                        if self.agent_locations[target_pos].is_some() {
                            ActionResult::Failure(
                                "Target position is occupied by another agent.".to_string(),
                            )
                        } else {
                            // Open the door in the grid and move
                            if let Some(cell) = self.terrain.get_mut(target_x, target_y) {
                                *cell = CellType::Door {
                                    open: true,
                                    door_type: None,
                                };
                            }

                            // Update agent_locations and agent's state
                            self.agent_locations[current_pos] = None;
                            self.agent_locations[target_pos] = Some(agent_id);
                            agent_state.position = target_pos;
                            ActionResult::Success
                        }
                    }
                    Some(CellType::Door {
                        open: false,
                        door_type: Some(required_type),
                    }) => {
                        // Door is closed and requires a specific key type
                        if self.agent_locations[target_pos].is_some() {
                            ActionResult::Failure(
                                "Target position is occupied by another agent.".to_string(),
                            )
                        } else {
                            // Check if agent has the key type
                            let key_index =
                                agent_state.inventory.iter().position(|item| match item {
                                    Item::Key { key_type } => *key_type == required_type, // Compare types
                                    _ => false,
                                });

                            if let Some(index) = key_index {
                                // Agent has the key: Consume it, open door, move.
                                agent_state.inventory.remove(index); // Consume key from agent state

                                // Update door state in the terrain grid
                                if let Some(cell) = self.terrain.get_mut(target_x, target_y) {
                                    *cell = CellType::Door {
                                        open: true,
                                        door_type: Some(required_type),
                                    };
                                }

                                // Update agent position in grid and state
                                self.agent_locations[current_pos] = None;
                                self.agent_locations[target_pos] = Some(agent_id);
                                agent_state.position = target_pos;
                                ActionResult::Success
                            } else {
                                // Agent lacks the required key type
                                ActionResult::Failure(format!(
                                    "Agent lacks the required key type: {:?}.",
                                    required_type
                                ))
                            }
                        }
                    }
                    Some(CellType::Floor) => {
                        if self.agent_locations[target_pos].is_some() {
                            ActionResult::Failure(
                                "Target position is occupied by another agent.".to_string(),
                            )
                        } else {
                            // Move succeeds
                            self.agent_locations[current_pos] = None;
                            self.agent_locations[target_pos] = Some(agent_id);
                            agent_state.position = target_pos;
                            ActionResult::Success
                        }
                    }
                    None => {
                        ActionResult::Failure("Target cell not found (internal error).".to_string())
                    }
                }
            }
        }
    }

    pub fn terrain(&self) -> &Grid<CellType> {
        &self.terrain
    }
    pub fn items(&self) -> &Grid<Option<Item>> {
        &self.items
    }
    pub fn agent_locations(&self) -> &Grid<Option<EntityId>> {
        &self.agent_locations
    }
    pub fn get_agent_state(&self, agent_id: EntityId) -> Option<&AgentState> {
        self.agents.get(&agent_id)
    }

    /// Finds all positions of *closed* doors of a specific type.
    /// If `type_filter` is `None`, finds doors that require no key.
    pub fn get_door_locations(&self, type_filter: Option<DoorKeyType>) -> Vec<Position> {
        self.terrain
            .enumerate()
            .filter_map(|((x, y), cell)| match cell {
                CellType::Door {
                    open: false,
                    door_type,
                } if *door_type == type_filter => Some(Position { x, y }),
                _ => None,
            })
            .collect()
    }

    /// Finds the location of the first occurrence of a specific key *type* on the ground.
    pub fn get_key_location(&self, type_to_find: DoorKeyType) -> Option<Position> {
        self.items
            .enumerate()
            .find_map(|((x, y), item_option)| match item_option {
                Some(Item::Key { key_type }) if *key_type == type_to_find => {
                    Some(Position { x, y })
                }
                _ => None,
            })
    }

    /// Given a door's position, finds the location of the corresponding key type on the ground.
    pub fn get_corresponding_key_location(&self, door_pos: Position) -> Option<Position> {
        // 1. Check if the position contains a door that requires a key type
        if let Some(CellType::Door {
            door_type: Some(required_key_type),
            ..
        }) = self.terrain.get(door_pos.x, door_pos.y)
        {
            // 2. Search for that key type on the item grid
            self.get_key_location(*required_key_type)
        } else {
            // Not a door or doesn't require a key
            None
        }
    }
}

/// Loads an environment state from a string representation of a map.
/// Uses DoorKeyType enum for keys/doors.
pub fn load_environment_from_string(map_string: &str) -> Result<(Environment, Position), String> {
    let lines: Vec<&str> = map_string.trim().lines().collect();
    if lines.is_empty() {
        return Err("Map string is empty.".to_string());
    }

    let height = lines.len();
    let mut width = 0;
    let mut parsed_rows: Vec<Vec<&str>> = Vec::with_capacity(height);

    for (y, line) in lines.iter().enumerate() {
        let tokens: Vec<&str> = line.trim().split_whitespace().collect();
        if y == 0 {
            width = tokens.len();
            if width == 0 {
                return Err("Map has zero width.".to_string());
            }
        } else if tokens.len() != width {
            return Err(format!(
                "Inconsistent width at row {}: expected {}, found {}",
                y,
                width,
                tokens.len()
            ));
        }
        parsed_rows.push(tokens);
    }

    let mut environment = Environment::new(width, height);
    let mut start_position: Option<Position> = None;

    for (y, row_tokens) in parsed_rows.iter().enumerate() {
        for (x, token) in row_tokens.iter().enumerate() {
            let pos = Position { x, y };
            // Use DoorKeyType enum
            let (cell_type, item) = match *token {
                "ST" => {
                    if start_position.is_some() {
                        return Err("Multiple start positions ('ST') found.".to_string());
                    }
                    start_position = Some(pos);
                    (CellType::Floor, None)
                }
                "BL" => (CellType::Floor, None),
                "WL" | "WA" => (CellType::Wall, None),
                "DP" => (CellType::Floor, None), // Goal door is floor
                "PL" => (CellType::Floor, Some(Item::Goal)),
                "CH" => (CellType::Floor, Some(Item::Chip)),
                // Use DoorKeyType enum for doors
                "DG" => (
                    CellType::Door {
                        open: false,
                        door_type: Some(DoorKeyType::Green),
                    },
                    None,
                ),
                "DY" => (
                    CellType::Door {
                        open: false,
                        door_type: Some(DoorKeyType::Yellow),
                    },
                    None,
                ),
                "DB" => (
                    CellType::Door {
                        open: false,
                        door_type: Some(DoorKeyType::Blue),
                    },
                    None,
                ),
                "DR" => (
                    CellType::Door {
                        open: false,
                        door_type: Some(DoorKeyType::Red),
                    },
                    None,
                ),
                // Use DoorKeyType enum for keys
                "KG" => (
                    CellType::Floor,
                    Some(Item::Key {
                        key_type: DoorKeyType::Green,
                    }),
                ),
                "KY" => (
                    CellType::Floor,
                    Some(Item::Key {
                        key_type: DoorKeyType::Yellow,
                    }),
                ),
                "KB" => (
                    CellType::Floor,
                    Some(Item::Key {
                        key_type: DoorKeyType::Blue,
                    }),
                ),
                "KR" => (
                    CellType::Floor,
                    Some(Item::Key {
                        key_type: DoorKeyType::Red,
                    }),
                ),
                unknown => {
                    return Err(format!(
                        "Unknown map code '{}' at position ({}, {}).",
                        unknown, x, y
                    ));
                }
            };

            environment.terrain[pos] = cell_type;
            if let Some(it) = item {
                environment.items[pos] = Some(it);
            }
        }
    }

    let start_pos =
        start_position.ok_or_else(|| "No start position ('ST') found in map.".to_string())?;

    Ok((environment, start_pos))
}
