use crate::agent::build_autonomous_coding_graph;
use crate::git;
use crate::graph::state::AgentState;
use crate::llm::HeuristicEngine;
use anyhow::Result;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::{Backend, CrosstermBackend},
    layout::{Alignment, Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Gauge, List, ListItem, Paragraph, Tabs, Wrap},
    Frame, Terminal,
};
use std::io;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum RightTab {
    Logs,
    DiffAndPr,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum AgentStatus {
    Idle,
    Running,
    Success,
    Failed,
}

pub struct AppState {
    pub status: AgentStatus,
    pub status_text: String,
    pub current_node: Option<String>,
    pub completed_nodes: Vec<String>,
    pub failed_nodes: Vec<String>,
    pub logs: Vec<String>,
    pub active_tab: RightTab,
    pub progress_percent: u16,
    pub iteration: usize,
    pub max_retries: usize,
    pub task: String,
    pub repo_path: String,
    pub git_branch: String,
    pub diff_preview: String,
    pub pr_preview: String,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            status: AgentStatus::Idle,
            status_text: "Ready. Press [D] to run autonomous demo.".to_string(),
            current_node: None,
            completed_nodes: Vec::new(),
            failed_nodes: Vec::new(),
            logs: vec![
                "Welcome to CodingBot Terminal Dashboard.".to_string(),
                "LangGraph state machine initialized and ready.".to_string(),
                "Press [D] to trigger autonomous demo with self-correction.".to_string(),
            ],
            active_tab: RightTab::Logs,
            progress_percent: 0,
            iteration: 0,
            max_retries: 3,
            task: "Implement Fibonacci with self-healing tests".to_string(),
            repo_path: ".".to_string(),
            git_branch: "main".to_string(),
            diff_preview: "(No diff generated yet. Run agent or demo to generate)".to_string(),
            pr_preview: "(No Pull Request summary generated yet)".to_string(),
        }
    }
}

pub async fn run_terminal_gui() -> Result<()> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let app_state = Arc::new(Mutex::new(AppState::default()));
    let is_task_running = Arc::new(AtomicBool::new(false));

    let res = run_loop(&mut terminal, app_state, is_task_running).await;

    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    if let Err(err) = res {
        eprintln!("Error in Terminal GUI: {:?}", err);
    }

    Ok(())
}

async fn run_loop<B: Backend>(
    terminal: &mut Terminal<B>,
    app_state: Arc<Mutex<AppState>>,
    is_task_running: Arc<AtomicBool>,
) -> Result<()> {
    let tick_rate = Duration::from_millis(100);
    let mut last_tick = Instant::now();

    loop {
        {
            let state = app_state.lock().unwrap();
            terminal.draw(|f| ui(f, &state))?;
        }

        let timeout = tick_rate
            .checked_sub(last_tick.elapsed())
            .unwrap_or_else(|| Duration::from_secs(0));

        if crossterm::event::poll(timeout)? {
            if let Event::Key(key) = event::read()? {
                match key.code {
                    KeyCode::Char('q') | KeyCode::Esc => return Ok(()),
                    KeyCode::Tab => {
                        let mut state = app_state.lock().unwrap();
                        state.active_tab = match state.active_tab {
                            RightTab::Logs => RightTab::DiffAndPr,
                            RightTab::DiffAndPr => RightTab::Logs,
                        };
                    }
                    KeyCode::Char('d') | KeyCode::Char('D') => {
                        if !is_task_running.load(Ordering::SeqCst) {
                            spawn_demo_task(app_state.clone(), is_task_running.clone());
                        }
                    }
                    KeyCode::Char('t') | KeyCode::Char('T') => {
                        if !is_task_running.load(Ordering::SeqCst) {
                            spawn_test_task(app_state.clone(), is_task_running.clone());
                        }
                    }
                    _ => {}
                }
            }
        }

        if last_tick.elapsed() >= tick_rate {
            last_tick = Instant::now();
        }
    }
}

fn spawn_demo_task(app_state: Arc<Mutex<AppState>>, is_task_running: Arc<AtomicBool>) {
    is_task_running.store(true, Ordering::SeqCst);

    {
        let mut state = app_state.lock().unwrap();
        state.status = AgentStatus::Running;
        state.status_text = "Running autonomous self-healing demo...".to_string();
        state.completed_nodes.clear();
        state.failed_nodes.clear();
        state.progress_percent = 5;
        state.logs.push("[DEMO] Initializing sandbox environment...".to_string());
    }

    tokio::spawn(async move {
        let run_result = run_demo_workflow(app_state.clone()).await;

        let mut state = app_state.lock().unwrap();
        match run_result {
            Ok(_) => {
                state.status = AgentStatus::Success;
                state.status_text = "Workflow completed successfully! (All tests green)".to_string();
                state.progress_percent = 100;
            }
            Err(e) => {
                state.status = AgentStatus::Failed;
                state.status_text = format!("Workflow failed: {}", e);
                state.logs.push(format!("[ERROR] {}", e));
            }
        }
        is_task_running.store(false, Ordering::SeqCst);
    });
}

