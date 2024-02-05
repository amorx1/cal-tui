use std::{
    collections::{BTreeMap, HashMap},
    env,
    io::{self, stdout},
    sync::{
        mpsc::{channel, Receiver, Sender},
        Arc, Mutex,
    },
    thread,
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
    widgets::{Row, Table, TableState},
};

use graph_oauth::oauth::{AccessToken, IdToken, OAuth};
use reqwest::Client;
use tokio::{runtime, task};
use warp::Filter;

use dotenv::dotenv;

mod outlook;
use outlook::Root;

#[derive(Default, Clone)]
struct CalendarEvent {
    start_time: DateTime<Utc>,
    end_time: DateTime<Utc>,
    subject: String,
}

struct App {
    auth_token: Option<String>,
    show_table: bool,
    events: BTreeMap<DateTime<Utc>, CalendarEvent>,
}

fn oauth_open_id() -> OAuth {
    let mut oauth = OAuth::new();
    oauth
        .client_id(
            env::var("CLIENT_ID")
                .expect("No CLIENT_ID provided")
                .as_ref(),
        )
        .authorize_url("https://login.microsoftonline.com/common/oauth2/v2.0/authorize")
        .redirect_uri("http://localhost:8000/redirect")
        .access_token_url("https://login.microsoftonline.com/common/oauth2/v2.0/token")
        .refresh_token_url("https://login.microsoftonline.com/common/oauth2/v2.0/token")
        .response_type("id_token code")
        .response_mode("form_post")
        .add_scope("openid")
        .add_scope("Calendars.ReadBasic")
        .add_scope("offline_access")
        .nonce("7362CAEA-9CA5")
        .prompt("consent")
        .state("12345");
    oauth
}

async fn handle_redirect(
    id_token: IdToken,
    tx: Sender<String>,
) -> Result<Box<dyn warp::Reply>, warp::Rejection> {
    // println!("Received IdToken: {id_token:#?}");

    let mut oauth = oauth_open_id();

    // Pass the id token to the oauth client.
    oauth.id_token(id_token);

    // Build the request to get an access token using open id connect.
    let mut request = oauth.build_async().open_id_connect();

    // Request an access token.
    let response = request.access_token().send().await.unwrap();
    // println!("{response:#?}");

    if response.status().is_success() {
        let access_token: AccessToken = response.json().await.unwrap();

        // You can optionally pass the access token to the oauth client in order
        // to use a refresh token to get more access tokens. The refresh token
        // is stored in AccessToken.
        let bearer_token = access_token.bearer_token();
        tx.send(bearer_token.to_string())
            .expect("ERROR: Could not send token between threads!");
        oauth.access_token(access_token);

        // If all went well here we can print out the OAuth config with the Access Token.
        // println!("OAuth:\n{:#?}\n", &oauth);
    } else {
        // See if Microsoft Graph returned an error in the Response body
        // let result: reqwest::Result<serde::Value> = response.json().await;
        // println!("{result:#?}");
    }

    // Generic login page response.
    Ok(Box::new(
        "Successfully Logged In! You can close your browser.",
    ))
}

pub async fn start_server_main(tx: Sender<String>) {
    let cors = warp::cors().allow_any_origin();

    let routes = warp::post()
        .and(warp::path("redirect"))
        .and(warp::body::form())
        .map(|simple_map: HashMap<String, String>| {
            IdToken::new(
                simple_map.get("id_token").expect("No id_token returned"),
                simple_map.get("code").expect("No code returned"),
                simple_map.get("state").expect("No state returned"),
                simple_map
                    .get("session_state")
                    .expect("No session_state returned"),
            )
        })
        .and_then(move |id_token| {
            let tx = tx.clone();
            handle_redirect(id_token, tx)
        })
        .with(cors);

    // Get the oauth client and request a browser sign in.
    let mut oauth = oauth_open_id();
    let mut request = oauth.build_async().open_id_connect();
    request.browser_authorization().open().unwrap();

    warp::serve(routes).run(([127, 0, 0, 1], 8000)).await;
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
        auth_token: None,
        show_table: false,
        events: BTreeMap::new(),
    };

    let (tx_event, rx_event) = channel();
    let client = Client::new();

    outlook_thread.spawn(async move { refresh(token, start_arg, end_arg, client, tx_event).await });
    // tokio::task::spawn(async move {
    //     let client = client.clone();
    //     refresh(token, start_arg, end_arg, client, tx_event).await;
    // })
    // .await
    // .unwrap();

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

            calendar_events.for_each(|e| {
                tx.send(e).expect("ERROR: Could not send event to UI");
            });
        };
    }
}

fn run_app<B: Backend>(
    terminal: &mut Terminal<B>,
    mut app: App,
    rx: Receiver<CalendarEvent>,
) -> io::Result<()> {
    loop {
        let e = rx.try_recv();
        if let Ok(event) = e {
            _ = app.events.insert(event.start_time.clone(), event);
        }

        terminal.draw(|f| ui(f, &app))?;

        // rx.try_iter().for_each(|e| {
        //     println!("Received something");
        //     _ = app.events.insert(e.start_time.clone(), e);
        // });

        if let Event::Key(key) = event::read()? {
            if key.kind == KeyEventKind::Press {
                match key.code {
                    KeyCode::Char('q') => return Ok(()),
                    KeyCode::Char('p') => app.show_table = !app.show_table,
                    _ => {}
                }
            }
        }
    }
}

fn ui(frame: &mut Frame, app: &App) {
    let rows = app
        .events
        .iter()
        .map(|(time, e)| Row::new(vec![time.to_string(), e.subject.clone()]));

    let widths = (0..app.events.len()).map(|_| Constraint::Length(60));
    let table = Table::new(rows, widths);
    frame.render_widget(table, frame.size());
    // let n = app.events.len();
    // frame.render_widget(Text::raw(format!("{}", n)), frame.size());
}
