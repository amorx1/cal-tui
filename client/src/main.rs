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
mod ui;
use ui::PALETTES;
mod backend;
use backend::*;

fn main() -> io::Result<()> {
    dotenv().ok();
    enable_raw_mode()?;
    stdout().execute(EnterAlternateScreen)?;

    let mut terminal = Terminal::new(CrosstermBackend::new(stdout()))?;
    let backend = Backend::new();
    let app = App::new(&PALETTES[8], backend);
    app.run(&mut terminal).unwrap();

    disable_raw_mode()?;
    stdout().execute(LeaveAlternateScreen)?;

    Ok(())
}
