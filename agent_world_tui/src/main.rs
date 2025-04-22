use agent_world_core::{
    DoorKeyType, EntityId, Item,
    agent::PlanningAgent,
    environment::{ActionResult, AgentState, CellType, Environment, load_environment_from_string},
};
use anyhow::Result;
use clap::Parser;
use ratatui::{
    crossterm::{
        self,
        event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode},
        execute,
        terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
    },
    prelude::*,
    widgets::*,
};
use std::{
    collections::HashMap,
    io::{self, Stdout},
    path::PathBuf,
    time::{Duration, Instant},
};

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// Map file to load
    #[arg(short, long, value_name = "MAP_FILE")]
    map: Option<PathBuf>,
}

struct App {
    /// The core simulation environment.
    environment: Environment,
    /// Flag to control the main loop.
    should_quit: bool,
    /// Flag to control if the game is over.
    game_over: bool,
}

impl App {
    fn new(map_file: PathBuf) -> Self {
        // Get map from file
        let file_string = std::fs::read_to_string(map_file).expect("Failed to read map file");
        let (mut environment, start_position) =
            load_environment_from_string(&file_string).expect("Failed to load environment");

        let agent = PlanningAgent::new(environment.reserve_entity_id());
        environment
            .add_agent(start_position, Box::new(agent), vec![])
            .expect("Adding agent");

        App {
            environment,
            should_quit: false,
            game_over: false,
        }
    }

    /// Handles one step of the simulation.
    fn tick(&mut self) {
        if self.game_over {
            return;
        }
        let result = self.environment.process_turn();
        match result {
            ActionResult::Win => {
                self.game_over = true;
            }
            _ => {}
        }
    }

    /// Sets the quit flag.
    fn quit(&mut self) {
        self.should_quit = true;
    }
}

fn main() -> Result<()> {
    // Parse command line arguments
    let args = Args::parse();
    // If no map file is provided, use the default map
    let map_file = args.map.unwrap_or(PathBuf::from("maps/map01.txt"));
    // Ensure the map file exists
    if !map_file.exists() {
        return Err(anyhow::anyhow!(
            "Map file does not exist: {}",
            map_file.display()
        ));
    }

    // Set up the terminal
    let mut terminal = setup_terminal()?;

    // Create the application state
    let mut app = App::new(map_file);

    // Run the main application loop
    run_app(&mut terminal, &mut app)?;

    // Restore the terminal state
    restore_terminal(&mut terminal)?;

    Ok(())
}

/// Configures the terminal for TUI interaction.
fn setup_terminal() -> Result<Terminal<CrosstermBackend<Stdout>>> {
    let mut stdout = io::stdout();
    enable_raw_mode()?; // Put terminal in raw mode
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?; // Use alternate screen and enable mouse capture
    let backend = CrosstermBackend::new(stdout);
    Terminal::new(backend).map_err(Into::into) // Map io::Error to anyhow::Error
}

/// Restores the terminal to its original state.
fn restore_terminal(terminal: &mut Terminal<CrosstermBackend<Stdout>>) -> Result<()> {
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;
    Ok(())
}

/// Runs the main loop of the TUI application.
fn run_app(terminal: &mut Terminal<CrosstermBackend<Stdout>>, app: &mut App) -> Result<()> {
    let tick_rate = Duration::from_millis(250); // Update rate
    let mut last_tick = Instant::now();

    loop {
        // Draw the UI
        terminal.draw(|f| ui(f, app))?;

        // Calculate timeout for event polling
        let timeout = tick_rate
            .checked_sub(last_tick.elapsed())
            .unwrap_or_else(|| Duration::from_secs(0));

        // Poll for events (keyboard, mouse, etc.)
        if crossterm::event::poll(timeout)? {
            if let Event::Key(key) = event::read()? {
                match key.code {
                    KeyCode::Char('q') | KeyCode::Esc => app.quit(),
                    _ => {}
                }
            }
        }

        // Update application state if enough time has passed
        if last_tick.elapsed() >= tick_rate {
            app.tick(); // Perform simulation step
            last_tick = Instant::now();
        }

        // Exit loop if requested
        if app.should_quit {
            break;
        }
    }
    Ok(())
}

/// Renders the user interface.
fn ui(frame: &mut Frame, app: &App) {
    let main_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage(70), // Area for the map
            Constraint::Percentage(20), // Area for inventory
            Constraint::Percentage(10), // Area for status/help
        ])
        .split(frame.area());

    // Render the map
    render_map(frame, main_layout[0], &app.environment);

    // Render the inventory
    render_inventory(frame, main_layout[1], &app.environment.agents);

    // Render status/help text
    let help_text = Paragraph::new("Press 'q' or 'Esc' to quit.")
        .alignment(Alignment::Center)
        .block(Block::default().borders(Borders::TOP));
    frame.render_widget(help_text, main_layout[2]);
}