fn spawn_test_task(app_state: Arc<Mutex<AppState>>, is_task_running: Arc<AtomicBool>) {
    is_task_running.store(true, Ordering::SeqCst);

    {
        let mut state = app_state.lock().unwrap();
        state.status = AgentStatus::Running;
        state.status_text = "Running 'cargo test'...".to_string();
        state.logs.push("[TEST] Spawning test runner...".to_string());
    }

    tokio::spawn(async move {
        let output = std::process::Command::new("cargo")
            .arg("test")
            .output();

        let mut state = app_state.lock().unwrap();
        match output {
            Ok(out) => {
                let stdout = String::from_utf8_lossy(&out.stdout);
                for line in stdout.lines().take(15) {
                    state.logs.push(line.to_string());
                }
                if out.status.success() {
                    state.status = AgentStatus::Success;
                    state.status_text = "All tests passed!".to_string();
                } else {
                    state.status = AgentStatus::Failed;
                    state.status_text = "Test suite failed.".to_string();
                }
            }
            Err(e) => {
                state.status = AgentStatus::Failed;
                state.status_text = format!("Failed to run test command: {}", e);
            }
        }
        is_task_running.store(false, Ordering::SeqCst);
    });
}

async fn run_demo_workflow(app_state: Arc<Mutex<AppState>>) -> Result<()> {
    let temp_dir = tempfile::tempdir()?;
    let repo_path = temp_dir.path().to_path_buf();

    git::git_init(&repo_path)?;
    let _ = std::process::Command::new("git")
        .args(["config", "user.name", "CodingBot Agent"])
        .current_dir(&repo_path)
        .output();
    let _ = std::process::Command::new("git")
        .args(["config", "user.email", "agent@codingbot.local"])
        .current_dir(&repo_path)
        .output();

    let cargo_toml = r#"[package]
name = "math_lib"
version = "0.1.0"
edition = "2021"

[dependencies]
"#;
    std::fs::write(repo_path.join("Cargo.toml"), cargo_toml)?;
    std::fs::create_dir_all(repo_path.join("src"))?;
    let initial_lib = r#"pub fn add(a: i32, b: i32) -> i32 { a + b }
#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn test_add() { assert_eq!(add(2, 2), 4); }
}
"#;
    std::fs::write(repo_path.join("src").join("lib.rs"), initial_lib)?;
    git::git_commit_all(&repo_path, "initial commit")?;

    let task = "Implement the Fibonacci function fibonacci(n: u64) -> u64 with unit tests";
    let mut state = AgentState::new(repo_path.clone(), task, 3);
    state.metadata.insert("is_demo".to_string(), "true".to_string());
    state.metadata.insert("create_branch".to_string(), "true".to_string());

    let llm = Arc::new(HeuristicEngine::new());
    let mut graph = build_autonomous_coding_graph(llm);

    // Register callback hook to stream steps into the TUI
    let hook_state = app_state.clone();
    graph.on_step(move |node_name, current_state| {
        let mut app = hook_state.lock().unwrap();
        app.current_node = Some(node_name.to_string());
        app.completed_nodes.push(node_name.to_string());
        app.iteration = current_state.iteration;
        app.logs.push(format!("[NODE] Step completed: {}", node_name.to_uppercase()));

        // Update progress
        match node_name {
            "index_repo" => app.progress_percent = 20,
            "plan_changes" => app.progress_percent = 40,
            "generate_code" => app.progress_percent = 55,
            "run_tests" => {
                if let Some(res) = &current_state.test_result {
                    if res.success {
                        app.progress_percent = 85;
                        app.logs.push("[TEST] Tests PASSED! All test assertions green.".to_string());
                    } else {
                        app.progress_percent = 65;
                        app.failed_nodes.push("run_tests".to_string());
                        app.logs.push("[TEST] Tests FAILED! Triggering self-correction...".to_string());
                    }
                }
            }
            "self_correct" => {
                app.progress_percent = 75;
                app.logs.push(format!("[REPAIR] Self-correction patch applied (Attempt {}/{})", current_state.iteration, current_state.max_retries));
            }
            "create_diff_pr" => {
                app.progress_percent = 100;
                if let Some(diff) = &current_state.diff {
                    app.diff_preview = diff.clone();
                }
                if let Some(pr) = &current_state.pr_summary {
                    app.pr_preview = pr.summary_markdown.clone();
                }
                app.logs.push("[DONE] Git diff and Pull Request generated.".to_string());
            }
            _ => {}
        }
    });

    let final_state = graph.run(state).await?;

    {
        let mut app = app_state.lock().unwrap();
        if let Some(diff) = final_state.diff {
            app.diff_preview = diff;
        }
        if let Some(pr) = final_state.pr_summary {
            app.pr_preview = pr.summary_markdown;
        }
    }

    Ok(())
}

