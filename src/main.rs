use anyhow::Result;
use chrono::Local;
use clap::Parser;
use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph, Wrap},
    Frame, Terminal,
};
use std::{
    io::{self, Write},
    process::{Command, Stdio},
};

mod git;
use git::{BackupRef, GitEntry, GitManager, RestoreMode};

#[derive(Parser)]
#[command(name = "git-time-machine")]
#[command(about = "🕰️  Browse Git reflog and restore reachable local history", long_about = None)]
#[command(after_help = "EXAMPLES:\n  \
    git-time-machine              # Show last 50 reflog entries\n  \
    git-time-machine --all        # Show up to 1000 reflog entries\n  \
    git-time-machine --export-json # Export reflog as JSON for automation\n  \
    git-time-machine --list-backups # List hard-reset backup refs\n\n\
CONTROLS:\n  \
    ↑/k, ↓/j    Navigate up/down\n  \
    Home/End    Jump to first/last entry\n  \
    gg/G        Jump to first/last entry (vim-style)\n  \
    PgUp/PgDn   Jump 10 entries\n  \
    Space       Toggle diff panel\n  \
    d           Switch between diff summary and full diff\n  \
    t           Toggle relative/absolute timestamps\n  \
    y           Copy selected commit hash to clipboard\n  \
    ?           Show contextual help\n  \
    Enter       Hard reset to selected commit (creates backup ref first)\n  \
    s           Soft reset to selected commit\n  \
    c           Checkout selected commit (detached HEAD)\n  \
    /           Search/filter commits by message, hash, author, or time\n  \
    Esc         Clear active filter (or quit if no filter)\n  \
    q           Quit\n\n\
SEARCH MODE:\n  \
    type        Filter commits (case-insensitive, multi-word AND)\n  \
    Enter       Apply filter and return to navigation\n  \
    Esc         Cancel search and clear filter\n  \
    Backspace   Delete last character")]
struct Cli {
    /// Show up to 1000 reflog entries (default: last 50)
    #[arg(short, long)]
    all: bool,

    /// Export reflog timeline as JSON
    #[arg(long, conflicts_with = "list_backups")]
    export_json: bool,

    /// List hard-reset backup refs and recovery commands
    #[arg(long, conflicts_with = "export_json")]
    list_backups: bool,
}

struct App {
    git_manager: GitManager,
    entries: Vec<GitEntry>,
    list_state: ListState,
    show_confirmation: bool,
    show_diff: bool,
    show_full_diff: bool,
    diff_content: String,
    full_diff_content: String,
    diff_scroll_offset: u16,
    diff_visible_height: u16,
    has_uncommitted_changes: bool,
    search_mode: bool,
    search_query: String,
    filtered_entries: Vec<usize>,
    search_active: bool,
    show_absolute_time: bool,
    last_key_was_g: bool,
    pending_restore_mode: Option<RestoreMode>,
    show_help: bool,
    status_message: Option<String>,
}

struct RestoreSummary {
    hash: String,
    message: String,
    mode: RestoreMode,
    backup_ref: Option<String>,
}

impl App {
    fn new(show_all: bool) -> Result<Self> {
        let git_manager = GitManager::new()?;
        let entries = git_manager.get_reflog_entries(show_all)?;
        let has_uncommitted_changes = git_manager.has_uncommitted_changes()?;

        let mut list_state = ListState::default();
        if !entries.is_empty() {
            list_state.select(Some(0));
        }

        let filtered_entries = (0..entries.len()).collect();

        Ok(Self {
            git_manager,
            entries,
            list_state,
            show_confirmation: false,
            show_diff: false,
            show_full_diff: false,
            diff_content: String::new(),
            full_diff_content: String::new(),
            diff_scroll_offset: 0,
            diff_visible_height: 10,
            has_uncommitted_changes,
            search_mode: false,
            search_query: String::new(),
            filtered_entries,
            search_active: false,
            show_absolute_time: false,
            last_key_was_g: false,
            pending_restore_mode: None,
            show_help: false,
            status_message: None,
        })
    }

    fn selected_index(&self) -> usize {
        self.list_state.selected().unwrap_or(0)
    }

    fn selected_entry_idx(&self) -> Option<usize> {
        let sel = self.list_state.selected()?;
        self.filtered_entries.get(sel).copied()
    }

    fn selected_entry(&self) -> Option<&GitEntry> {
        self.selected_entry_idx()
            .and_then(|idx| self.entries.get(idx))
    }

