use std::{
    collections::BTreeMap,
    io::{self, stdout},
    sync::mpsc::{channel, Receiver, Sender},
    time::Duration,
};

use chrono::{prelude::*, Days};

use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind},
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
    ExecutableCommand,
};
use ratatui::{
    prelude::*,
    widgets::{Block, Borders, Row, Table},
};

use reqwest::Client;
use tokio::runtime;

use dotenv::dotenv;

mod outlook;
use outlook::Root;

mod auth;
use auth::start_server_main;

#[derive(Default, Clone)]
struct CalendarEvent {
    end_time: DateTime<Utc>,
    start_time: DateTime<Utc>,
    subject: String,
}

struct App {
    events: BTreeMap<DateTime<Utc>, CalendarEvent>,
    show_table: bool,
}

fn main() -> io::Result<()> {
    dotenv().ok();

    enable_raw_mode()?;
    stdout().execute(EnterAlternateScreen)?;
    let mut terminal = Terminal::new(CrosstermBackend::new(stdout()))?;

    let server_thread = runtime::Builder::new_multi_thread()
        .worker_threads(1)
        .thread_name("warp")
        .enable_all()
        .build()
        .unwrap();

    let outlook_thread = runtime::Builder::new_multi_thread()
        .worker_threads(1)
        .thread_name("outlook")
        .enable_all()
        .build()
        .unwrap();

    // Authentication
    let (tx, rx) = channel();
    server_thread.spawn(async move { start_server_main(tx).await });
    let token = rx
        .recv_timeout(Duration::from_millis(10000))
        .expect("ERROR: Unsuccessful authentication!");

    let start = Utc::now();
    let end = start.checked_add_days(Days::new(7)).unwrap();

    let start_arg = format!(
        "{}T{}",
        start.date_naive().to_string(),
        start.time().to_string().rsplit_once(':').unwrap().0,
    );
    let end_arg = format!(
        "{}T{}",
        end.date_naive().to_string(),
        start.time().to_string().rsplit_once(':').unwrap().0,
    );

    // App
    let app = App {
        show_table: false,
        events: BTreeMap::new(),
    };

    let (tx_event, rx_event) = channel();
    let client = Client::new();
    outlook_thread.spawn(async move { refresh(token, start_arg, end_arg, client, tx_event).await });

    run_app(&mut terminal, app, rx_event).unwrap();

    disable_raw_mode()?;
    stdout().execute(LeaveAlternateScreen)?;
    Ok(())
}

async fn refresh(
    token: String,
    start: String,
    end: String,
    client: Client,
    tx: Sender<CalendarEvent>,
) {
    loop {
        let url = format!(
            "https://graph.microsoft.com/v1.0/me/calendarView?startDateTime={}&endDateTime={}",
            start, end
        );

        if Utc::now().second() % 10 == 0 {
            // refresh
            let response = client
                .get(url)
                .header("Authorization", format!("Bearer {}", token))
                .send()
                .await
                .unwrap()
                .json::<Root>()
                .await
                .unwrap();

            let calendar_events = response
                .value
                .iter()
                .map(|v| {
                    let start_time_string =
                        String::from(format!("{}+0000", v.start.date_time.clone().unwrap()));
                    let start_time =
                        DateTime::parse_from_str(&start_time_string, "%Y-%m-%dT%H:%M:%S%.f%z")
                            .ok()
                            .and_then(|dt| Some(dt.with_timezone(&Utc::now().timezone())))
                            .unwrap();
                    let end_time_string =
                        String::from(format!("{}+0000", v.end.date_time.clone().unwrap()));
                    let end_time =
                        DateTime::parse_from_str(&end_time_string, "%Y-%m-%dT%H:%M:%S%.f%z")
                            .ok()
                            .and_then(|dt| Some(dt.with_timezone(&Utc::now().timezone())))
                            .unwrap();

                    CalendarEvent {
                        start_time,
                        end_time,
                        subject: v.subject.clone().unwrap(),
                    }
                })
                .filter(|e| e.start_time > Utc::now());

            for event in calendar_events {
                tx.send(event)
                    .expect("ERROR: Could not send to main thread");
            }
        };
    }
}

fn run_app<B: Backend>(
    terminal: &mut Terminal<B>,
    mut app: App,
    rx: Receiver<CalendarEvent>,
) -> io::Result<()> {
    loop {
        terminal.draw(|f| ui(f, &app))?;

        match event::poll(Duration::from_millis(50)) {
            Ok(true) => match event::read()? {
                Event::Key(key) => {
                    if key.kind == KeyEventKind::Press {
                        match key.code {
                            KeyCode::Char('q') => return Ok(()),
                            KeyCode::Char('p') => app.show_table = !app.show_table,
                            _ => {}
                        }
                    }
                }
                _ => {}
            },
            _ => {}
        }

        while let Some(event) = rx.try_iter().next() {
            app.events.insert(event.start_time.clone(), event);
        }
    }
}

fn ui(frame: &mut Frame, app: &App) {
    let layout = Layout::horizontal([Constraint::Percentage(100)])
        .flex(layout::Flex::SpaceBetween)
        .split(frame.size());

    let rows = app
        .events
        .iter()
        .map(|(time, e)| Row::new(vec![time.to_string(), e.subject.clone()]));

    let widths = [Constraint::Length(100), Constraint::Length(100)];
    let table = Table::new(rows, widths);
    let block = Block::default().title("Events").borders(Borders::ALL);
    frame.render_widget(table.block(block), layout[0]);
}
