use std::{collections::BTreeMap, process::Command};

use chrono::{DateTime, Utc};
use ratatui::widgets::TableState;

use crate::{outlook::CalendarEvent, ui::TableColors};

#[derive(Clone, Copy)]
pub enum Focus {
    Table,
    Selected,
    Popup,
}

pub struct App {
    pub state: TableState,
    pub focus: Focus,
    pub events: BTreeMap<DateTime<Utc>, CalendarEvent>,
    pub colors: TableColors,
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