    fn update_filter(&mut self) {
        let query_lower = self.search_query.to_lowercase();
        let tokens: Vec<&str> = query_lower.split_whitespace().collect();
        if tokens.is_empty() {
            self.filtered_entries = (0..self.entries.len()).collect();
        } else {
            self.filtered_entries = self
                .entries
                .iter()
                .enumerate()
                .filter(|(_, entry)| entry_matches_query(entry, &tokens))
                .map(|(i, _)| i)
                .collect();
        }
        if self.filtered_entries.is_empty() {
            self.list_state.select(None);
        } else {
            let sel = self.list_state.selected().unwrap_or(0);
            if sel >= self.filtered_entries.len() {
                self.list_state
                    .select(Some(self.filtered_entries.len() - 1));
            } else if self.list_state.selected().is_none() {
                self.list_state.select(Some(0));
            }
        }
    }

    fn set_status_message(&mut self, message: impl Into<String>) {
        self.status_message = Some(message.into());
    }

    fn clear_status_message(&mut self) {
        self.status_message = None;
    }

    fn clear_filter(&mut self) {
        self.search_query.clear();
        self.search_active = false;
        self.search_mode = false;
        self.clear_status_message();
        self.filtered_entries = (0..self.entries.len()).collect();
        if !self.entries.is_empty() {
            self.list_state.select(Some(0));
        }
    }

    fn update_diff_if_visible(&mut self) -> Result<()> {
        if let Some(idx) = self.selected_entry_idx() {
            if let Some(entry) = self.entries.get(idx) {
                if self.show_diff {
                    self.diff_content = self.git_manager.get_diff_stat(&entry.hash)?;
                }
                if self.show_full_diff {
                    self.full_diff_content = self.git_manager.get_full_diff(&entry.hash)?;
                }
                self.diff_scroll_offset = 0;
            }
        }
        Ok(())
    }

    fn next(&mut self) -> Result<()> {
        if self.filtered_entries.is_empty() {
            return Ok(());
        }
        let i = match self.list_state.selected() {
            Some(i) => {
                if i >= self.filtered_entries.len() - 1 {
                    i // Clamp at bottom instead of wrap-around
                } else {
                    i + 1
                }
            }
            None => 0,
        };
        self.list_state.select(Some(i));
        self.clear_status_message();
        self.update_diff_if_visible()?;
        Ok(())
    }

    fn previous(&mut self) -> Result<()> {
        if self.filtered_entries.is_empty() {
            return Ok(());
        }
        let i = match self.list_state.selected() {
            Some(i) => {
                if i == 0 {
                    0 // Clamp at top instead of wrap-around
                } else {
                    i - 1
                }
            }
            None => 0,
        };
        self.list_state.select(Some(i));
        self.clear_status_message();
        self.update_diff_if_visible()?;
        Ok(())
    }

    fn toggle_diff(&mut self) -> Result<()> {
        self.clear_status_message();
        self.show_diff = !self.show_diff;
        if self.show_diff {
            self.show_full_diff = false;
            if let Some(idx) = self.selected_entry_idx() {
                if let Some(entry) = self.entries.get(idx) {
                    self.diff_content = self.git_manager.get_diff_stat(&entry.hash)?;
                }
            }
        }
        self.diff_scroll_offset = 0;
        Ok(())
    }

    fn toggle_diff_mode(&mut self) -> Result<()> {
        if !self.show_diff {
            return Ok(());
        }
        self.clear_status_message();
        self.show_full_diff = !self.show_full_diff;
        if self.show_full_diff {
            if let Some(idx) = self.selected_entry_idx() {
                if let Some(entry) = self.entries.get(idx) {
                    self.full_diff_content = self.git_manager.get_full_diff(&entry.hash)?;
                }
            }
        }
        self.diff_scroll_offset = 0;
        Ok(())
    }

    fn scroll_diff_up(&mut self) {
        self.diff_scroll_offset = self.diff_scroll_offset.saturating_sub(1);
    }

    fn active_diff_content(&self) -> &str {
        if self.show_full_diff {
            &self.full_diff_content
        } else {
            &self.diff_content
        }
    }

    fn scroll_diff_down(&mut self) {
        let line_count = self.active_diff_content().lines().count() as u16;
        let max_scroll = line_count.saturating_sub(self.diff_visible_height);
        self.diff_scroll_offset = (self.diff_scroll_offset + 1).min(max_scroll);
    }

