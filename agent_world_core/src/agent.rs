use std::{
    cmp::Ordering,
    collections::{BinaryHeap, HashMap, HashSet, VecDeque},
};

use rand::{Rng, SeedableRng, rngs::StdRng};

use crate::{
    DoorKeyType, EntityId, Item, Position,
    environment::{Action, CellType, EnvironmentView},
};

/// Trait defining the behavior of an agent.
/// Agents decide which action to take based on the EnvironmentView.
pub trait Agent {
    /// Returns the unique ID of this agent.
    fn id(&self) -> EntityId;

    /// Determines the action the agent wants to perform based on its view of the environment.
    /// `&mut self` allows the agent to maintain internal state for decision making (e.g., pathfinding).
    fn get_action(&mut self, view: &EnvironmentView) -> Action;
}

/// A simple agent that tries to move randomly.
#[derive(Debug)]
pub struct RandomWalker {
    id: EntityId,
    rng: StdRng,
}

impl RandomWalker {
    pub fn new(id: EntityId, seed: u64) -> Self {
        Self {
            id,
            rng: StdRng::seed_from_u64(seed),
        }
    }
}

impl Agent for RandomWalker {
    fn id(&self) -> EntityId {
        self.id
    }

    fn get_action(&mut self, _view: &EnvironmentView) -> Action {
        // Random movement
        let dx: i8 = self.rng.random_range(-1..=1);
        let dy: i8 = self.rng.random_range(-1..=1);

        if dx == 0 && dy == 0 {
            Action::Wait
        } else {
            Action::Move {
                dx: dx as isize,
                dy: dy as isize,
            }
        }
    }
}

/// A planning agent that tries to move towards the goal after collecting all chips.
#[derive(Debug)]
pub struct PlanningAgent {
    id: EntityId,
    current_plan: VecDeque<Position>, // Queue of positions to visit
}

impl PlanningAgent {
    pub fn new(id: EntityId) -> Self {
        Self {
            id,
            current_plan: VecDeque::new(),
        }
    }

    /// Returns manhattan distance between two positions
    fn manhattan_distance(a: &Position, b: &Position) -> usize {
        let dx = if a.x > b.x { a.x - b.x } else { b.x - a.x };
        let dy = if a.y > b.y { a.y - b.y } else { b.y - a.y };
        dx + dy
    }

    /// Converts a move between two adjacent positions into an Action
    fn position_to_action(src: &Position, dst: &Position) -> Action {
        let dx = dst.x as isize - src.x as isize;
        let dy = dst.y as isize - src.y as isize;

        match (dx, dy) {
            (0, 0) => Action::Wait,
            (0, 1) => Action::Move { dx: 0, dy: 1 },
            (0, -1) => Action::Move { dx: 0, dy: -1 },
            (1, 0) => Action::Move { dx: 1, dy: 0 },
            (-1, 0) => Action::Move { dx: -1, dy: 0 },
            _ => {
                // This shouldn't happen if positions are adjacent
                eprintln!("Invalid move from {:?} to {:?}", src, dst);
                Action::Wait
            }
        }
    }

    /// A* pathfinding implementation
    fn a_star_path(
        &self,
        start: Position,
        goal: Position,
        view: &EnvironmentView,
        keys_held: &HashSet<DoorKeyType>,
    ) -> Option<Vec<Position>> {
        // For priority queue
        #[derive(Clone, Eq, PartialEq)]
        struct PrioritizedItem {
            priority: usize,
            position: Position,
        }

        impl Ord for PrioritizedItem {
            fn cmp(&self, other: &Self) -> Ordering {
                // Reverse ordering for min-heap behavior
                other.priority.cmp(&self.priority)
            }
        }

        impl PartialOrd for PrioritizedItem {
            fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
                Some(self.cmp(other))
            }
        }

        let mut frontier = BinaryHeap::new();
        let mut came_from: HashMap<Position, Position> = HashMap::new();
        let mut cost_so_far: HashMap<Position, usize> = HashMap::new();

        frontier.push(PrioritizedItem {
            priority: 0,
            position: start,
        });
        cost_so_far.insert(start, 0);

