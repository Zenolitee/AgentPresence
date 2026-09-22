//! Terminal UI for AgentPresence.
//!
//! Renders live session state on the left and a toggleable settings panel on
//! the right. Every toggle writes through to the shared [`AppState`] so the
//! background presence loop picks changes up on its next poll, and persists
//! the config to disk via [`save_config`].
//!
//! Keys: `↑`/`↓` (`k`/`j`) move, `Enter`/`Space` toggles + saves,
//! `s` saves without changes, `q`/`Esc` quits.

use std::io;
use std::sync::Arc;
use std::time::Duration;

use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use parking_lot::Mutex;
use ratatui::backend::Backend;
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, ListState, Paragraph};
use ratatui::Frame;
use ratatui::Terminal;

use crate::config::{save_config, Config, DisplaySettings};
use crate::harnesses::{AgentKind, AgentSession};

/// Shared state between the presence loop (background thread) and the TUI
/// (main thread).
pub struct AppState {
    pub config: Config,
    pub current_session: Option<AgentSession>,
    pub discord_connected: bool,
    pub running: bool,
}

/// One toggleable settings row.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Field {
    DetectProcesses,
    DetectCodex,
    DetectClaude,
    DetectPi,
    DetectOmp,
    DetectOpencode,
    DetectHermes,
    PriorityPresence,
    FlavorText,
    ShowProject,
    ShowModel,
    ShowBranch,
    ShowActivity,
    ShowSurface,
    ShowPlan,
    ShowTokens,
    ShowCost,
    ShowContext,
    ShowLimits,
}