    fn show_confirmation_dialog(&mut self, mode: RestoreMode) {
        self.pending_restore_mode = Some(mode);
        self.show_confirmation = true;
        self.clear_status_message();
    }

    fn cancel_confirmation(&mut self) {
        self.show_confirmation = false;
        self.pending_restore_mode = None;
    }

    fn restore_selected(&mut self) -> Result<Option<RestoreSummary>> {
        let Some(idx) = self.selected_entry_idx() else {
            return Ok(None);
        };
        if let Some(entry) = self.entries.get(idx) {
            let mode = self.pending_restore_mode.unwrap_or(RestoreMode::HardReset);
            let outcome = self.git_manager.restore_to_commit(&entry.hash, mode)?;
            self.pending_restore_mode = None;
            Ok(Some(RestoreSummary {
                hash: entry.short_hash(),
                message: entry.message.clone(),
                mode: outcome.mode,
                backup_ref: outcome.backup_ref,
            }))
        } else {
            Ok(None)
        }
    }

    fn copy_selected_hash(&mut self) {
        let Some(hash) = self.selected_entry().map(|entry| entry.hash.clone()) else {
            self.set_status_message("No commit selected to copy.");
            return;
        };

        match copy_to_clipboard(&hash) {
            Ok(()) => self.set_status_message(format!(
                "Copied {} to clipboard.",
                hash.chars().take(7).collect::<String>()
            )),
            Err(err) => self.set_status_message(format!(
                "Could not copy {}: {}",
                hash.chars().take(7).collect::<String>(),
                err
            )),
        }
    }
}

