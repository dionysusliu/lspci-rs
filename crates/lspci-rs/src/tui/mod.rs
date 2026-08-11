mod styled;
mod tree;
mod ui;

use std::io::IsTerminal;

use crossterm::ExecutableCommand;
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind};
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use ratatui::text::Text;

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
    pub detail: Text<'static>,
    pub detail_scroll: u16,
    pub mode: Mode,
    pub filter_input: String,
    pub color: ColorMode,
}

impl App {
    fn new(session: PciSession, model: tree::TreeModel, color: ColorMode) -> App {
        let visible = model.visible_rows();
        let first_device = visible
            .iter()
            .position(|row| model.rows[*row].address.is_some())
            .unwrap_or(0);
        let mut app = App {
            session,
            model,
            visible,
            cursor: first_device,
            tree_offset: 0,
            detail: Text::default(),
            detail_scroll: 0,
            mode: Mode::Normal,
            filter_input: String::new(),
            color,
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
            self.detail = Text::default();
            return;
        };
        let text = crate::render_device_detail(&mut self.session, address, None, self.color)
            .unwrap_or_else(|error| format!("failed to load details: {error}"));
        self.detail = styled::text_from_ansi(&text);
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

    pub fn handle_key(&mut self, key: &KeyEvent) -> Flow {
        if key
            .modifiers
            .contains(crossterm::event::KeyModifiers::CONTROL)
            && matches!(key.code, KeyCode::Char('c') | KeyCode::Char('q'))
        {
            return Flow::Quit;
        }
        match self.mode {
            Mode::Normal => self.handle_normal(key.code),
            Mode::Filter => self.handle_filter(key.code),
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
                let lines = self.detail.lines.len() as u16;
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

pub fn run_tui(color: ColorMode) -> Result<(), Box<dyn std::error::Error>> {
    if !std::io::stdout().is_terminal() {
        return Err("tui requires an interactive terminal".into());
    }

    let mut session = PciSession::new()?;
    let snapshot = session.scan()?;
    let windows = collect_bridge_windows(&mut session, &snapshot);
    let model = tree::TreeModel::build(&snapshot, &windows);
    let mut app = App::new(session, model, color);

    enable_raw_mode()?;
    let _guard = TerminalGuard;
    std::io::stdout().execute(EnterAlternateScreen)?;

    let mut terminal = Terminal::new(CrosstermBackend::new(std::io::stdout()))?;

    loop {
        terminal.draw(|frame| ui::draw(frame, &mut app))?;
        let Event::Key(key) = event::read()? else {
            continue;
        };
        if key.kind != KeyEventKind::Press {
            continue;
        }
        if app.handle_key(&key) == Flow::Quit {
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
