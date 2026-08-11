mod ui;

use std::io::IsTerminal;

use crossterm::ExecutableCommand;
use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;

use pci::PciSession;

use crate::tree::collect_bridge_windows;

pub fn run_tui() -> Result<(), Box<dyn std::error::Error>> {
    if !std::io::stdout().is_terminal() {
        return Err("tui requires an interactive terminal".into());
    }

    let mut session = PciSession::new()?;
    let snapshot = session.scan()?;
    let _windows = collect_bridge_windows(&mut session, &snapshot);

    enable_raw_mode()?;
    std::io::stdout().execute(EnterAlternateScreen)?;
    let _guard = TerminalGuard;

    let mut terminal = Terminal::new(CrosstermBackend::new(std::io::stdout()))?;

    loop {
        terminal.draw(ui::draw)?;
        let Event::Key(key) = event::read()? else {
            continue;
        };
        if key.kind != KeyEventKind::Press {
            continue;
        }
        if key.code == KeyCode::Char('q') || key.code == KeyCode::Esc {
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
