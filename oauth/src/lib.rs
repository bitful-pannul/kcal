use std::collections::HashMap;

use kinode::process::standard::get_state;
use kinode_process_lib::http::send_response;
use kinode_process_lib::{
    await_message, call_init, get_blob, http, println, Address, Request, Response,
};
use oauth2::basic::BasicClient;
use oauth2::{
    AuthUrl, ClientId, ClientSecret, CsrfToken, PkceCodeChallenge, RedirectUrl, Scope, TokenUrl,
};
use serde::{Deserialize, Serialize};

const SUCCESS_HTML: &[u8] = include_bytes!("../../pkg/oauth-ui/index.html");

// still todo:
// - website to dislay congrats, you're logged in, and your google token has been sent to your kinode,
// {address}. Go back and finish the setup!
// {maybe we put goggle calendar in there too?}
wit_bindgen::generate!({
    path: "wit",
    world: "process",
});

#[derive(Debug, Serialize, Deserialize)]
struct State {
    inner: OauthState,
    tokens: HashMap<Address, TokenMetadata>,
    exchanges: HashMap<String, (Address, String)>,
}

#[derive(Debug, Serialize, Deserialize)]
struct OauthState {
    client_id: String,
    client_secret: String,
    auth_url: String,
    // auth_type: AuthType,
    token_url: String,
    redirect_url: String,
    // introspection_url: Option<IntrospectionUrl>,
    // revocation_url: Option<RevocationUrl>,
}

#[derive(Debug, Serialize, Deserialize)]
struct TokenMetadata {
    token_expiry: u64,
    token_scope: Vec<String>,
    refresh_token: String,
}

#[derive(Debug, Serialize, Deserialize)]
enum OauthRequest {
    GenerateUrl,
    RefreshToken,
    Exchange { code: String, state: String },
}

#[derive(Debug, Serialize, Deserialize)]
enum OauthResponse {
    Url { url: String },
    Token { token: String },
    Error { error: String },
}

#[derive(Debug, Serialize, Deserialize)]
struct Initialize {
    client_id: String,
    client_secret: String,
    auth_url: String,
    token_url: String,
    redirect_url: String,
}

fn generate_url(
    source: &Address,
    client: &mut BasicClient,
    state: &mut State,
) -> anyhow::Result<()> {
    let (pkce_challenge, pkce_verifier) = PkceCodeChallenge::new_random_sha256();

    let (auth_url, csrf_token) = client
        .authorize_url(CsrfToken::new_random)
        .add_scope(Scope::new(
            "https://www.googleapis.com/auth/calendar".to_string(),
        ))
        .add_extra_param("access_type", "offline")
        .add_extra_param("prompt", "consent")
        .set_pkce_challenge(pkce_challenge)
        .url();

    // NOTE here, we're adding the extra_param prompt:consent every time, which
    // leads to every time user is redirected, needing consent.
    // could be improved, and technically one shouldn't need this after initial consent,
    // but in our usecase, ideally this should only happen once/not very often, as the UI is telegram.

    println!("Browse to: {}", auth_url);

    println!("csrf token: {:?}", csrf_token.secret());
    println!("pkce verifier: {:?}", pkce_verifier.secret());

    state.exchanges.insert(
        csrf_token.secret().clone(),
        (source.clone(), pkce_verifier.secret().clone()),
    );

    let _ = Response::new()
        .body(
            serde_json::to_vec(&OauthResponse::Url {
                url: auth_url.to_string(),
            })
            .unwrap(),
        )
        .send();

    Ok(())
}

