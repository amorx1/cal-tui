use chrono::{prelude::*, Days, Duration as ChronoDuration};
use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind},
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
    ExecutableCommand,
};
use dotenv::dotenv;
use ratatui::{
    backend::{Backend, CrosstermBackend},
    widgets::TableState,
    Frame, Terminal,
};
use reqwest::Client;
use std::{
    collections::BTreeMap,
    io::{self, stdout},
    sync::mpsc::{channel, Receiver},
    time::Duration,
};
use tokio::{runtime, time::sleep};
mod outlook;
use outlook::{refresh, EventCommand};
mod auth;
use auth::start_server_main;
mod app;
use app::{App, Focus};
mod ui;
use ui::{render_popup, render_selection, render_table, TableColors, PALETTES};

fn main() -> io::Result<()> {
    dotenv().ok();

    enable_raw_mode()?;
    stdout().execute(EnterAlternateScreen)?;
    let mut terminal = Terminal::new(CrosstermBackend::new(stdout()))?;

    let server_thread = runtime::Builder::new_multi_thread()
        .worker_threads(1)
        .thread_name("warp")
        .enable_all()
        .build()?;

    let outlook_thread = runtime::Builder::new_multi_thread()
        .worker_threads(1)
        .thread_name("outlook")
        .enable_all()
        .build()?;

    // Authentication
    let (auth_tx, auth_rx) = channel();
    server_thread.spawn(async move { start_server_main(auth_tx).await });
    let token = auth_rx
        .recv_timeout(Duration::from_millis(10000))
        .expect("ERROR: Unsuccessful authentication!");

    let start = Utc::now();
    let end = start.checked_add_days(Days::new(7)).unwrap();

    let start_arg = format!(
        "{}T{}",
        start.date_naive(),
        start.time().to_string().rsplit_once(':').unwrap().0
    );
    let end_arg = format!(
        "{}T{}",
        end.date_naive(),
        start.time().to_string().rsplit_once(':').unwrap().0,
    );

    let app = App {
        events: BTreeMap::new(),
        colors: TableColors::new(&PALETTES[8]),
        state: TableState::default().with_selected(0),
        focus: Focus::Table,
    };

    // Email refresh thread
    let (event_tx, event_rx) = channel();
    let client = Client::new();
    outlook_thread.spawn(async move { refresh(token, start_arg, end_arg, client, event_tx).await });

    // Entrypoint
    run_app(&mut terminal, app, event_rx).unwrap();

    disable_raw_mode()?;
    stdout().execute(LeaveAlternateScreen)?;

    Ok(())
}

fn run_app<B: Backend>(
    terminal: &mut Terminal<B>,
    mut app: App,
    event_rx: Receiver<EventCommand>,
) -> io::Result<()> {
    // Separate runtime for timer thread.
    let timer_thread = runtime::Builder::new_multi_thread()
        .worker_threads(1)
        .thread_name("timer")
        .enable_all()
        .build()?;

    // Timer notifications contain no messages -> ()
    let (timer_tx, timer_rx) = channel::<()>();

    loop {
        terminal.draw(|f| ui(f, &mut app))?;

        // Manual event handlers.
        if let Ok(true) = event::poll(Duration::from_millis(50)) {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    match key.code {
                        KeyCode::Char('q') => return Ok(()),
                        KeyCode::Char('j') | KeyCode::Down => match app.focus {
                            Focus::Table => app.next(),
                            _ => {}
                        },
                        KeyCode::Char('k') | KeyCode::Up => match app.focus {
                            Focus::Table => app.previous(),
                            _ => {}
                        },
                        KeyCode::Enter => {
                            app.focus = match app.focus {
                                Focus::Table => Focus::Selected,
                                Focus::Selected => Focus::Table,
                                Focus::Popup => Focus::Table,
                            }
                        }
                        _ => (),
                    }
                }
            }
        }

        // Listen for new events from refresh thread.
        while let Some(command) = event_rx.try_iter().next() {
            if let EventCommand::Add(event) = command {
                let eta = event
                    .start_time
                    .checked_sub_signed(ChronoDuration::minutes(2)) // TODO: Make reminder offset configurable
                    .map(|x| x.signed_duration_since(Utc::now()).num_milliseconds())
                    .unwrap();

                // None -> event didn't already exist, so it is safe to create a new timer for it without duplicating.
                if app.events.insert(event.start_time, event).is_none() {
                    let timer_tx = timer_tx.clone();
                    timer_thread.spawn(async move {
                        sleep(Duration::from_millis(eta as u64)).await;
                        timer_tx
                            .send(())
                            .expect("ERROR: Could not send timer notification");
                    });
                }
            }
        }

        // A timeout notification has been received, meaning an alert should be displayed.
        if timer_rx.try_recv().is_ok() {
            app.popup();
        }

        // Clear past events
        app.events.retain(|_, event| event.end_time >= Utc::now());
    }
}

/* Render UI components */
fn ui(frame: &mut Frame, app: &mut App) {
    let area = frame.size();

    match app.focus {
        // Alert for upcoming event
        Focus::Popup => {
            render_popup(app, frame, area);
        }
        // Detailed view for selected event
        Focus::Selected => {
            render_selection(app, frame, area);
        }
        // Table of upcoming events
        Focus::Table => {
            render_table(app, frame, area);
        }
    }
}