        let mut goal_reached = false;

        while let Some(PrioritizedItem {
            position: current, ..
        }) = frontier.pop()
        {
            if current == goal {
                goal_reached = true;
                break;
            }

            // Get valid neighbors
            let valid_neighbors = self.get_valid_neighbors(&current, view, keys_held);

            for neighbor in valid_neighbors {
                let new_cost = cost_so_far.get(&current).unwrap_or(&usize::MAX) + 1;

                if !cost_so_far.contains_key(&neighbor)
                    || new_cost < *cost_so_far.get(&neighbor).unwrap()
                {
                    cost_so_far.insert(neighbor, new_cost);
                    let priority = new_cost + Self::manhattan_distance(&neighbor, &goal);
                    frontier.push(PrioritizedItem {
                        priority,
                        position: neighbor,
                    });
                    came_from.insert(neighbor, current);
                }
            }
        }

        if !goal_reached {
            return None;
        }

        // Reconstruct path
        let mut path = Vec::new();
        let mut current = goal;
        path.push(current);

        while current != start {
            current = *came_from.get(&current)?;
            path.push(current);
        }

        path.reverse();
        Some(path)
    }

    /// Gets valid neighbors for a position based on the environment and keys held
    fn get_valid_neighbors(
        &self,
        position: &Position,
        view: &EnvironmentView,
        keys_held: &HashSet<DoorKeyType>,
    ) -> Vec<Position> {
        let mut neighbors = Vec::new();
        let terrain = view.terrain_grid;
        let agents = view.agent_location_grid;

        // Check all four directions
        let directions = [
            (0, 1),  // Down
            (0, -1), // Up
            (1, 0),  // Right
            (-1, 0), // Left
        ];

        for (dx, dy) in directions.iter() {
            // Calculate neighbor position, handling potential overflow
            let nx = match position.x.checked_add_signed(*dx) {
                Some(x) => x,
                None => continue, // Skip invalid positions
            };

            let ny = match position.y.checked_add_signed(*dy) {
                Some(y) => y,
                None => continue, // Skip invalid positions
            };

            // Check if position is valid in the grid
            if !terrain.is_valid(nx, ny) {
                continue;
            }

            let neighbor_pos = Position { x: nx, y: ny };

            // Check if position is occupied by another agent
            if let Some(Some(_)) = agents.get(nx, ny) {
                continue;
            }

            // Check terrain type
            match terrain.get(nx, ny) {
                Some(CellType::Wall) => continue,
                Some(CellType::Door {
                    open: false,
                    door_type: Some(required_key),
                }) => {
                    // Check if we have the key for this door
                    if !keys_held.contains(required_key) {
                        continue;
                    }
                }
                Some(CellType::Door {
                    open: false,
                    door_type: None,
                }) => {
                    // No key required, can be opened
                }
                Some(CellType::Door { open: true, .. }) | Some(CellType::Floor) => {
                    // These are always valid
                }
                None => continue, // Should never happen with valid position
            }

            neighbors.push(neighbor_pos);
        }

        neighbors
    }

    /// Extracts the keys currently held by the agent
    fn get_keys_held(&self, view: &EnvironmentView) -> HashSet<DoorKeyType> {
        let mut keys = HashSet::new();

        for item in &view.agent_state.inventory {
            if let Item::Key { key_type } = item {
                keys.insert(*key_type);
            }
        }

        keys
    }

    /// Finds all positions with chips in the environment
    fn find_chips(&self, view: &EnvironmentView) -> Vec<Position> {
        let mut chip_positions = Vec::new();

        for ((x, y), item_opt) in view.item_grid.enumerate() {
            if let Some(Item::Chip) = item_opt {
                chip_positions.push(Position { x, y });
            }
        }

        chip_positions
    }

    /// Finds the goal position in the environment
    fn find_goals(&self, view: &EnvironmentView) -> Vec<Position> {
        let mut goal_positions = Vec::new();

        for ((x, y), item_opt) in view.item_grid.enumerate() {
            if let Some(Item::Goal) = item_opt {
                goal_positions.push(Position { x, y });
            }
        }

        goal_positions
    }

    /// Finds keys of a given type in the environment
    fn find_keys(&self, view: &EnvironmentView) -> HashMap<DoorKeyType, Vec<Position>> {
        let mut key_positions = HashMap::new();

        for ((x, y), item_opt) in view.item_grid.enumerate() {
            if let Some(Item::Key { key_type }) = item_opt {
                key_positions
                    .entry(*key_type)
                    .or_insert_with(Vec::new)
                    .push(Position { x, y });
            }
        }

        key_positions
    }

    /// Plans to the nearest target from a list of positions
    fn plan_to_nearest_target(
        &self,
        start: Position,
        targets: &[Position],
        view: &EnvironmentView,
        keys_held: &HashSet<DoorKeyType>,
    ) -> Option<Vec<Position>> {
        if targets.is_empty() {
            return None;
        }

        let mut best_plan = None;
        let mut min_length = usize::MAX;

        for target in targets {
            if let Some(plan) = self.a_star_path(start, *target, view, keys_held) {
                if plan.len() < min_length {
                    min_length = plan.len();
                    best_plan = Some(plan);
                }
            }
        }

        best_plan
    }

    /// Plan to the nearest reachable key that we don't currently have
    fn plan_to_nearest_reachable_key(
        &self,
        start: Position,
        view: &EnvironmentView,
        keys_held: &HashSet<DoorKeyType>,
    ) -> Option<Vec<Position>> {
        let key_locations = self.find_keys(view);

        let mut best_plan = None;
        let mut min_length = usize::MAX;

        for (key_type, positions) in key_locations {
            // Skip keys we already have
            if keys_held.contains(&key_type) {
                continue;
            }

            for key_pos in &positions {
                if let Some(plan) = self.a_star_path(start, *key_pos, view, keys_held) {
                    if plan.len() < min_length {
                        min_length = plan.len();
                        best_plan = Some(plan);
                    }
                }
            }
        }

        best_plan
    }
}

