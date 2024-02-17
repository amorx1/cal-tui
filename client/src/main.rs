use chrono::{prelude::*, Days, Duration as ChronoDuration};
use std::{
    collections::BTreeMap,
    io::{self, stdout},
    process::Command,
    sync::mpsc::{channel, Receiver},
    time::Duration,
};

use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind},
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
    ExecutableCommand,
};

use ratatui::{
    prelude::*,
    widgets::{Block, Borders, Cell, Clear, Paragraph, Row, Table, TableState},
};

use reqwest::Client;
use tokio::{runtime, time::sleep};

use dotenv::dotenv;

mod outlook;
use outlook::{refresh, TeamsMeeting};

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

enum EventCommand {
    Add(CalendarEvent),
    Remove(CalendarEvent),
}

#[derive(Clone, Copy)]
enum Focus {
    Table,
    Selected,
    Popup,
}

#[derive(Debug, Default)]
struct CalendarEvent {
    id: String,
    is_cancelled: bool,
    end_time: DateTime<Utc>,
    start_time: DateTime<Utc>,
    organizer: String,
    subject: String,
    teams_meeting: Option<TeamsMeeting>,
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
    focus: Focus,
    events: BTreeMap<DateTime<Utc>, CalendarEvent>,
    colors: TableColors,
}

impl App {
    pub fn popup(&mut self) {
        self.focus = Focus::Popup;
        _ = Command::new("zellij")
            .args(["action", "toggle-floating-panes"])
            .status()
            .expect("ERROR: Could not send command to Zellij");
    }
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

    // App
    let app = App {
        events: BTreeMap::new(),
        colors: TableColors::new(&PALETTES[2]),
        state: TableState::default().with_selected(0),
        focus: Focus::Table,
    };

    let (event_tx, event_rx) = channel();
    let client = Client::new();
    outlook_thread.spawn(async move { refresh(token, start_arg, end_arg, client, event_tx).await });

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
    let timer_thread = runtime::Builder::new_multi_thread()
        .worker_threads(1)
        .thread_name("timer")
        .enable_all()
        .build()?;

    let (timer_tx, timer_rx) = channel::<bool>();

    loop {
        terminal.draw(|f| ui(f, &mut app))?;

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

        while let Some(command) = event_rx.try_iter().next() {
            if let EventCommand::Add(event) = command {
                let eta = event
                    .start_time
                    .checked_sub_signed(ChronoDuration::minutes(2))
                    .map(|x| x.signed_duration_since(Utc::now()).num_milliseconds())
                    .unwrap();
                if app.events.insert(event.start_time, event).is_none() {
                    let timer_tx = timer_tx.clone();
                    timer_thread.spawn(async move {
                        sleep(Duration::from_millis(eta as u64)).await;
                        timer_tx
                            .send(true)
                            .expect("ERROR: Could not send timer notification");
                    });
                }
            }
        }

        if timer_rx.try_recv().is_ok() {
            app.popup();
        }

        app.events.retain(|_, event| event.end_time >= Utc::now());
    }
}

fn ui(frame: &mut Frame, app: &mut App) {
    let area = frame.size();

    match app.focus {
        Focus::Popup => {
            let block = Block::default().title("Event").borders(Borders::ALL);
            let text = app
                .events
                .first_key_value()
                .map_or(Paragraph::new(""), |(_, event)| {
                    Paragraph::new(Text::styled(
                        format!("{}\n{}", event.subject, event.organizer,),
                        Style::default().fg(Color::Red).bold(),
                    ))
                });

            let inner_area = centered_rect(60, 20, area);
            frame.render_widget(Clear, area); //this clears out the background
            frame.render_widget(Block::default().bg(Color::LightRed), area);
            frame.render_widget(text.block(block).on_black(), inner_area);
        }
        Focus::Selected => {
            let block = Block::default().title("Event").borders(Borders::ALL);
            if let Some(i) = app.state.selected() {
                let text = app
                    .events
                    .iter()
                    .nth(i)
                    .map_or(Paragraph::new(""), |(_, event)| {
                        Paragraph::new(Text::styled(
                            format!(
                                "{}\n{}\n{}",
                                event.subject,
                                event.organizer,
                                event
                                    .teams_meeting
                                    .clone()
                                    .map_or("".to_string(), |meeting| meeting.url)
                            ),
                            Style::default().fg(Color::Red).bold(),
                        ))
                    });

                let inner_area = centered_rect(60, 20, area);
                frame.render_widget(Clear, area); //this clears out the background
                frame.render_widget(Block::default().bg(Color::LightBlue), area);
                frame.render_widget(text.block(block).on_black(), inner_area);
            }
        }
        Focus::Table => {
            let layout = Layout::horizontal([Constraint::Percentage(100)])
                .flex(layout::Flex::SpaceBetween)
                .split(area);

            let header_style = Style::default()
                .fg(app.colors.header_fg)
                .bg(app.colors.header_bg);
            let selected_style = Style::default()
                .add_modifier(Modifier::REVERSED)
                .fg(app.colors.selected_style_fg);
            let header = [
                Text::from("Event")
                    .style(Style::default().bold())
                    .alignment(Alignment::Left),
                Text::from("Start Time")
                    .style(Style::default().bold())
                    .alignment(Alignment::Left),
                Text::from("Duration")
                    .style(Style::default().bold())
                    .alignment(Alignment::Left),
            ]
            .iter()
            .cloned()
            .map(Cell::from)
            .collect::<Row>()
            .style(header_style)
            .height(2);

            let rows = app.events.iter().enumerate().map(|(i, (_, e))| {
                let color = match i % 2 {
                    0 => app.colors.normal_row_color,
                    _ => app.colors.alt_row_color,
                };

                let duration = &e.end_time.signed_duration_since(e.start_time).num_minutes();
                let subject = e.subject.clone();
                // let (date, time) = reformat_time(&e.start_time);
                let local_dt: DateTime<Local> = DateTime::from(e.start_time);
                let date = local_dt.date_naive();
                let time = local_dt.time();

                Row::new(vec![
                    Text::from(subject)
                        .style(Style::default().bold())
                        .alignment(Alignment::Left),
                    Text::from(format!("{date:?} @ {time:?}")).alignment(Alignment::Left),
                    Text::from(format!("{duration:?} mins")).alignment(Alignment::Left),
                ])
                .style(Style::new().fg(app.colors.row_fg).bg(color))
                .height(3)
            });

            let widths = [
                Constraint::Length(100),
                Constraint::Length(100),
                Constraint::Length(50),
            ];
            let table = Table::new(rows, widths)
                .header(header)
                .bg(app.colors.buffer_bg)
                .highlight_style(selected_style);

            frame.render_stateful_widget(table, layout[0], &mut app.state);
        }
    }
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

fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::vertical([
        Constraint::Percentage((100 - percent_y) / 2),
        Constraint::Percentage(percent_y),
        Constraint::Percentage((100 - percent_y) / 2),
    ])
    .split(r);

    Layout::horizontal([
        Constraint::Percentage((100 - percent_x) / 2),
        Constraint::Percentage(percent_x),
        Constraint::Percentage((100 - percent_x) / 2),
    ])
    .split(popup_layout[1])[1]
}
