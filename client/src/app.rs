use crate::{
    backend::Backend as AppBackend,
    outlook::{CalendarEvent, EventCommand},
    ui::{render_popup, render_selection, render_table, TableColors},
};
use chrono::{DateTime, Utc};
use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use ratatui::{
    backend::Backend, style::palette::tailwind::Palette, widgets::TableState, Frame, Terminal,
};
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
    pub fn new(theme: &Palette, backend: AppBackend) -> Self {
        backend.start();
        Self {
            events: BTreeMap::new(),
            colors: TableColors::new(theme),
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
            while let Some(command) = self.poll_calendar_events() {
                if let EventCommand::Add(event) = command {
                    // None -> event didn't already exist, so it is safe to create a new timer for it without duplicating.
                    let start_time = event.start_time;
                    if self.events.insert(start_time, event).is_none() {
                        self.spawn_timer(start_time);
                    }
                }
            }

            // A timeout notification has been received, meaning an alert should be displayed.
            if self.poll_timers() {
                self.popup();
            }

            // Clear past events
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

    pub fn set_focus(&mut self, focus: Focus) {
        self.focus = focus;
    }

    pub fn poll_calendar_events(&self) -> Option<EventCommand> {
        self.backend.event_rx.try_iter().next()
    }

    pub fn spawn_timer(&self, end: DateTime<Utc>) {
        let eta = end
            .checked_sub_signed(chrono::Duration::minutes(2)) // TODO: Make reminder offset configurable
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