impl Agent for PlanningAgent {
    fn id(&self) -> EntityId {
        self.id
    }

    fn get_action(&mut self, view: &EnvironmentView) -> Action {
        let current_pos = view.location;
        let keys_held = self.get_keys_held(view);

        // 1. Follow existing plan if available
        if let Some(next_pos) = self.current_plan.pop_front() {
            return Self::position_to_action(&current_pos, &next_pos);
        }

        // 2. Determine primary targets (chips or goal)
        let chips = self.find_chips(view);

        if !chips.is_empty() {
            // Try to plan to the nearest chip
            if let Some(plan) = self.plan_to_nearest_target(current_pos, &chips, view, &keys_held) {
                if plan.len() > 1 {
                    // Skip the first position (current position)
                    self.current_plan.extend(plan.into_iter().skip(1));
                    return if let Some(next_pos) = self.current_plan.pop_front() {
                        Self::position_to_action(&current_pos, &next_pos)
                    } else {
                        Action::Wait
                    };
                }
            }
        } else {
            // No chips left, try to plan to the goal
            let goals = self.find_goals(view);
            if let Some(plan) = self.plan_to_nearest_target(current_pos, &goals, view, &keys_held) {
                if plan.len() > 1 {
                    // Skip the first position (current position)
                    self.current_plan.extend(plan.into_iter().skip(1));
                    return if let Some(next_pos) = self.current_plan.pop_front() {
                        Self::position_to_action(&current_pos, &next_pos)
                    } else {
                        Action::Wait
                    };
                }
            }
        }

        // 3. If primary targets unreachable, try to get a key
        if let Some(key_plan) = self.plan_to_nearest_reachable_key(current_pos, view, &keys_held) {
            if key_plan.len() > 1 {
                // Skip the first position (current position)
                self.current_plan.extend(key_plan.into_iter().skip(1));
                return if let Some(next_pos) = self.current_plan.pop_front() {
                    Self::position_to_action(&current_pos, &next_pos)
                } else {
                    Action::Wait
                };
            }
        }

        // 4. No valid plan, Do nothing
        Action::Wait
    }
}