fn ui(f: &mut Frame, state: &AppState) {
    let size = f.area();

    // Vertical layout: Header, Gauge, Main Content, Footer
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Header
            Constraint::Length(3), // Progress Gauge
            Constraint::Min(10),   // Main area
            Constraint::Length(3), // Footer keybindings
        ])
        .split(size);

    // 1. Header Block
    let status_style = match state.status {
        AgentStatus::Idle => Style::default().fg(Color::Yellow),
        AgentStatus::Running => Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
        AgentStatus::Success => Style::default().fg(Color::Green).add_modifier(Modifier::BOLD),
        AgentStatus::Failed => Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
    };

    let status_badge = match state.status {
        AgentStatus::Idle => "[IDLE]",
        AgentStatus::Running => "[RUNNING]",
        AgentStatus::Success => "[SUCCESS]",
        AgentStatus::Failed => "[FAILED]",
    };

    let header_text = vec![
        Span::styled("CODINGBOT: ", Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
        Span::styled("Autonomous Coding Agent Runtime  ", Style::default().fg(Color::Gray)),
        Span::styled(format!("Status: {} {}  ", status_badge, state.status_text), status_style),
        Span::styled(format!("Iter: {}/{} ", state.iteration, state.max_retries), Style::default().fg(Color::Magenta)),
    ];
    let header_p = Paragraph::new(Line::from(header_text))
        .block(Block::default().borders(Borders::ALL).border_type(BorderType::Rounded).title(" System Status "));
    f.render_widget(header_p, chunks[0]);

    // 2. Progress Gauge
    let gauge_color = match state.status {
        AgentStatus::Success => Color::Green,
        AgentStatus::Failed => Color::Red,
        _ => Color::Cyan,
    };
    let gauge = Gauge::default()
        .block(Block::default().borders(Borders::ALL).border_type(BorderType::Rounded).title(" Workflow Cycle Execution Progress "))
        .gauge_style(Style::default().fg(gauge_color).bg(Color::DarkGray))
        .percent(state.progress_percent)
        .label(format!("{}% Completed", state.progress_percent));
    f.render_widget(gauge, chunks[1]);

    // 3. Main Area: Horizontal Split
    let main_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(42), // Left Pane: State Machine Pipeline
            Constraint::Percentage(58), // Right Pane: Live Logs / Diff
        ])
        .split(chunks[2]);

    // Left Pane: State Machine Pipeline
    let pipeline_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(10), // Pipeline Nodes
            Constraint::Min(5),    // State Info Card
        ])
        .split(main_chunks[0]);

    let nodes = [
        ("index_repo", "1. Index Repository (Git & AST)"),
        ("plan_changes", "2. Plan Architecture & Changes"),
        ("generate_code", "3. Synthesize Code Patches"),
        ("run_tests", "4. Run Test Suite (cargo test)"),
        ("self_correct", "5. Self-Correction Cycle"),
        ("create_diff_pr", "6. Output Clean Git Diff & PR"),
    ];

    let mut list_items = Vec::new();
    for (id, label) in nodes {
        let is_current = state.current_node.as_deref() == Some(id);
        let is_completed = state.completed_nodes.contains(&id.to_string());
        let is_failed = state.failed_nodes.contains(&id.to_string());

        let (prefix, style) = if is_current {
            (" ==> ", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD))
        } else if is_failed {
            (" [X] ", Style::default().fg(Color::Red))
        } else if is_completed {
            (" [V] ", Style::default().fg(Color::Green))
        } else {
            (" [ ] ", Style::default().fg(Color::DarkGray))
        };

        list_items.push(ListItem::new(Line::from(vec![
            Span::styled(prefix, style),
            Span::styled(label, style),
        ])));
    }

    let pipeline_list = List::new(list_items)
        .block(Block::default().borders(Borders::ALL).border_type(BorderType::Rounded).title(" LangGraph State Machine Nodes "));
    f.render_widget(pipeline_list, pipeline_chunks[0]);

    // State Info Card
    let info_lines = vec![
        Line::from(vec![
            Span::styled("Target Task:   ", Style::default().fg(Color::Cyan)),
            Span::raw(&state.task),
        ]),
        Line::from(vec![
            Span::styled("Active Branch: ", Style::default().fg(Color::Cyan)),
            Span::styled(&state.git_branch, Style::default().fg(Color::Green)),
        ]),
        Line::from(vec![
            Span::styled("Test Harness:  ", Style::default().fg(Color::Cyan)),
            Span::raw("cargo test"),
        ]),
        Line::from(vec![
            Span::styled("LLM Backend:   ", Style::default().fg(Color::Cyan)),
            Span::styled("Heuristic Engine (Autonomous)", Style::default().fg(Color::Yellow)),
        ]),
        Line::from(vec![
            Span::styled("Self-Healing:  ", Style::default().fg(Color::Cyan)),
            Span::styled("Active (Up to 3 iterations)", Style::default().fg(Color::Green)),
        ]),
    ];
    let info_p = Paragraph::new(info_lines)
        .block(Block::default().borders(Borders::ALL).border_type(BorderType::Rounded).title(" Session Metadata "))
        .wrap(Wrap { trim: true });
    f.render_widget(info_p, pipeline_chunks[1]);

    // Right Pane: Tabs (Logs or Diff/PR)
    let right_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Tab Selector
            Constraint::Min(5),    // Content
        ])
        .split(main_chunks[1]);

    let tab_titles = vec!["Live Activity Logs", "Git Diff & PR Preview"];
    let tab_idx = match state.active_tab {
        RightTab::Logs => 0,
        RightTab::DiffAndPr => 1,
    };
    let tabs = Tabs::new(tab_titles)
        .select(tab_idx)
        .block(Block::default().borders(Borders::ALL).border_type(BorderType::Rounded).title(" Inspector View [Tab to Switch] "))
        .style(Style::default().fg(Color::DarkGray))
        .highlight_style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD));
    f.render_widget(tabs, right_chunks[0]);

    match state.active_tab {
        RightTab::Logs => {
            let log_items: Vec<ListItem> = state
                .logs
                .iter()
                .rev()
                .take(30)
                .map(|l| {
                    let style = if l.contains("[ERROR]") || l.contains("[FAIL]") {
                        Style::default().fg(Color::Red)
                    } else if l.contains("[PASS]") || l.contains("[DONE]") {
                        Style::default().fg(Color::Green)
                    } else if l.contains("[REPAIR]") || l.contains("[NODE]") {
                        Style::default().fg(Color::Yellow)
                    } else {
                        Style::default().fg(Color::White)
                    };
                    ListItem::new(Line::from(Span::styled(l, style)))
                })
                .collect();

            let log_list = List::new(log_items)
                .block(Block::default().borders(Borders::ALL).border_type(BorderType::Rounded).title(" Real-Time Agent Stream "));
            f.render_widget(log_list, right_chunks[1]);
        }
        RightTab::DiffAndPr => {
            let text = if !state.diff_preview.is_empty() && state.diff_preview != "(No diff generated yet. Run agent or demo to generate)" {
                format!("=== GENERATED GIT DIFF ===\n{}\n\n=== PULL REQUEST PREVIEW ===\n{}", state.diff_preview, state.pr_preview)
            } else {
                state.diff_preview.clone()
            };
            let diff_p = Paragraph::new(text)
                .block(Block::default().borders(Borders::ALL).border_type(BorderType::Rounded).title(" Unified Git Diff & PR Preview "))
                .wrap(Wrap { trim: false })
                .style(Style::default().fg(Color::Green));
            f.render_widget(diff_p, right_chunks[1]);
        }
    }

    // 4. Footer Keybindings
    let keys = vec![
        Span::styled(" [D] ", Style::default().fg(Color::Black).bg(Color::Cyan)),
        Span::raw(" Run Demo  "),
        Span::styled(" [T] ", Style::default().fg(Color::Black).bg(Color::Yellow)),
        Span::raw(" Run Tests  "),
        Span::styled(" [Tab] ", Style::default().fg(Color::Black).bg(Color::Magenta)),
        Span::raw(" Switch Tab  "),
        Span::styled(" [Q / Esc] ", Style::default().fg(Color::White).bg(Color::Red)),
        Span::raw(" Quit"),
    ];
    let footer_p = Paragraph::new(Line::from(keys))
        .alignment(Alignment::Center)
        .block(Block::default().borders(Borders::ALL).border_type(BorderType::Rounded));
    f.render_widget(footer_p, chunks[3]);
}