fn entry_matches_query(entry: &GitEntry, tokens: &[&str]) -> bool {
    tokens.iter().all(|token| {
        let token = token.to_lowercase();
        entry.message.to_lowercase().contains(&token)
            || entry.hash.to_lowercase().contains(&token)
            || entry.author.to_lowercase().contains(&token)
            || entry.relative_time.to_lowercase().contains(&token)
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ClipboardCommand {
    program: &'static str,
    args: &'static [&'static str],
}

fn clipboard_commands() -> Vec<ClipboardCommand> {
    #[cfg(target_os = "macos")]
    {
        vec![ClipboardCommand {
            program: "pbcopy",
            args: &[],
        }]
    }

    #[cfg(target_os = "windows")]
    {
        vec![ClipboardCommand {
            program: "clip",
            args: &[],
        }]
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    {
        vec![
            ClipboardCommand {
                program: "wl-copy",
                args: &[],
            },
            ClipboardCommand {
                program: "xclip",
                args: &["-selection", "clipboard"],
            },
            ClipboardCommand {
                program: "xsel",
                args: &["--clipboard", "--input"],
            },
        ]
    }
}

fn copy_to_clipboard(text: &str) -> std::result::Result<(), String> {
    let commands = clipboard_commands();
    let mut failures = Vec::new();

    for command in commands {
        match run_clipboard_command(command, text) {
            Ok(()) => return Ok(()),
            Err(err) => failures.push(format!("{}: {}", command.program, err)),
        }
    }

    if failures.is_empty() {
        Err("clipboard command is not configured for this platform".to_string())
    } else {
        Err(format!("clipboard unavailable ({})", failures.join("; ")))
    }
}

fn run_clipboard_command(command: ClipboardCommand, text: &str) -> io::Result<()> {
    let mut child = Command::new(command.program)
        .args(command.args)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()?;

    if let Some(stdin) = child.stdin.as_mut() {
        stdin.write_all(text.as_bytes())?;
    }

    let status = child.wait()?;
    if status.success() {
        Ok(())
    } else {
        Err(io::Error::other(format!("exited with {status}")))
    }
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    if cli.export_json {
        let git_manager = GitManager::new()?;
        let entries = git_manager.get_reflog_entries(cli.all)?;
        println!("{}", serde_json::to_string_pretty(&entries)?);
        return Ok(());
    }

    if cli.list_backups {
        let git_manager = GitManager::new()?;
        let backups = git_manager.list_backup_refs()?;
        print_backup_refs(&backups);
        return Ok(());
    }

    // Setup panic hook to restore terminal
    let original_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |panic_info| {
        let _ = disable_raw_mode();
        let _ = execute!(io::stdout(), LeaveAlternateScreen);
        original_hook(panic_info);
    }));

    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Create app and run
    let mut app = App::new(cli.all)?;
    let res = run_app(&mut terminal, &mut app);

    // Restore terminal
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    match res {
        Ok(Some(summary)) => {
            println!(
                "✅ Completed {} to {} - {}",
                summary.mode.label(),
                summary.hash,
                summary.message
            );
            if let Some(backup_ref) = summary.backup_ref {
                println!("Backup ref: {}", backup_ref);
            }
            Ok(())
        }
        Ok(None) => Ok(()),
        Err(err) => {
            println!("Error: {:?}", err);
            Err(err)
        }
    }
}

fn print_backup_refs(backups: &[BackupRef]) {
    if backups.is_empty() {
        println!("No git-time-machine backup refs found.");
        println!("Hard reset creates refs under refs/git-time-machine/backups/.");
        return;
    }

    println!("Backup refs (newest first):");
    for backup in backups {
        println!(
            "{}  {}  {}",
            backup.relative_time,
            backup.short_hash(),
            backup.name
        );
        if !backup.subject.is_empty() {
            println!("    commit: {}", backup.subject);
        }
        println!("    inspect: {}", backup.inspect_command());
        println!("    restore: {}", backup.restore_command());
    }
}

fn run_app<B: ratatui::backend::Backend>(
    terminal: &mut Terminal<B>,
    app: &mut App,
) -> Result<Option<RestoreSummary>> {
    loop {
        terminal.draw(|f| ui(f, app))?;

        if let Event::Key(key) = event::read()? {
            // Only handle key press events, ignore key release to prevent double-triggering on Windows
            if key.kind != KeyEventKind::Press {
                continue;
            }

            if app.show_help {
                match key.code {
                    KeyCode::Esc | KeyCode::Char('?') | KeyCode::Char('q') => {
                        app.show_help = false;
                    }
                    _ => {}
                }
                continue;
            }

            if app.search_mode {
                match key.code {
                    KeyCode::Char('?') => {
                        app.show_help = true;
                    }
                    KeyCode::Esc => {
                        app.search_mode = false;
                        app.search_query.clear();
                        app.search_active = false;
                        app.update_filter();
                        if !app.filtered_entries.is_empty() {
                            app.list_state.select(Some(0));
                        }
                        app.update_diff_if_visible()?;
                    }
                    KeyCode::Enter => {
                        app.search_mode = false;
                        app.search_active = !app.search_query.is_empty();
                    }
                    KeyCode::Backspace => {
                        app.search_query.pop();
                        app.update_filter();
                        app.update_diff_if_visible()?;
                    }
                    KeyCode::Char(c) => {
                        app.search_query.push(c);
                        app.update_filter();
                        app.update_diff_if_visible()?;
                    }
                    _ => {}
                }
                continue;
            }

            if app.show_confirmation {
                match key.code {
                    KeyCode::Char('?') => {
                        app.show_help = true;
                    }
                    KeyCode::Char('y') | KeyCode::Char('Y') => {
                        return app.restore_selected();
                    }
                    KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
                        app.cancel_confirmation();
                    }
                    _ => {}
                }
            } else {
                match key.code {
                    KeyCode::Char('q') => return Ok(None),
                    KeyCode::Esc => {
                        if app.search_active {
                            app.clear_filter();
                            app.update_diff_if_visible()?;
                        } else {
                            return Ok(None);
                        }
                    }
                    KeyCode::Char('/') => {
                        app.search_mode = true;
                        app.clear_status_message();
                    }
                    KeyCode::Char('?') => {
                        app.show_help = true;
                    }
                    KeyCode::Down | KeyCode::Char('j') => {
                        if app.show_diff && key.modifiers.contains(event::KeyModifiers::SHIFT) {
                            app.scroll_diff_down();
                        } else {
                            app.next()?;
                        }
                    }
                    KeyCode::Up | KeyCode::Char('k') => {
                        if app.show_diff && key.modifiers.contains(event::KeyModifiers::SHIFT) {
                            app.scroll_diff_up();
                        } else {
                            app.previous()?;
                        }
                    }
                    KeyCode::Char('J') if app.show_diff => {
                        app.scroll_diff_down();
                    }
                    KeyCode::Char('K') if app.show_diff => {
                        app.scroll_diff_up();
                    }
                    KeyCode::Home if !app.filtered_entries.is_empty() => {
                        app.list_state.select(Some(0));
                        app.clear_status_message();
                        app.update_diff_if_visible()?;
                    }
                    KeyCode::End if !app.filtered_entries.is_empty() => {
                        let last = app.filtered_entries.len() - 1;
                        app.list_state.select(Some(last));
                        app.clear_status_message();
                        app.update_diff_if_visible()?;
                    }
                    KeyCode::Char('g') => {
                        if app.last_key_was_g && !app.filtered_entries.is_empty() {
                            app.list_state.select(Some(0));
                            app.clear_status_message();
                            app.update_diff_if_visible()?;
                            app.last_key_was_g = false;
                        } else {
                            app.last_key_was_g = true;
                        }
                        continue;
                    }
                    KeyCode::Char('G') if !app.filtered_entries.is_empty() => {
                        let last = app.filtered_entries.len() - 1;
                        app.list_state.select(Some(last));
                        app.clear_status_message();
                        app.update_diff_if_visible()?;
                    }
                    KeyCode::PageDown if !app.filtered_entries.is_empty() => {
                        let current = app.list_state.selected().unwrap_or(0);
                        let next = (current + 10).min(app.filtered_entries.len() - 1);
                        app.list_state.select(Some(next));
                        app.clear_status_message();
                        app.update_diff_if_visible()?;
                    }
                    KeyCode::PageUp if !app.filtered_entries.is_empty() => {
                        let current = app.list_state.selected().unwrap_or(0);
                        let prev = current.saturating_sub(10);
                        app.list_state.select(Some(prev));
                        app.clear_status_message();
                        app.update_diff_if_visible()?;
                    }
                    KeyCode::Char(' ') => {
                        app.toggle_diff()?;
                    }
                    KeyCode::Char('d') => {
                        app.toggle_diff_mode()?;
                    }
                    KeyCode::Char('t') => {
                        app.clear_status_message();
                        app.show_absolute_time = !app.show_absolute_time;
                    }
                    KeyCode::Char('y') => {
                        app.copy_selected_hash();
                    }
                    KeyCode::Enter if app.selected_entry_idx().is_some() => {
                        app.show_confirmation_dialog(RestoreMode::HardReset);
                    }
                    KeyCode::Char('s') if app.selected_entry_idx().is_some() => {
                        app.show_confirmation_dialog(RestoreMode::SoftReset);
                    }
                    KeyCode::Char('c') if app.selected_entry_idx().is_some() => {
                        app.show_confirmation_dialog(RestoreMode::Checkout);
                    }
                    _ => {}
                }
                app.last_key_was_g = false;
            }
        }
    }
}