fn refresh_access_token(
    source: &Address,
    refresh_token: &str,
    state: &mut State,
) -> anyhow::Result<()> {
    let mut headers = HashMap::new();
    headers.insert(
        "Content-Type".to_string(),
        "application/x-www-form-urlencoded".to_string(),
    );

    let body = url::form_urlencoded::Serializer::new(String::new())
        .append_pair("grant_type", "refresh_token")
        .append_pair("client_id", &state.inner.client_id)
        .append_pair("client_secret", &state.inner.client_secret)
        .append_pair("refresh_token", refresh_token)
        .finish();

    let resp = http::send_request_await_response(
        http::Method::POST,
        state.inner.token_url.parse().unwrap(),
        Some(headers),
        5,
        body.into_bytes(),
    )?;

    println!("HTTP response: {:?}", resp);

    let resp_json_body: serde_json::Value = serde_json::from_slice(&resp.body())?;

    println!("Response JSON body: {:?}", resp_json_body);
    let new_access_token = resp_json_body
        .get("access_token")
        .ok_or_else(|| anyhow::anyhow!("Access token not found in response"))?
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("Invalid access token format"))?
        .to_string();

    let new_expires_in = resp_json_body
        .get("expires_in")
        .ok_or_else(|| anyhow::anyhow!("Expires in not found in response"))?
        .as_u64()
        .ok_or_else(|| anyhow::anyhow!("Invalid expires in format"))?;
    println!("Response JSON body: {:?}", resp_json_body);

    state.tokens.insert(
        source.clone(),
        TokenMetadata {
            token_expiry: new_expires_in,
            token_scope: vec![],
            refresh_token: refresh_token.to_string(),
        },
    );

    let _ = Request::new()
        .target(source)
        .body(
            serde_json::to_vec(&OauthResponse::Token {
                token: new_access_token.to_string(),
            })
            .unwrap(),
        )
        .send();

    // todo, refactor into a better

    Ok(())
}

// take other things in too. it's source + url? only? or should we separate the code...
// well actually
// this action. should come from the redirect ourselves.
// so, it should come from http_server
// and, it should have the same state thing as we passed first, and an address.
// in that case we can send a request to that mofo that had the thing going!
// and errors will be propagated to both client/server UI?
//
// all client UI needs is generate_url() for auth. (this can be done again and again I think)?
//
fn exchange_code(
    code: &String,
    source: &Address,
    verifier: &str,
    state: &mut State,
) -> anyhow::Result<()> {
    let mut headers = HashMap::new();
    headers.insert(
        "Content-Type".to_string(),
        "application/x-www-form-urlencoded".to_string(),
    );

    let body = url::form_urlencoded::Serializer::new(String::new())
        .append_pair("grant_type", "authorization_code")
        .append_pair("client_id", &state.inner.client_id)
        .append_pair("client_secret", &state.inner.client_secret)
        .append_pair("code", &code)
        .append_pair("redirect_uri", &state.inner.redirect_url)
        .append_pair("code_verifier", &verifier)
        .finish();

    let resp = http::send_request_await_response(
        http::Method::POST,
        state.inner.token_url.parse().unwrap(),
        Some(headers),
        5,
        body.into_bytes(),
    )?;

    println!("resp: {:?}", resp);

    let resp_json_body: serde_json::Value = serde_json::from_slice(&resp.body())?;
    println!("resp json body: {:?}", resp_json_body);
    let token = resp_json_body
        .get("access_token")
        .ok_or_else(|| anyhow::anyhow!("Access token not found in response"))?
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("Invalid access token format"))?
        .to_string();

    let refresh_token = resp_json_body
        .get("refresh_token")
        .ok_or_else(|| anyhow::anyhow!("Refresh token not found in response"))?
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("Invalid refresh token format"))?
        .to_string();

    let expires_in = resp_json_body
        .get("expires_in")
        .ok_or_else(|| anyhow::anyhow!("Expires in not found in response"))?
        .as_u64()
        .ok_or_else(|| anyhow::anyhow!("Invalid expires in format"))?;

    println!("resp json exchange: {:?}", resp_json_body);
    println!("inserting with source! {:?}", source);
    state.tokens.insert(
        source.clone(),
        TokenMetadata {
            token_expiry: expires_in,
            token_scope: vec![],
            refresh_token: refresh_token,
        },
    );

    let _ = Request::new()
        .target(source)
        .body(
            serde_json::to_vec(&OauthResponse::Token {
                token: token.to_string(),
            })
            .unwrap(),
        )
        .send();

    Ok(())
}

