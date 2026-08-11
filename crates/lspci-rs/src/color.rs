use clap::ValueEnum;
use std::io::IsTerminal;

#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum ColorMode {
    Auto,
    Always,
    Never,
}

impl ColorMode {
    pub fn enabled(self) -> bool {
        match self {
            ColorMode::Auto => std::io::stdout().is_terminal(),
            ColorMode::Always => true,
            ColorMode::Never => false,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Palette {
    pub enabled: bool,
}

impl Palette {
    pub fn new(mode: ColorMode) -> Self {
        Self {
            enabled: mode.enabled(),
        }
    }

    fn paint(&self, code: &str, text: &str) -> String {
        if self.enabled {
            format!("\x1b[{code}m{text}\x1b[0m")
        } else {
            text.to_owned()
        }
    }

    /// device addresses — cyan bold
    pub fn address(&self, text: &str) -> String {
        self.paint("1;36", text)
    }

    /// field labels, IDs, offsets, disabled markers — dim
    pub fn dim(&self, text: &str) -> String {
        self.paint("2", text)
    }

    /// unavailable / failed values — red
    pub fn unavailable(&self, text: &str) -> String {
        self.paint("31", text)
    }

    /// capability names — green
    pub fn capability(&self, text: &str) -> String {
        self.paint("32", text)
    }
}