fn ui(f: &mut Frame, app: &mut App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(0),
            Constraint::Length(4),
        ])
        .split(f.area());

    // Header with warning if uncommitted changes
    let header_text = if app.has_uncommitted_changes {
        vec![Line::from(vec![
            Span::styled(
                "⚠️  ",
                Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                "UNCOMMITTED CHANGES DETECTED",
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw("  |  "),
            Span::styled("Navigate: ↑↓/jk", Style::default().fg(Color::Gray)),
            Span::raw("  |  "),
            Span::styled("Diff: Space", Style::default().fg(Color::Cyan)),
            Span::raw("  |  "),
            Span::styled("Restore: Enter/s/c", Style::default().fg(Color::Red)),
            Span::raw("  |  "),
            Span::styled("Copy: y", Style::default().fg(Color::Green)),
            Span::raw("  |  "),
            Span::styled("Help: ?", Style::default().fg(Color::Cyan)),
            Span::raw("  |  "),
            Span::styled("Quit: q", Style::default().fg(Color::Red)),
        ])]
    } else {
        vec![Line::from(vec![
            Span::styled("🕰️  ", Style::default().fg(Color::Cyan)),
            Span::styled(
                "Git Time Machine",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw("  |  "),
            Span::styled("Navigate: ↑↓/jk", Style::default().fg(Color::Gray)),
            Span::raw("  |  "),
            Span::styled("Diff: Space", Style::default().fg(Color::Cyan)),
            Span::raw("  |  "),
            Span::styled("Restore: Enter/s/c", Style::default().fg(Color::Red)),
            Span::raw("  |  "),
            Span::styled("Copy: y", Style::default().fg(Color::Green)),
            Span::raw("  |  "),
            Span::styled("Help: ?", Style::default().fg(Color::Cyan)),
            Span::raw("  |  "),
            Span::styled("Quit: q", Style::default().fg(Color::Red)),
        ])]
    };

    let header =
        Paragraph::new(header_text).block(Block::default().borders(Borders::ALL).border_style(
            if app.has_uncommitted_changes {
                Style::default().fg(Color::Red)
            } else {
                Style::default().fg(Color::Cyan)
            },
        ));
    f.render_widget(header, chunks[0]);

    // Main content area - split if showing diff
    let main_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints(if app.show_diff {
            vec![Constraint::Percentage(50), Constraint::Percentage(50)]
        } else {
            vec![Constraint::Percentage(100)]
        })
        .split(chunks[1]);

    // Timeline list
    let selected_idx = app.selected_index();
    let query_lower = app.search_query.to_lowercase();
    let query_tokens: Vec<String> = query_lower
        .split_whitespace()
        .map(|s| s.to_string())
        .collect();
    let highlight_query = (app.search_active || app.search_mode) && !query_tokens.is_empty();
    let items: Vec<ListItem> = app
        .filtered_entries
        .iter()
        .enumerate()
        .filter_map(|(i, &entry_idx)| {
            let entry = app.entries.get(entry_idx)?;
            let is_selected = i == selected_idx;
            let style = if is_selected {
                Style::default()
                    .bg(Color::DarkGray)
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::White)
            };

            let prefix = if is_selected { "▶ " } else { "  " };
            let time_style = if is_selected {
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::DarkGray)
            };

            let mut spans = vec![
                Span::styled(prefix, style),
                Span::styled(
                    if app.show_absolute_time {
                        entry
                            .timestamp
                            .with_timezone(&Local)
                            .format("%Y-%m-%d %H:%M:%S")
                            .to_string()
                    } else {
                        entry.relative_time.clone()
                    },
                    time_style,
                ),
                Span::raw("  "),
                Span::styled(entry.short_hash(), Style::default().fg(Color::Yellow)),
                Span::raw("  "),
            ];

            if highlight_query {
                let msg = &entry.message;
                let msg_lower = msg.to_lowercase();
                let highlight_style = Style::default().fg(Color::Red).add_modifier(Modifier::BOLD);
                // Collect all match ranges from all tokens, then merge overlaps
                let mut ranges: Vec<(usize, usize)> = Vec::new();
                for token in &query_tokens {
                    let mut start = 0;
                    while let Some(pos) = msg_lower[start..].find(token.as_str()) {
                        let abs = start + pos;
                        ranges.push((abs, abs + token.len()));
                        start = abs + token.len();
                    }
                }
                ranges.sort_by_key(|r| r.0);
                let mut merged: Vec<(usize, usize)> = Vec::new();
                for r in ranges {
                    if let Some(last) = merged.last_mut() {
                        if r.0 <= last.1 {
                            last.1 = last.1.max(r.1);
                            continue;
                        }
                    }
                    merged.push(r);
                }
                let mut cursor = 0;
                for (s, e) in merged {
                    if s > cursor {
                        spans.push(Span::styled(msg[cursor..s].to_string(), style));
                    }
                    spans.push(Span::styled(msg[s..e].to_string(), highlight_style));
                    cursor = e;
                }
                if cursor < msg.len() {
                    spans.push(Span::styled(msg[cursor..].to_string(), style));
                }
            } else {
                spans.push(Span::styled(&entry.message, style));
            }

            Some(ListItem::new(Line::from(spans)))
        })
        .collect();

    let list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Timeline (newest first) ")
                .border_style(Style::default().fg(Color::Cyan)),
        )
        .highlight_style(Style::default().bg(Color::DarkGray));

    f.render_stateful_widget(list, main_chunks[0], &mut app.list_state);

    // Diff preview pane
    if app.show_diff {
        let diff_area = main_chunks[1];
        app.diff_visible_height = diff_area.height.saturating_sub(2);

        if app.show_full_diff {
            let lines: Vec<Line> = if app.full_diff_content.is_empty() {
                vec![Line::raw("Loading diff...")]
            } else {
                app.full_diff_content
                    .lines()
                    .map(|line| {
                        if line.starts_with('+') && !line.starts_with("+++") {
                            Line::styled(line, Style::default().fg(Color::Green))
                        } else if line.starts_with('-') && !line.starts_with("---") {
                            Line::styled(line, Style::default().fg(Color::Red))
                        } else if line.starts_with("@@") {
                            Line::styled(line, Style::default().fg(Color::Cyan))
                        } else if line.starts_with("diff ") || line.starts_with("index ") {
                            Line::styled(line, Style::default().fg(Color::Yellow))
                        } else {
                            Line::raw(line)
                        }
                    })
                    .collect()
            };

            let diff = Paragraph::new(lines)
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .title(" Full Diff (d: back to summary | Shift+↑↓/J/K: scroll) ")
                        .border_style(Style::default().fg(Color::Cyan)),
                )
                .scroll((app.diff_scroll_offset, 0))
                .wrap(Wrap { trim: false });

            f.render_widget(diff, diff_area);
        } else {
            let diff_text = if app.diff_content.is_empty() {
                "Loading diff..."
            } else {
                &app.diff_content
            };

            let diff = Paragraph::new(diff_text)
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .title(" Diff Summary (d: full diff | Shift+↑↓/J/K: scroll) ")
                        .border_style(Style::default().fg(Color::Cyan)),
                )
                .style(Style::default().fg(Color::White))
                .scroll((app.diff_scroll_offset, 0))
                .wrap(Wrap { trim: false });

            f.render_widget(diff, diff_area);
        }
    }

    // Footer with preview or confirmation dialog
    if app.show_confirmation {
        let confirm_text =
            if let Some(entry) = app.selected_entry_idx().and_then(|i| app.entries.get(i)) {
                let mode = app.pending_restore_mode.unwrap_or(RestoreMode::HardReset);
                let note = if mode == RestoreMode::HardReset && app.has_uncommitted_changes {
                    "Hard reset discards uncommitted changes. A backup ref is created first."
                } else if mode == RestoreMode::HardReset {
                    "A backup ref is created before the hard reset."
                } else {
                    "Non-hard restore mode. Confirm only after previewing the target."
                };
                vec![
                    Line::from(vec![
                        Span::styled(
                            format!("CONFIRM {}: ", mode.label()),
                            Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
                        ),
                        Span::styled(
                            mode.command(&entry.hash),
                            Style::default()
                                .fg(Color::Yellow)
                                .add_modifier(Modifier::BOLD),
                        ),
                    ]),
                    Line::from(format!(
                        "{} - {} | {} [y/N]",
                        entry.short_hash(),
                        entry.message,
                        note
                    )),
                ]
            } else {
                vec![Line::raw("No entry selected")]
            };

        let footer = Paragraph::new(confirm_text)
            .style(Style::default().fg(Color::Red).add_modifier(Modifier::BOLD))
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Red)),
            );
        f.render_widget(footer, chunks[2]);
    } else if app.search_mode {
        let match_count = app.filtered_entries.len();
        let footer_line = Line::from(vec![
            Span::styled("🔍 Search: ", Style::default().fg(Color::Cyan)),
            Span::styled(
                app.search_query.clone(),
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled("█", Style::default().fg(Color::Yellow)),
            Span::raw(" "),
            Span::styled(
                format!("({} matches)", match_count),
                Style::default().fg(Color::Gray),
            ),
            Span::raw("  |  "),
            Span::styled("Enter: apply", Style::default().fg(Color::Green)),
            Span::raw("  |  "),
            Span::styled("Esc: cancel", Style::default().fg(Color::Red)),
            Span::raw("  |  "),
            Span::styled("?: help", Style::default().fg(Color::Cyan)),
        ]);
        let footer = Paragraph::new(footer_line).block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Yellow)),
        );
        f.render_widget(footer, chunks[2]);
    } else if app.search_active {
        let match_count = app.filtered_entries.len();
        let text = format!(
            "🔍 Filtered: {} ({} matches) | / edit | Esc clear | ? help",
            app.search_query, match_count
        );
        let footer = Paragraph::new(text)
            .style(Style::default().fg(Color::Yellow))
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Yellow)),
            );
        f.render_widget(footer, chunks[2]);
    } else {
        let entry_idx = app.selected_entry_idx().unwrap_or(0);
        let footer_text = if let Some(message) = &app.status_message {
            message.clone()
        } else if let Some(entry) = app.entries.get(entry_idx) {
            format!(
                "📍 Target: {} - {} | Enter hard | s soft | c checkout | y copy | / search | ? help",
                entry.short_hash(),
                entry.message
            )
        } else {
            "No entries found".to_string()
        };

        let footer = Paragraph::new(footer_text)
            .style(Style::default().fg(Color::Green))
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Cyan)),
            );
        f.render_widget(footer, chunks[2]);
    }

    if app.show_help {
        render_help_overlay(f, app);
    }
}

