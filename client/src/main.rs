use crossterm::{
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
    ExecutableCommand,
};
use dotenv::dotenv;
use ratatui::{backend::CrosstermBackend, Terminal};

use std::io::{self, stdout};

mod app;
mod auth;
mod outlook;
use app::App;
mod backend;
mod ui;
use backend::*;

// TODO: Enumerate possibilities in README
static THEME: usize = 8;

fn main() -> io::Result<()> {
    dotenv().ok();
    enable_raw_mode()?;
    stdout().execute(EnterAlternateScreen)?;

    let mut terminal = Terminal::new(CrosstermBackend::new(stdout()))?;
    let backend = Backend::new();
    let app = App::new(THEME, backend);

    app.run(&mut terminal).unwrap();

    disable_raw_mode()?;
    stdout().execute(LeaveAlternateScreen)?;

    Ok(())
}
