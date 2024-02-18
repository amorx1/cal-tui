use std::{
    collections::BTreeMap,
    process::Command,
    sync::mpsc::{Receiver, Sender},
    time::Duration,
};

use chrono::{DateTime, Utc};
use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use ratatui::{
    backend::Backend, style::palette::tailwind::Palette, widgets::TableState, Frame, Terminal,
};
use tokio::{io, runtime::Runtime, time::sleep};

use crate::{
    outlook::{CalendarEvent, EventCommand},
    ui::{render_popup, render_selection, render_table, TableColors},
};

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
}

impl App {
    pub fn new(theme: &Palette) -> Self {
        Self {
            events: BTreeMap::new(),
            colors: TableColors::new(theme),
            table_state: TableState::default().with_selected(0),
            focus: Focus::Table,
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

    pub fn run<B: Backend>(
        mut self,
        terminal: &mut Terminal<B>,
        data_runtime: Runtime,
        event_rx: Receiver<EventCommand>,
        timer_tx: Sender<()>,
        timer_rx: Receiver<()>,
    ) -> io::Result<()> {
        loop {
            terminal.draw(|f| self.ui(f))?;

            // Manual event handlers.
            if let Ok(true) = event::poll(Duration::from_millis(50)) {
                if let Event::Key(key) = event::read()? {
                    if key.kind == KeyEventKind::Press {
                        match key.code {
                            KeyCode::Char('q') => return Ok(()),
                            KeyCode::Char('h') => self.focus = Focus::Table,
                            KeyCode::Char('l') => self.focus = Focus::Selected,
                            KeyCode::Char('j') | KeyCode::Down => match self.focus {
                                Focus::Table => self.next(),
                                _ => {}
                            },
                            KeyCode::Char('k') | KeyCode::Up => match self.focus {
                                Focus::Table => self.previous(),
                                _ => {}
                            },
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
                        .checked_sub_signed(chrono::Duration::minutes(2)) // TODO: Make reminder offset configurable
                        .map(|x| x.signed_duration_since(Utc::now()).num_milliseconds())
                        .unwrap();

                    // None -> event didn't already exist, so it is safe to create a new timer for it without duplicating.
                    if self.events.insert(event.start_time, event).is_none() {
                        let timer_tx = timer_tx.clone();
                        data_runtime.spawn(async move {
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
                self.popup();
            }

            // Clear past events
            self.events.retain(|_, event| event.end_time >= Utc::now());
        }
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