/// Renders the inventory of each agent onto the frame.
fn render_inventory(frame: &mut Frame, area: Rect, agents: &HashMap<EntityId, AgentState>) {
    let inventory_items: Vec<ListItem> = agents
        .iter()
        .map(|(id, agent)| {
            // Amount of chips collected
            let chip_amount = agent
                .inventory
                .iter()
                .filter(|item| matches!(item, Item::Chip))
                .count();
            // Colored keys collected
            let collected_keys: Vec<Span> = agent
                .inventory
                .iter()
                .filter_map(|item| match item {
                    Item::Key { key_type } => match key_type {
                        DoorKeyType::Red => {
                            Some(Span::styled("k", Style::default().fg(Color::Red)))
                        }
                        DoorKeyType::Blue => {
                            Some(Span::styled("k", Style::default().fg(Color::Blue)))
                        }
                        DoorKeyType::Yellow => {
                            Some(Span::styled("k", Style::default().fg(Color::Yellow)))
                        }
                        DoorKeyType::Green => {
                            Some(Span::styled("k", Style::default().fg(Color::Green)))
                        }
                    },
                    _ => None,
                })
                .collect();
            let agent_pos = agent.position;
            let mut agent_info_text = vec![Span::styled(
                format!(
                    "Agent: {:?} Pos: ({}, {}) Chips collected: {} Keys collected: ",
                    id, agent_pos.x, agent_pos.y, chip_amount
                ),
                Style::default(),
            )];
            agent_info_text.extend(collected_keys);
            ListItem::from(Line::from(agent_info_text))
        })
        .collect();

    let inventory_widget =
        List::new(inventory_items).block(Block::default().borders(Borders::ALL).title("Inventory"));
    frame.render_widget(inventory_widget, area);
}

/// Renders the environment map onto the frame.
fn render_map(frame: &mut Frame, area: Rect, environment: &Environment) {
    let map = &environment.terrain;
    let agents = &environment.agents;
    let items = &environment.items;

    // Create a representation of the map grid with agents
    let mut lines: Vec<Line> = Vec::with_capacity(map.height());

    for y in 0..map.height() {
        let mut spans: Vec<Span> = Vec::with_capacity(map.width());
        for x in 0..map.width() {
            // Check if an agent is at this position
            let agent_char = agents
                .values()
                .find(|a| a.position.x == x && a.position.y == y)
                .map(|_| {
                    // Display agent character '@' with color
                    Span::styled("@", Style::default().fg(Color::Red).bold())
                });
            // Check if an item is at this position
            let item_char = if let Some(pos) = items.get(x, y) {
                match pos {
                    Some(item) => match item {
                        Item::Chip => Some(Span::styled("c", Style::default().fg(Color::Yellow))),
                        Item::Goal => Some(Span::styled("g", Style::default().fg(Color::Green))),
                        Item::Key { key_type } => match key_type {
                            DoorKeyType::Red => {
                                Some(Span::styled("k", Style::default().fg(Color::Red)))
                            }
                            DoorKeyType::Blue => {
                                Some(Span::styled("k", Style::default().fg(Color::Blue)))
                            }
                            DoorKeyType::Yellow => {
                                Some(Span::styled("k", Style::default().fg(Color::Yellow)))
                            }
                            DoorKeyType::Green => {
                                Some(Span::styled("k", Style::default().fg(Color::Green)))
                            }
                        },
                    },
                    None => None,
                }
            } else {
                None
            };

            if let Some(item_span) = item_char {
                spans.push(item_span);
            } else if let Some(agent_span) = agent_char {
                spans.push(agent_span);
            } else {
                // Display map tile character
                let tile = map.get(x, y).unwrap_or(&CellType::Floor); // Handle potential out-of-bounds safely
                let tile_char = match tile {
                    CellType::Floor => " ",
                    CellType::Wall => "#",
                    CellType::Door { open, .. } => {
                        if *open {
                            "+"
                        } else {
                            "|"
                        }
                    }
                };
                let tile_style = match tile {
                    CellType::Wall => Style::default().fg(Color::DarkGray),
                    CellType::Door { door_type, .. } => {
                        if let Some(door_style) = door_type {
                            match door_style {
                                DoorKeyType::Red => Style::default().fg(Color::Red),
                                DoorKeyType::Blue => Style::default().fg(Color::Blue),
                                DoorKeyType::Green => Style::default().fg(Color::Green),
                                DoorKeyType::Yellow => Style::default().fg(Color::Yellow),
                            }
                        } else {
                            Style::default()
                        }
                    }
                    _ => Style::default(),
                };
                spans.push(Span::styled(tile_char, tile_style));
            }
        }
        lines.push(Line::from(spans));
    }

    let map_paragraph = Paragraph::new(lines)
        .block(Block::default().title("Agent World").borders(Borders::ALL))
        .alignment(Alignment::Center);

    frame.render_widget(map_paragraph, area);
}