fn handle_message(
    _our: &Address,
    state: &mut State,
    client: &mut BasicClient,
) -> anyhow::Result<()> {
    let message = await_message()?;

    if !message.is_request() {
        return Err(anyhow::anyhow!("unexpected Response: {:?}", message));
    }

    if message.source().process == "http_server:distro:sys" {
        let msg: http::HttpServerRequest = serde_json::from_slice(message.body())?;

        let http::HttpServerRequest::Http(req) = msg else {
            send_response(http::StatusCode::BAD_REQUEST, None, vec![]);
            return Err(anyhow::anyhow!("unexpected message: {:?}", message));
        };

        if req.path()? != "/auth" {
            send_response(http::StatusCode::BAD_REQUEST, None, vec![]);
            return Err(anyhow::anyhow!("unexpected path: {:?}", req.path()?));
        }

        let headers = HashMap::from([("Content-Type".to_string(), "text/html".to_string())]);
        send_response(http::StatusCode::OK, Some(headers), SUCCESS_HTML.to_vec());

        let code = req
            .query_params()
            .get("code")
            .ok_or_else(|| anyhow::anyhow!("no code in query params"))?;
        let state_str = req
            .query_params()
            .get("state")
            .ok_or_else(|| anyhow::anyhow!("no state in query params"))?;

        if let Some((addr, verifier)) = state.exchanges.get_mut(state_str).cloned() {
            exchange_code(code, &addr, &verifier, state)?;
        } else {
            send_response(http::StatusCode::UNAUTHORIZED, None, vec![]);
            return Err(anyhow::anyhow!(
                "unexpected state, no auth for u : {:?}",
                state
            ));
        }
    }

    let req: OauthRequest = serde_json::from_slice(message.body())?;

    match req {
        OauthRequest::GenerateUrl => {
            generate_url(message.source(), client, state)?;
        }
        OauthRequest::RefreshToken => {
            if let Some(token_metadata) = state.tokens.get(&message.source()) {
                let refresh_token = token_metadata.refresh_token.clone();
                refresh_access_token(&message.source(), &refresh_token, state)?;
            }
        }
        OauthRequest::Exchange { .. } => {
            // reason for this is, the http_redirect url needs to be defined at the start.
            // so you can't really redirect to your own kinode that would do this.
            println!("currently only available through http redirect");
        }
    }

    Ok(())
}

fn initialize() -> State {
    // try to get saved state first, then wait for Initialize message either from
    // http or command line.
    match get_state() {
        Some(state) => {
            let state: State = serde_json::from_slice(&state).unwrap();
            return state;
        }
        None => {}
    }

    loop {
        if let Ok(message) = await_message() {
            println!("got message!");
            if message.source().process == "http_server:distro:sys" {
                let msg: http::HttpServerRequest = serde_json::from_slice(message.body()).unwrap();

                if let http::HttpServerRequest::Http(_req) = msg {
                    let body = get_blob().unwrap();
                    let init = serde_json::from_slice::<Initialize>(&body.bytes).unwrap();
                    return State {
                        inner: OauthState {
                            client_id: init.client_id,
                            client_secret: init.client_secret,
                            auth_url: init.auth_url,
                            token_url: init.token_url,
                            redirect_url: init.redirect_url,
                        },
                        tokens: HashMap::new(),
                        exchanges: HashMap::new(),
                    };
                }
            }
            println!("trying deserialize");
            if let Ok(init) = serde_json::from_slice::<Initialize>(message.body()) {
                println!("got init!");
                return State {
                    inner: OauthState {
                        client_id: init.client_id,
                        client_secret: init.client_secret,
                        auth_url: init.auth_url,
                        token_url: init.token_url,
                        redirect_url: init.redirect_url,
                    },
                    tokens: HashMap::new(),
                    exchanges: HashMap::new(),
                };
            }
        }
    }
}

fn create_oauth_client(state: &State) -> Result<BasicClient, url::ParseError> {
    let client = BasicClient::new(
        ClientId::new(state.inner.client_id.clone()),
        Some(ClientSecret::new(state.inner.client_secret.clone())),
        AuthUrl::new(state.inner.auth_url.clone())?,
        Some(TokenUrl::new(state.inner.token_url.clone())?),
    );
    let client = client.set_redirect_uri(RedirectUrl::new(state.inner.redirect_url.clone())?);
    Ok(client)
}

// general plan:
// boot sst.wasm in here.
// make sure command_center is installed before anything else

// oauth flow.
// client app sends a message to default server provider? (could also be server url.. I think)
// server stores address, +state -> sends back a url to client.
// client opens browser to url, user logs in, gets redirected to server.
// server exchanges code for token, stores token, sends back to client <- actually can be sent with KINODE!
call_init!(init);
fn init(our: Address) {
    println!("begin, our: {:?}", our);

    // only bound for potential UI initialization
    http::bind_http_path("/server", false, false).unwrap();

    // bound as the redirect path for successful auth!
    // todo: better webpage serving too...
    http::bind_http_path("/auth", false, false).unwrap();

    let mut state = initialize();
    let mut client = create_oauth_client(&state).unwrap();

    loop {
        match handle_message(&our, &mut state, &mut client) {
            Ok(()) => {}
            Err(e) => {
                println!("error: {:?}", e);
            }
        };
    }
}
