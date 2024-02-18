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

    let mut backend = Backend::new();
    let (event_rx, timer_tx, timer_rx) = backend.init();
    let app = App::new(&PALETTES[8]);

    app.run(&mut terminal, backend.data, event_rx, timer_tx, timer_rx)
        .unwrap();

    disable_raw_mode()?;
    stdout().execute(LeaveAlternateScreen)?;

    Ok(())
}
