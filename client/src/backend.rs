use std::{
    sync::mpsc::{channel, Receiver, Sender},
    time::Duration,
};

use reqwest::Client;
use tokio::runtime::{self, Runtime};

use crate::{
    auth::start_server_main,
    outlook::{refresh, EventCommand},
};

pub struct Backend {
    pub auth: Runtime,
    pub data: Runtime,
    pub timer: Runtime,
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

        Self { auth, data, timer }
    }

    pub fn init(&mut self) -> (Receiver<EventCommand>, Sender<()>, Receiver<()>) {
        // OAuth Listener
        let (auth_tx, auth_rx) = channel();
        self.auth
            .spawn(async move { start_server_main(auth_tx).await });
        let token = auth_rx
            .recv_timeout(Duration::from_millis(10000))
            .expect("ERROR: Unsuccessful authentication!");

        // Data refresh thread
        let (event_tx, event_rx) = channel();
        self.data
            .spawn(async move { refresh(token, Client::new(), event_tx).await });

        let (timer_tx, timer_rx) = channel();

        (event_rx, timer_tx, timer_rx)
    }
}
