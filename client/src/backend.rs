use crate::{
    auth::start_auth_server,
    outlook::{refresh, CalendarEvent},
};
use reqwest::Client;
use std::{
    sync::mpsc::{channel, Receiver, Sender},
    time::Duration,
};
use tokio::runtime::{self, Runtime};

pub struct Backend {
    pub auth: Runtime,
    pub data: Runtime,
    pub timer: Runtime,
    pub event_tx: Sender<CalendarEvent>,
    pub event_rx: Receiver<CalendarEvent>,
    pub timer_tx: Sender<()>,
    pub timer_rx: Receiver<()>,
}

impl Backend {
    pub fn new() -> Self {
        let auth = runtime::Builder::new_multi_thread()
            .worker_threads(1)
            .thread_name("warp")
            .enable_all()
            .build()
            .unwrap();

        let data = runtime::Builder::new_multi_thread()
            .worker_threads(1)
            .thread_name("outlook")
            .enable_all()
            .build()
            .unwrap();

        let timer = runtime::Builder::new_multi_thread()
            .worker_threads(1)
            .thread_name("timer")
            .enable_all()
            .build()
            .unwrap();

        let (event_tx, event_rx) = channel();
        let (timer_tx, timer_rx) = channel();

        Self {
            auth,
            data,
            timer,
            event_tx,
            event_rx,
            timer_tx,
            timer_rx,
        }
    }

    pub fn start(&self) {
        // Auth thread
        let (auth_tx, auth_rx) = channel();
        self.auth
            .spawn(async move { start_auth_server(auth_tx).await });
        let token = auth_rx
            .recv_timeout(Duration::from_millis(10000))
            .expect("ERROR: Unsuccessful authentication!");

        // Start data refresh thread
        let event_tx = self.event_tx.clone();
        self.data
            .spawn(async move { refresh(token, Client::new(), event_tx).await });
    }
}