/// A rendered settings row: either a group header or a toggle.
#[derive(Clone, Copy)]
enum Row {
    Header(&'static str),
    Toggle(Field, &'static str),
}

const DETECT_ROWS: [(Field, &str); 7] = [
    (Field::DetectProcesses, "Process fallback detection"),
    (Field::DetectCodex, "Detect Codex"),
    (Field::DetectClaude, "Detect Claude Code"),
    (Field::DetectPi, "Detect Pi"),
    (Field::DetectOmp, "Detect Oh My Pi"),
    (Field::DetectOpencode, "Detect OpenCode"),
    (Field::DetectHermes, "Detect Hermes"),
];

const PRESENCE_ROWS: [(Field, &str); 2] = [
    (Field::PriorityPresence, "Priority presence (republish)"),
    (Field::FlavorText, "Flavor text"),
];

const DISPLAY_ROWS: [(Field, &str); 11] = [
    (Field::ShowProject, "Show project"),
    (Field::ShowModel, "Show model"),
    (Field::ShowBranch, "Show branch"),
    (Field::ShowActivity, "Show activity"),
    (Field::ShowSurface, "Show surface"),
    (Field::ShowPlan, "Show plan"),
    (Field::ShowTokens, "Show tokens"),
    (Field::ShowCost, "Show cost"),
    (Field::ShowContext, "Show context"),
    (Field::ShowLimits, "Show limits"),
    (Field::FlavorText, "Flavor text"),
];

fn build_rows() -> Vec<Row> {
    let mut rows = vec![Row::Header("Detection")];
    rows.extend(DETECT_ROWS.map(|(field, label)| Row::Toggle(field, label)));
    rows.push(Row::Header("Presence"));
    rows.extend(PRESENCE_ROWS.map(|(field, label)| Row::Toggle(field, label)));
    rows.push(Row::Header("Display (active agent)"));
    rows.extend(DISPLAY_ROWS.map(|(field, label)| Row::Toggle(field, label)));
    rows
}

fn display_mut(config: &mut Config, agent: AgentKind) -> &mut DisplaySettings {
    match agent {
        AgentKind::Codex => &mut config.codex_display,
        AgentKind::Claude => &mut config.claude_display,
        AgentKind::OpenCode => &mut config.opencode_display,
        AgentKind::Pi => &mut config.pi_display,
        AgentKind::OhMyPi => &mut config.omp_display,
        AgentKind::Hermes => &mut config.hermes_display,
    }
}

fn display_ref(config: &Config, agent: AgentKind) -> &DisplaySettings {
    match agent {
        AgentKind::Codex => &config.codex_display,
        AgentKind::Claude => &config.claude_display,
        AgentKind::OpenCode => &config.opencode_display,
        AgentKind::Pi => &config.pi_display,
        AgentKind::OhMyPi => &config.omp_display,
        AgentKind::Hermes => &config.hermes_display,
    }
}

impl Field {
    fn get(self, config: &Config, agent: AgentKind) -> bool {
        match self {
            Field::DetectProcesses => config.detect_processes,
            Field::DetectCodex => config.detect_codex,
            Field::DetectClaude => config.detect_claude,
            Field::DetectPi => config.detect_pi,
            Field::DetectOmp => config.detect_omp,
            Field::DetectOpencode => config.detect_opencode,
            Field::DetectHermes => config.detect_hermes,
            Field::PriorityPresence => config.priority_presence,
            Field::FlavorText => display_ref(config, agent).flavor_text,
            Field::ShowProject => display_ref(config, agent).show_project,
            Field::ShowModel => display_ref(config, agent).show_model,
            Field::ShowBranch => display_ref(config, agent).show_branch,
            Field::ShowActivity => display_ref(config, agent).show_activity,
            Field::ShowSurface => display_ref(config, agent).show_surface,
            Field::ShowPlan => display_ref(config, agent).show_plan,
            Field::ShowTokens => display_ref(config, agent).show_tokens,
            Field::ShowCost => display_ref(config, agent).show_cost,
            Field::ShowContext => display_ref(config, agent).show_context,
            Field::ShowLimits => display_ref(config, agent).show_limits,
        }
    }

    fn set(self, config: &mut Config, agent: AgentKind, value: bool) {
        match self {
            Field::DetectProcesses => config.detect_processes = value,
            Field::DetectCodex => config.detect_codex = value,
            Field::DetectClaude => config.detect_claude = value,
            Field::DetectPi => config.detect_pi = value,
            Field::DetectOmp => config.detect_omp = value,
            Field::DetectOpencode => config.detect_opencode = value,
            Field::DetectHermes => config.detect_hermes = value,
            Field::PriorityPresence => config.priority_presence = value,
            Field::FlavorText => display_mut(config, agent).flavor_text = value,
            Field::ShowProject => display_mut(config, agent).show_project = value,
            Field::ShowModel => display_mut(config, agent).show_model = value,
            Field::ShowBranch => display_mut(config, agent).show_branch = value,
            Field::ShowActivity => display_mut(config, agent).show_activity = value,
            Field::ShowSurface => display_mut(config, agent).show_surface = value,
            Field::ShowPlan => display_mut(config, agent).show_plan = value,
            Field::ShowTokens => display_mut(config, agent).show_tokens = value,
            Field::ShowCost => display_mut(config, agent).show_cost = value,
            Field::ShowContext => display_mut(config, agent).show_context = value,
            Field::ShowLimits => display_mut(config, agent).show_limits = value,
        }
    }
}

/// Point-in-time copy of the shared state, taken once per frame.
struct Snapshot {
    config: Config,
    session: Option<AgentSession>,
    discord_connected: bool,
}

fn snapshot(state: &Arc<Mutex<AppState>>) -> (Snapshot, bool) {
    let guard = state.lock();
    (
        Snapshot {
            config: guard.config.clone(),
            session: guard.current_session.clone(),
            discord_connected: guard.discord_connected,
        },
        guard.running,
    )
}

/// Entry point: takes the terminal over, runs the UI loop, restores the
/// terminal afterwards even if the loop errors.
pub fn run_tui(config: Config, state: Arc<Mutex<AppState>>) -> io::Result<()> {
    {
        let mut guard = state.lock();
        guard.config = config;
    }

    enable_raw_mode()?;
    execute!(io::stdout(), EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(io::stdout());
    let mut terminal = Terminal::new(backend)?;

    let result = ui_loop(&mut terminal, &state);

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;
    result
}

fn ui_loop<B: Backend>(terminal: &mut Terminal<B>, state: &Arc<Mutex<AppState>>) -> io::Result<()> {
    let rows = build_rows();
    let mut list_state = ListState::default();
    list_state.select(Some(next_toggle(&rows, None, 1)));
    let mut status: Option<String> = None;

    loop {
        let (snap, running) = snapshot(state);
        if !running {
            break;
        }

        let agent = snap
            .session
            .as_ref()
            .map(|session| session.agent)
            .unwrap_or(AgentKind::Codex);

        terminal.draw(|frame| {
            render(
                frame,
                &snap,
                agent,
                &rows,
                &mut list_state,
                status.as_deref(),
            )
        })?;

        if event::poll(Duration::from_millis(200))? {
            let Event::Key(key) = event::read()? else {
                continue;
            };
            if key.kind != KeyEventKind::Press {
                continue;
            }

            match key.code {
                KeyCode::Char('q') | KeyCode::Esc => {
                    state.lock().running = false;
                    break;
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    let index = next_toggle(&rows, list_state.selected(), 1);
                    list_state.select(Some(index));
                    status = None;
                }
                KeyCode::Up | KeyCode::Char('k') => {
                    let index = next_toggle(&rows, list_state.selected(), -1);
                    list_state.select(Some(index));
                    status = None;
                }
                KeyCode::Enter | KeyCode::Char(' ') => {
                    if let Some(Row::Toggle(field, label)) =
                        list_state.selected().and_then(|index| rows.get(index))
                    {
                        let mut guard = state.lock();
                        let current = field.get(&guard.config, agent);
                        field.set(&mut guard.config, agent, !current);
                        let value = field.get(&guard.config, agent);
                        status = Some(match save_config(&guard.config) {
                            Ok(()) => format!("{label}: {}", if value { "on" } else { "off" }),
                            Err(error) => format!("save failed: {error}"),
                        });
                    }
                }
                KeyCode::Char('s') => {
                    let guard = state.lock();
                    status = Some(match save_config(&guard.config) {
                        Ok(()) => "config saved".to_string(),
                        Err(error) => format!("save failed: {error}"),
                    });
                }
                _ => {}
            }
        }
    }

    Ok(())
}

/// Index of the next toggle row after `current` moving `direction` (+1/-1),
/// wrapping at the edges and skipping group headers.
fn next_toggle(rows: &[Row], current: Option<usize>, direction: i32) -> usize {
    let toggles: Vec<usize> = rows
        .iter()
        .enumerate()
        .filter(|(_, row)| matches!(row, Row::Toggle(..)))
        .map(|(index, _)| index)
        .collect();
    if toggles.is_empty() {
        return 0;
    }

    let position = current
        .and_then(|index| toggles.iter().position(|toggle| *toggle == index))
        .map(|position| position as i32)
        .unwrap_or(if direction > 0 { -1 } else { 0 });

    let count = toggles.len() as i32;
    let next = (position + direction).rem_euclid(count);
    toggles[next as usize]
}

fn value_line(label: &str, value: Option<&String>) -> Line<'static> {
    let text = match value {
        Some(value) if !value.is_empty() => format!("{label}: {value}"),
        _ => format!("{label}: —"),
    };
    Line::from(text)
}

fn render(
    frame: &mut Frame,
    snap: &Snapshot,
    agent: AgentKind,
    rows: &[Row],
    list_state: &mut ListState,
    status: Option<&str>,
) {
    let areas = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(8),
            Constraint::Length(3),
        ])
        .split(frame.area());

