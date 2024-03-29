use crate::{
    backend::Backend as AppBackend,
    outlook::CalendarEvent,
    ui::{render_popup, render_selection, render_table, TableColors, PALETTES},
    CONFIG, CONFIG_PATH,
};
use chrono::{DateTime, Utc};
use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use ratatui::{backend::Backend, widgets::TableState, Frame, Terminal};
use serde::Deserialize;
use std::{collections::BTreeMap, process::Command, time::Duration};
use tokio::{io, time::sleep};

#[derive(Clone, Copy)]
pub enum Focus {
    Table,
    Selected,
    Popup,
}

pub struct App {
    pub table_state: TableState,
    pub focus: Focus,
    pub events: BTreeMap<DateTime<Utc>, CalendarEvent>,
    pub colors: TableColors,
    pub backend: AppBackend,
}

impl App {
    pub fn new(backend: AppBackend) -> Self {
        backend.start();
        Self {
            events: BTreeMap::new(),
            colors: TableColors::new(&PALETTES[CONFIG.get().unwrap().theme]),
            table_state: TableState::default().with_selected(0),
            focus: Focus::Table,
            backend,
        }
    }

    pub fn run<B: Backend>(mut self, terminal: &mut Terminal<B>) -> io::Result<()> {
        loop {
            terminal.draw(|f| self.ui(f))?;

            // Manual event handlers.
            if let Ok(true) = event::poll(Duration::from_millis(50)) {
                if let Event::Key(key) = event::read()? {
                    if key.kind == KeyEventKind::Press {
                        match key.code {
                            KeyCode::Char('q') => return Ok(()),
                            KeyCode::Char('h') => self.set_focus(Focus::Table),
                            KeyCode::Char('l') => self.set_focus(Focus::Selected),
                            KeyCode::Char('j') | KeyCode::Down => {
                                if let Focus::Table = self.focus {
                                    self.next()
                                }
                            }
                            KeyCode::Char('k') | KeyCode::Up => {
                                if let Focus::Table = self.focus {
                                    self.previous()
                                }
                            }
                            _ => (),
                        }
                    }
                }
            }

            // Listen for new events from refresh thread.
            while let Some(event) = self.poll_calendar_events() {
                if let Some(time) = self.add_event(event) {
                    self.spawn_timer(time);
                }
            }

            // A timeout notification has been received, meaning an alert should be displayed.
            if self.poll_timers() {
                self.popup();
            }

            // Clear expired events
            self.events.retain(|_, event| event.end_time >= Utc::now());
        }
    }

    pub fn ui(&mut self, frame: &mut Frame) {
        let area = frame.size();

        match self.focus {
            // Alert for upcoming event
            Focus::Popup => {
                render_popup(self, frame, area);
            }
            // Detailed view for selected event
            Focus::Selected => {
                render_selection(self, frame, area);
            }
            // Table of upcoming events
            Focus::Table => {
                render_table(self, frame, area);
            }
        }
    }
    pub fn add_event(&mut self, event: CalendarEvent) -> Option<DateTime<Utc>> {
        let start_time = event.start_time;
        if self.events.insert(start_time, event).is_none() {
            return Some(start_time);
        }
        None
    }

    pub fn set_focus(&mut self, focus: Focus) {
        self.focus = focus;
    }

    pub fn poll_calendar_events(&self) -> Option<CalendarEvent> {
        self.backend.event_rx.try_iter().next()
    }

    pub fn spawn_timer(&self, end: DateTime<Utc>) {
        let eta = end
            .checked_sub_signed(chrono::Duration::minutes(
                CONFIG.get().unwrap().notification_period_minutes,
            )) // TODO: Make reminder offset configurable
            .map(|x| x.signed_duration_since(Utc::now()).num_milliseconds())
            .unwrap();

        let timer_tx = self.backend.timer_tx.clone();
        self.backend.timer.spawn(async move {
            sleep(Duration::from_millis(eta as u64)).await;
            timer_tx
                .send(())
                .expect("ERROR: Could not send timer notification");
        });
    }

    pub fn poll_timers(&self) -> bool {
        self.backend.timer_rx.try_recv().is_ok()
    }

    pub fn popup(&mut self) {
        self.focus = Focus::Popup;
        _ = Command::new("zellij")
            .args(["action", "toggle-floating-panes"])
            .status()
            .expect("ERROR: Could not send command to Zellij");
    }

    pub fn next(&mut self) {
        let i = match self.table_state.selected() {
            Some(i) => {
                if i >= self.events.len() - 1 {
                    0
                } else {
                    i + 1
                }
            }
            None => 0,
        };
        self.table_state.select(Some(i));
    }

    pub fn previous(&mut self) {
        let i = match self.table_state.selected() {
            Some(i) => {
                if i == 0 {
                    self.events.len() - 1
                } else {
                    i - 1
                }
            }
            None => 0,
        };
        self.table_state.select(Some(i));
    }
}

#[derive(Debug, Deserialize)]
pub struct Config {
    pub theme: usize,
    pub notification_period_minutes: i64,
    pub refresh_period_seconds: u32,
    pub limit_days: u64,
    pub auth_timeout_millis: u64,
    pub outlook: OutlookConfig,
}

#[derive(Debug, Deserialize)]
pub struct OutlookConfig {
    pub client_id: String,
    pub base_url: String,
}

impl Config {
    pub fn from_path() -> Self {
        let home = std::env::var_os("HOME").expect("ERROR: No HOME OS variable found!");
        let config_path = CONFIG_PATH
            .get()
            .expect("ERROR: No config path resolved!")
            .replace("$HOME", home.to_str().unwrap());
        let file =
            std::fs::read_to_string(config_path).expect("ERROR: Could not read config file!");
        toml::from_str(&file).unwrap()
    }
}
