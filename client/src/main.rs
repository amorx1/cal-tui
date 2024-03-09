use crossterm::{
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
    ExecutableCommand,
};
use ratatui::{backend::CrosstermBackend, Terminal};

use std::{
    io::{self, stdout},
    sync::OnceLock,
};

mod app;
mod auth;
mod outlook;
use app::App;
mod backend;
mod ui;
use backend::*;

use crate::app::Config;

// TODO: Enumerate possibilities in README
static CONFIG_PATH: &str = "$HOME/.config/cal-tui/config.toml";
static CONFIG: OnceLock<Config> = OnceLock::new();

fn main() -> io::Result<()> {
    CONFIG.get_or_init(Config::from_path);

    enable_raw_mode()?;
    stdout().execute(EnterAlternateScreen)?;

    let mut terminal = Terminal::new(CrosstermBackend::new(stdout()))?;
    let backend = Backend::new();
    let app = App::new(backend);

    app.run(&mut terminal).unwrap();

    disable_raw_mode()?;
    stdout().execute(LeaveAlternateScreen)?;

    Ok(())
}
