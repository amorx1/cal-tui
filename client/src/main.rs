use std::{
    collections::BTreeMap,
    io::{self, stdout},
    sync::mpsc::{channel, Receiver},
    time::Duration,
};

use chrono::{prelude::*, Days, Duration as ChronoDuration};

use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind},
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
    ExecutableCommand,
};
use ratatui::{
    prelude::*,
    widgets::{Cell, Row, Table, TableState},
};

use reqwest::Client;
use tokio::runtime;

use dotenv::dotenv;

mod outlook;
use outlook::refresh;

mod auth;
use auth::start_server_main;
use style::palette::tailwind;

const PALETTES: [tailwind::Palette; 6] = [
    tailwind::BLUE,
    tailwind::EMERALD,
    tailwind::INDIGO,
    tailwind::RED,
    tailwind::AMBER,
    tailwind::ROSE,
];

#[derive(Default, Clone)]
struct CalendarEvent {
    end_time: DateTime<Utc>,
    start_time: DateTime<Utc>,
    subject: String,
}

struct TableColors {
    buffer_bg: Color,
    header_bg: Color,
    header_fg: Color,
    row_fg: Color,
    selected_style_fg: Color,
    normal_row_color: Color,
    alt_row_color: Color,
    footer_border_color: Color,
}

impl TableColors {
    fn new(color: &tailwind::Palette) -> Self {
        Self {
            buffer_bg: color.c950,
            header_bg: color.c900,
            header_fg: color.c200,
            row_fg: color.c200,
            selected_style_fg: color.c400,
            normal_row_color: color.c950,
            alt_row_color: color.c900,
            footer_border_color: color.c400,
        }
    }
}

struct App {
    state: TableState,
    events: BTreeMap<DateTime<Utc>, CalendarEvent>,
    colors: TableColors,
}

impl App {
    pub fn next(&mut self) {
        let i = match self.state.selected() {
            Some(i) => {
                if i >= self.events.len() - 1 {
                    0
                } else {
                    i + 1
                }
            }
            None => 0,
        };
        self.state.select(Some(i));
    }

    pub fn previous(&mut self) {
        let i = match self.state.selected() {
            Some(i) => {
                if i == 0 {
                    self.events.len() - 1
                } else {
                    i - 1
                }
            }
            None => 0,
        };
        self.state.select(Some(i));
    }
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
        .build()?;

    let outlook_thread = runtime::Builder::new_multi_thread()
        .worker_threads(1)
        .thread_name("outlook")
        .enable_all()
        .build()?;

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
        start.date_naive(),
        start.time().to_string().rsplit_once(':').unwrap().0,
    );
    let end_arg = format!(
        "{}T{}",
        end.date_naive(),
        start.time().to_string().rsplit_once(':').unwrap().0,
    );

    // App
    let app = App {
        events: BTreeMap::new(),
        colors: TableColors::new(&PALETTES[5]),
        state: TableState::default().with_selected(0),
    };

    let (tx_event, rx_event) = channel();
    let client = Client::new();
    outlook_thread.spawn(async move { refresh(token, start_arg, end_arg, client, tx_event).await });

    run_app(&mut terminal, app, rx_event).unwrap();

    disable_raw_mode()?;
    stdout().execute(LeaveAlternateScreen)?;

    Ok(())
}

fn run_app<B: Backend>(
    terminal: &mut Terminal<B>,
    mut app: App,
    rx: Receiver<CalendarEvent>,
) -> io::Result<()> {
    loop {
        terminal.draw(|f| ui(f, &mut app))?;

        if let Ok(true) = event::poll(Duration::from_millis(50)) {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    match key.code {
                        KeyCode::Char('q') => return Ok(()),
                        KeyCode::Char('j') | KeyCode::Down => app.next(),
                        KeyCode::Char('k') | KeyCode::Up => app.previous(),
                        _ => (),
                    }
                }
            }
        }

        while let Some(event) = rx.try_iter().next() {
            app.events.insert(event.start_time, event);
        }
    }
}

fn ui(frame: &mut Frame, app: &mut App) {
    let layout = Layout::horizontal([Constraint::Percentage(100)])
        .flex(layout::Flex::SpaceBetween)
        .split(frame.size());

    let header_style = Style::default()
        .fg(app.colors.header_fg)
        .bg(app.colors.header_bg);
    let selected_style = Style::default()
        .add_modifier(Modifier::REVERSED)
        .fg(app.colors.selected_style_fg);
    let header = [
        Text::from("Event")
            .style(Style::default().bold())
            .alignment(Alignment::Center),
        Text::from("Start Time")
            .style(Style::default().bold())
            .alignment(Alignment::Center),
        Text::from("Duration")
            .style(Style::default().bold())
            .alignment(Alignment::Center),
    ]
    .iter()
    .cloned()
    .map(Cell::from)
    .collect::<Row>()
    .style(header_style)
    .height(2);

    let rows = app.events.iter().enumerate().map(|(i, (time, e))| {
        let color = match i % 2 {
            0 => app.colors.normal_row_color,
            _ => app.colors.alt_row_color,
        };

        let duration = &e.end_time.signed_duration_since(time).num_minutes();
        let subject = e.subject.clone();
        let (date, time) = reformat_time(time);

        Row::new(vec![
            Text::from(subject)
                .style(Style::default().bold())
                .alignment(Alignment::Center),
            Text::from(format!("{date:?} @ {time:?}")).alignment(Alignment::Center),
            Text::from(format!("{duration:?} mins")).alignment(Alignment::Center),
        ])
        .style(Style::new().fg(app.colors.row_fg).bg(color))
        .height(4)
    });

    let widths = [
        Constraint::Length(100),
        Constraint::Length(100),
        Constraint::Length(100),
    ];
    let table = Table::new(rows, widths)
        .header(header)
        .bg(app.colors.buffer_bg)
        .highlight_style(selected_style);

    frame.render_stateful_widget(table, layout[0], &mut app.state);
}

fn reformat_time(dt: &DateTime<Utc>) -> (String, String) {
    let mut day = String::new();
    dt.date_naive()
        .to_string()
        .split('-')
        .rev()
        .for_each(|p| day.push_str(format!("{}-", p).as_ref()));

    let time = dt
        .time()
        .overflowing_add_signed(ChronoDuration::hours(13))
        .0
        .to_string();

    (day.trim_end_matches('-').to_string(), time)
}
