use ratatui::widgets::{Block, Paragraph};
use ratatui::Frame;

pub fn draw(frame: &mut Frame) {
    let paragraph = Paragraph::new("lspci-rs tui — press q to quit")
        .block(Block::bordered().title("lspci-rs"));
    frame.render_widget(paragraph, frame.area());
}