    // Header: app name, active agent, Discord link state.
    let discord_span = if snap.discord_connected {
        Span::styled("● live", Style::default().fg(Color::Green))
    } else {
        Span::styled("○ idle", Style::default().fg(Color::DarkGray))
    };
    let header = Paragraph::new(Line::from(vec![
        Span::styled(
            " AgentPresence ",
            Style::default()
                .fg(Color::Magenta)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw("│ "),
        Span::styled(agent.display_name(), Style::default().fg(Color::Cyan)),
        Span::raw(" │ discord "),
        discord_span,
    ]))
    .block(Block::default().borders(Borders::ALL));
    frame.render_widget(header, areas[0]);

    // Left panel: current session details.
    let session_lines = match &snap.session {
        Some(session) => vec![
            value_line("agent", Some(&session.agent.display_name().to_string())),
            Line::from(format!("active: {}", session.active)),
            value_line("project", session.project.as_ref()),
            value_line("branch", session.branch.as_ref()),
            value_line("model", session.model.as_ref()),
            value_line("surface", session.surface.as_ref()),
            value_line("activity", session.activity.as_ref()),
            value_line("plan", session.plan.as_ref()),
            value_line("tokens", session.tokens.as_ref()),
            value_line("cost", session.cost.as_ref()),
            value_line("context", session.context.as_ref()),
            value_line("limits", session.limits.as_ref()),
            value_line(
                "source",
                session
                    .path
                    .as_ref()
                    .map(|path| path.display().to_string())
                    .as_ref(),
            ),
        ],
        None => vec![
            Line::from("no active session"),
            Line::from(""),
            Line::from("waiting for an agent to write"),
            Line::from("to its session files…"),
        ],
    };
    let session_panel = Paragraph::new(session_lines)
        .block(Block::default().borders(Borders::ALL).title(" Session "));
    frame.render_widget(session_panel, areas[1]);

    // Right panel: toggleable settings.
    let agent_label = agent.display_name();
    let items: Vec<ListItem> = rows
        .iter()
        .map(|row| match row {
            Row::Header(label) => ListItem::new(Line::styled(
                format!(" {label}"),
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            )),
            Row::Toggle(field, label) => {
                let value = field.get(&snap.config, agent);
                let mark = if value { "[x]" } else { "[ ]" };
                let color = if value { Color::Green } else { Color::DarkGray };
                ListItem::new(Line::from(vec![
                    Span::raw("  "),
                    Span::styled(mark, Style::default().fg(color)),
                    Span::raw(format!(" {label}")),
                ]))
            }
        })
        .collect();

    let settings_title = format!(" Settings — display applies to {agent_label} ");
    let settings = List::new(items)
        .block(Block::default().borders(Borders::ALL).title(settings_title))
        .highlight_symbol("▶ ")
        .highlight_style(
            Style::default()
                .fg(Color::Black)
                .bg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        );
    frame.render_stateful_widget(settings, areas[1], list_state);

    // Footer: keys + last status message.
    let footer_text = match status {
        Some(status) => format!(" ↑↓ move  enter toggle  s save  q quit   │ {status}"),
        None => " ↑↓ move  enter/space toggle + save  s save  q quit".to_string(),
    };
    let footer = Paragraph::new(Line::from(footer_text))
        .style(Style::default().fg(Color::DarkGray))
        .block(Block::default().borders(Borders::ALL));
    frame.render_widget(footer, areas[2]);
}
