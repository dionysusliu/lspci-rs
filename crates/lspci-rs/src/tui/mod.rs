mod tree;
mod ui;

use std::io::IsTerminal;

use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use crossterm::ExecutableCommand;
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;

use pci::PciSession;

use crate::color::ColorMode;
use crate::tree::collect_bridge_windows;

#[derive(PartialEq)]
pub enum Flow {
    Continue,
    Quit,
}

pub enum Mode {
    Normal,
    Filter,
}

pub struct App {
    session: PciSession,
    pub model: tree::TreeModel,
    pub visible: Vec<usize>,
    pub cursor: usize,
    pub tree_offset: usize,
    pub detail: String,
    pub detail_scroll: u16,
    pub mode: Mode,
    pub filter_input: String,
}

impl App {
    fn new(session: PciSession, model: tree::TreeModel) -> App {
        let visible = model.visible_rows();
        let mut app = App {
            session,
            model,
            visible,
            cursor: 0,
            tree_offset: 0,
            detail: String::new(),
            detail_scroll: 0,
            mode: Mode::Normal,
            filter_input: String::new(),
        };
        app.load_detail();
        app
    }

    fn refresh(&mut self) {
        self.visible = self.model.visible_rows();
        if self.cursor >= self.visible.len() {
            self.cursor = self.visible.len().saturating_sub(1);
        }
        self.load_detail();
    }

    fn load_detail(&mut self) {
        self.detail_scroll = 0;
        let address = self
            .visible
            .get(self.cursor)
            .and_then(|row| self.model.rows[*row].address);
        let Some(address) = address else {
            self.detail = String::new();
            return;
        };
        self.detail =
            crate::render_device_detail(&mut self.session, address, None, ColorMode::Never)
                .unwrap_or_else(|error| format!("failed to load details: {error}"));
    }

    fn move_cursor(&mut self, delta: isize) {
        if self.visible.is_empty() {
            return;
        }
        let next = (self.cursor as isize + delta).clamp(0, self.visible.len() as isize - 1);
        if next as usize != self.cursor {
            self.cursor = next as usize;
            self.load_detail();
        }
    }

    pub fn handle_key(&mut self, key: KeyCode) -> Flow {
        match self.mode {
            Mode::Normal => self.handle_normal(key),
            Mode::Filter => self.handle_filter(key),
        }
    }

    fn handle_normal(&mut self, key: KeyCode) -> Flow {
        match key {
            KeyCode::Char('q') | KeyCode::Esc => return Flow::Quit,
            KeyCode::Char('j') | KeyCode::Down => self.move_cursor(1),
            KeyCode::Char('k') | KeyCode::Up => self.move_cursor(-1),
            KeyCode::Char('l') | KeyCode::Right | KeyCode::Enter | KeyCode::Char(' ') => {
                if let Some(row) = self.visible.get(self.cursor).copied() {
                    self.model.expand(row);
                    self.refresh();
                }
            }
            KeyCode::Char('h') | KeyCode::Left => {
                if let Some(row) = self.visible.get(self.cursor).copied() {
                    if self.model.is_expanded(row) {
                        self.model.collapse(row);
                        self.refresh();
                    } else if let Some(parent) = self.model.parent(row) {
                        if let Some(position) = self.visible.iter().position(|r| *r == parent) {
                            self.cursor = position;
                            self.load_detail();
                        }
                    }
                }
            }
            KeyCode::PageUp => {
                self.detail_scroll = self.detail_scroll.saturating_sub(10);
            }
            KeyCode::PageDown => {
                let lines = self.detail.lines().count() as u16;
                self.detail_scroll = (self.detail_scroll + 10).min(lines.saturating_sub(1));
            }
            KeyCode::Char('/') => {
                self.mode = Mode::Filter;
                self.filter_input = self.model.filter.clone();
            }
            _ => {}
        }
        Flow::Continue
    }

    fn handle_filter(&mut self, key: KeyCode) -> Flow {
        match key {
            KeyCode::Esc => {
                self.filter_input.clear();
                self.model.filter.clear();
                self.mode = Mode::Normal;
                self.cursor = 0;
                self.tree_offset = 0;
                self.refresh();
            }
            KeyCode::Enter => {
                self.mode = Mode::Normal;
            }
            KeyCode::Backspace => {
                self.filter_input.pop();
                self.model.filter = self.filter_input.clone();
                self.cursor = 0;
                self.tree_offset = 0;
                self.refresh();
            }
            KeyCode::Char(character) => {
                self.filter_input.push(character);
                self.model.filter = self.filter_input.clone();
                self.cursor = 0;
                self.tree_offset = 0;
                self.refresh();
            }
            _ => {}
        }
        Flow::Continue
    }
}

pub fn run_tui() -> Result<(), Box<dyn std::error::Error>> {
    if !std::io::stdout().is_terminal() {
        return Err("tui requires an interactive terminal".into());
    }

    let mut session = PciSession::new()?;
    let snapshot = session.scan()?;
    let windows = collect_bridge_windows(&mut session, &snapshot);
    let model = tree::TreeModel::build(&snapshot, &windows);
    let mut app = App::new(session, model);

    enable_raw_mode()?;
    std::io::stdout().execute(EnterAlternateScreen)?;
    let _guard = TerminalGuard;

    let mut terminal = Terminal::new(CrosstermBackend::new(std::io::stdout()))?;

    loop {
        terminal.draw(|frame| ui::draw(frame, &mut app))?;
        let Event::Key(key) = event::read()? else {
            continue;
        };
        if key.kind != KeyEventKind::Press {
            continue;
        }
        if app.handle_key(key.code) == Flow::Quit {
            break;
        }
    }

    Ok(())
}

struct TerminalGuard;

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
        let _ = std::io::stdout().execute(LeaveAlternateScreen);
    }
}
