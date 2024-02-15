use std::{collections::HashMap, env, sync::mpsc::Sender};

use graph_oauth::oauth::{AccessToken, IdToken, OAuth};
use warp::Filter;

pub fn oauth_open_id() -> OAuth {
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
        .prompt("none")
        .state("12345");
    oauth
}

pub async fn handle_redirect(
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