fn render_help_overlay(f: &mut Frame, app: &App) {
    let area = centered_rect(76, 72, f.area());
    let help = Paragraph::new(help_lines(app))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Help (?/Esc/q to close) ")
                .border_style(Style::default().fg(Color::Cyan)),
        )
        .wrap(Wrap { trim: false });

    f.render_widget(Clear, area);
    f.render_widget(help, area);
}

fn centered_rect(percent_x: u16, percent_y: u16, area: Rect) -> Rect {
    let vertical = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(area);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(vertical[1])[1]
}

fn help_lines(app: &App) -> Vec<Line<'static>> {
    let context = if app.show_confirmation {
        "Confirm restore"
    } else if app.search_mode {
        "Search"
    } else if app.search_active {
        "Filtered timeline"
    } else if app.show_full_diff {
        "Full diff preview"
    } else if app.show_diff {
        "Diff summary"
    } else {
        "Timeline"
    };

    vec![
        Line::from(vec![
            Span::styled("Context: ", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw(context),
        ]),
        Line::raw(""),
        Line::styled("Navigation", Style::default().fg(Color::Cyan)),
        Line::raw("  ↑/k, ↓/j        Move selection"),
        Line::raw("  Home/End         Jump to first or last entry"),
        Line::raw("  gg/G             Vim-style first or last entry"),
        Line::raw("  PgUp/PgDn        Jump 10 entries"),
        Line::raw(""),
        Line::styled("Inspect", Style::default().fg(Color::Cyan)),
        Line::raw("  Space            Toggle diff panel"),
        Line::raw("  d                Switch diff summary/full diff"),
        Line::raw("  Shift+↑↓ or J/K  Scroll diff"),
        Line::raw("  t                Toggle relative/absolute timestamps"),
        Line::raw("  /                Search message, hash, author, or time"),
        Line::raw(""),
        Line::styled("Recover", Style::default().fg(Color::Cyan)),
        Line::raw("  Enter            Hard reset after confirmation"),
        Line::raw("  s                Soft reset after confirmation"),
        Line::raw("  c                Checkout selected commit detached"),
        Line::raw("  y                Copy selected commit hash"),
        Line::raw(""),
        Line::styled("Safety", Style::default().fg(Color::Yellow)),
        Line::raw("  Hard reset creates a backup ref first."),
        Line::raw("  Preview the diff and exact command before confirming a restore."),
        Line::raw("  Reflog recovery only works for history Git can still see locally."),
        Line::raw(""),
        Line::raw(
            "Esc clears an active filter, closes help, or quits from the unfiltered timeline.",
        ),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    fn entry() -> GitEntry {
        GitEntry {
            hash: "abcdef1234567890".to_string(),
            message: "rebase finished: returning to refs/heads/feature".to_string(),
            timestamp: Utc::now(),
            author: "Test User".to_string(),
            relative_time: "2h ago".to_string(),
        }
    }

    #[test]
    fn query_matches_message_hash_author_and_time() {
        let entry = entry();

        assert!(entry_matches_query(&entry, &["rebase", "feature"]));
        assert!(entry_matches_query(&entry, &["abcdef1"]));
        assert!(entry_matches_query(&entry, &["test", "user"]));
        assert!(entry_matches_query(&entry, &["2h"]));
        assert!(!entry_matches_query(&entry, &["stash"]));
    }

    #[test]
    fn help_mentions_copy_binding() {
        let lines = help_lines(&App {
            git_manager: GitManager::for_test(),
            entries: vec![entry()],
            list_state: ListState::default(),
            show_confirmation: false,
            show_diff: false,
            show_full_diff: false,
            diff_content: String::new(),
            full_diff_content: String::new(),
            diff_scroll_offset: 0,
            diff_visible_height: 10,
            has_uncommitted_changes: false,
            search_mode: false,
            search_query: String::new(),
            filtered_entries: vec![0],
            search_active: false,
            show_absolute_time: false,
            last_key_was_g: false,
            pending_restore_mode: None,
            show_help: true,
            status_message: None,
        });
        let help_text = format!("{lines:?}");

        assert!(help_text.contains("Copy selected commit hash"));
    }

    #[test]
    fn clipboard_has_platform_candidate() {
        assert!(!clipboard_commands().is_empty());
    }
}
