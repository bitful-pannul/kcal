use std::collections::HashMap;
use std::str::FromStr;

use chrono::{DateTime, Duration, Utc};
use frankenstein::GetFileParams;
use frankenstein::{ChatId, Message as TgMessage, SendMessageParams, UpdateContent};
use kinode_process_lib::{
    await_message, call_init, get_blob, http, http::send_response, println, Address, Message,
    Request, Response,
};

use llm_interface::openai::*;
use serde::{Deserialize, Serialize};
use stt_interface::STTRequest;
use stt_interface::STTResponse;
use telegram_interface::*;
use url::Url;

mod gcal;

// todo command_center extensibility!
// i want to from the UI, create a tg worker with a specific name?
pub const TG_ADDRESS: (&str, &str, &str, &str) = ("our", "tg", "command_center", "appattacc.os");
pub const LLM_ADDRESS: (&str, &str, &str, &str) =
    ("our", "openai", "command_center", "appattacc.os");
pub const STT_ADDRESS: (&str, &str, &str, &str) =
    ("our", "speech_to_text", "command_center", "appattacc.os");

wit_bindgen::generate!({
    path: "wit",
    world: "process",
});

#[derive(Debug, Serialize, Deserialize, Clone)]
struct State {
    google_token: Option<String>,
    // expiry logic here or server? or fetch upon error?
    // context? // iframe
}

#[derive(Debug, Serialize, Deserialize)]
enum CalendarRequest {
    // forwarded/accepted to/from oauth kinode
    GenerateUrl,
    Token { token: String },
    GetToday,
    ListCalendars,
}

#[derive(Debug, Serialize, Deserialize)]
enum OauthResponse {
    Url { url: String },
    RefreshToken { token: String },
    Error { error: String },
}

// for UI?
#[derive(Debug, Serialize, Deserialize)]
enum CalendarResponse {
    State { state: State },
    Error { error: String },
}

// plan:
// flow, get goggle api key by signing in, and display it too.
// keep redirect_url there, have a default kinode to contact, otherwise use another.
// then spawn tg/groqAI.whisper?
// then mirror tg interface?
// then talk, implement context + basic gets for calendars.
// maybe choose calendar to use in the beginning or something?

// also, what about security with ze bot? setting a username?
fn handle_http_message(state: &mut State, req: &http::HttpServerRequest) -> anyhow::Result<()> {
    if let http::HttpServerRequest::Http(incoming) = req {
        if incoming.path()? == "/status" {
            let headers =
                HashMap::from([("Content-Type".to_string(), "application/json".to_string())]);
            send_response(
                http::StatusCode::OK,
                Some(headers),
                serde_json::to_vec(&CalendarResponse::State {
                    state: state.clone(),
                })?,
            );
        }
    }

    Ok(())
}

fn handle_message(our: &Address, state: &mut State) -> anyhow::Result<()> {
    let msg = await_message()?;

    if msg.source().process == "http_server:distro:sys" {
        if msg.source().node != our.node {
            return Err(anyhow::anyhow!("src not our in http message..."));
        }

        let req = serde_json::from_slice::<http::HttpServerRequest>(msg.body())?;
        handle_http_message(state, &req)?;
    }

    match serde_json::from_slice::<CalendarRequest>(msg.body())? {
        CalendarRequest::GenerateUrl => {
            // choose a sane default.
            let target: Address = "our@oauth:ratatouille:template.os".parse()?;

            println!("got target: {:?}", target);
            let url = Request::new()
                .target(target)
                .body(serde_json::to_vec(&CalendarRequest::GenerateUrl)?)
                .send_and_await_response(5)??;

            println!("got url message back..");

            let res = serde_json::from_slice::<OauthResponse>(url.body())?;
            match res {
                OauthResponse::Url { url } => {
                    println!("got url: {:?}", url);
                    // open url in browser
                    // or send to UI
                }
                _ => {}
            }
        }
        CalendarRequest::Token { token } => {
            println!("got token: {:?}", token);
            // verify if it's from the right place too.
            state.google_token = Some(token);
        }
        CalendarRequest::GetToday => {
            if let Some(token) = &state.google_token {
                let (time_min, time_max) = get_time_24h();
                fetch_events_from_primary_calendar(token, &time_min, &time_max)?;
            }
        }
        CalendarRequest::ListCalendars => {
            if let Some(token) = &state.google_token {
                list_calendars(token)?;
            }
        }
    };

    Ok(())
}

fn handle_message2(our: &Address) -> anyhow::Result<()> {
    let message = await_message()?;
    if message.source().node != our.node {
        return Ok(());
    }
    handle_telegram_message(&message)
}

pub fn handle_telegram_message(message: &Message) -> anyhow::Result<()> {
    let Some(msg) = get_last_tg_msg(&message) else {
        return Ok(());
    };
    let id = msg.chat.id;
    let mut text = msg.text.clone().unwrap_or_default();
    if let Some(voice) = msg.voice.clone() {
        let audio = get_file(&voice.file_id)?;
        text += &get_text(audio)?;
    }
    let answer = get_groq_answer(&text)?;
    let _message = send_bot_message(&answer, id);
    Ok(())
}

fn send_bot_message(text: &str, id: i64) -> anyhow::Result<TgMessage> {
    let params = SendMessageParams::builder()
        .chat_id(ChatId::Integer(id))
        .text(text)
        .build();
    let send_message_request = serde_json::to_vec(&TgRequest::SendMessage(params))?;
    let response = Request::to(TG_ADDRESS)
        .body(send_message_request)
        .send_and_await_response(30)??;
    let TgResponse::SendMessage(message) = serde_json::from_slice(response.body())? else {
        return Err(anyhow::anyhow!("Failed to send message"));
    };
    Ok(message)
}

fn get_groq_answer(text: &str) -> anyhow::Result<String> {
    let request = ChatRequestBuilder::default()
        .model("llama3-8b-8192".to_string())
        .messages(vec![MessageBuilder::default()
            .role("user".to_string())
            .content(text.to_string())
            .build()?])
        .build()?;
    let request = serde_json::to_vec(&LLMRequest::GroqChat(request))?;
    let response = Request::to(LLM_ADDRESS)
        .body(request)
        .send_and_await_response(30)??;
    let LLMResponse::Chat(chat) = serde_json::from_slice(response.body())? else {
        return Err(anyhow::anyhow!("Failed to parse LLM response"));
    };
    Ok(chat.choices[0].message.content.clone())
}

fn get_text(audio: Vec<u8>) -> anyhow::Result<String> {
    let stt_request = serde_json::to_vec(&STTRequest::OpenaiTranscribe(audio))?;
    let response = Request::to(STT_ADDRESS)
        .body(stt_request)
        .send_and_await_response(3)??;
    let STTResponse::OpenaiTranscribed(text) = serde_json::from_slice(response.body())? else {
        return Err(anyhow::anyhow!("Failed to parse STT response"));
    };
    Ok(text)
}

fn get_file(file_id: &str) -> anyhow::Result<Vec<u8>> {
    let get_file_params = GetFileParams::builder().file_id(file_id).build();
    let tg_request = serde_json::to_vec(&TgRequest::GetFile(get_file_params))?;
    let _ = Request::to(TG_ADDRESS)
        .body(tg_request)
        .send_and_await_response(10)??;
    if let Some(blob) = get_blob() {
        return Ok(blob.bytes);
    }
    Err(anyhow::anyhow!("Failed to get file"))
}
fn get_last_tg_msg(message: &Message) -> Option<TgMessage> {
    let Ok(TgResponse::Update(tg_update)) = serde_json::from_slice(message.body()) else {
        return None;
    };
    let update = tg_update.updates.last()?;
    let msg = match &update.content {
        UpdateContent::Message(msg) | UpdateContent::ChannelPost(msg) => msg,
        _ => {
            return None;
        }
    };
    Some(msg.clone())
}

pub fn subscribe() -> anyhow::Result<()> {
    let subscribe_request = serde_json::to_vec(&TgRequest::Subscribe)?;
    let result = Request::to(TG_ADDRESS)
        .body(subscribe_request)
        .send_and_await_response(3)??;
    let TgResponse::Ok = serde_json::from_slice::<TgResponse>(result.body())? else {
        return Err(anyhow::anyhow!("Failed to parse subscription response"));
    };
    println!("Subscribed to telegram");
    Ok(())
}

fn fetch_events_from_primary_calendar(
    token: &str,
    time_min: &str,
    time_max: &str,
) -> anyhow::Result<()> {
    let url = Url::from_str(&format!(
        "https://www.googleapis.com/calendar/v3/calendars/primary/events?timeMin={}&timeMax={}",
        time_min, time_max
    ))?;
    let headers = HashMap::from([
        ("Authorization".to_string(), format!("Bearer {}", token)),
        ("Content-Type".to_string(), "application/json".to_string()),
    ]);
    let body = Vec::new(); // No body for GET request

    let res = http::send_request_await_response(http::Method::GET, url, Some(headers), 5, body)?;
    let json: serde_json::Value = serde_json::from_slice(&res.body())?;
    println!("events: {:?}", json);

    let events: gcal::Events = serde_json::from_slice(&res.body())?;

    println!("got events!!!: {:?}", events.items.len());
    Ok(())
}

fn get_time_24h() -> (String, String) {
    let now: DateTime<Utc> = Utc::now();
    let time_min = now.format("%Y-%m-%dT%H:%M:%SZ").to_string(); // UTC time, no milliseconds
    let time_max = (now + Duration::hours(24))
        .format("%Y-%m-%dT%H:%M:%SZ")
        .to_string(); // 24 hours from now, UTC time, no milliseconds

    (time_min, time_max)
}

fn list_calendars(token: &str) -> anyhow::Result<()> {
    let url =
        Url::from_str("https://www.googleapis.com/calendar/v3/users/me/calendarList").unwrap();

    let headers = HashMap::from([
        ("Authorization".to_string(), format!("Bearer {}", token)),
        ("Content-Type".to_string(), "application/json".to_string()),
    ]);
    let body = Vec::new(); // No body for GET request

    let res = http::send_request_await_response(http::Method::GET, url, Some(headers), 5, body)?;
    let json: serde_json::Value = serde_json::from_slice(&res.body())?;
    println!("calendars: {:?}", json);
    Ok(())
}
fn fetch_events_today(token: &str) -> anyhow::Result<()> {
    let today = Utc::now().date_naive();
    let url = format!(
        "https://www.googleapis.com/calendar/v3/calendars/primary/events?timeMin={}&timeMax={}",
        today.and_hms_opt(0, 0, 0).unwrap(),
        today.and_hms_opt(23, 59, 59).unwrap()
    );
    let headers = HashMap::from([
        ("Authorization".to_string(), format!("Bearer {}", token)),
        ("Content-Type".to_string(), "application/json".to_string()),
    ]);
    let body = Vec::new();

    let resp = http::send_request_await_response(
        http::Method::GET,
        Url::from_str(&url)?,
        Some(headers),
        5,
        body,
    )?;
    let json: serde_json::Value = serde_json::from_slice(&resp.body())?;
    println!("events: {:?}", json);

    // parse resp into somethings
    Ok(())
}

// others

call_init!(init);
fn init(our: Address) {
    println!("client begin");

    http::bind_http_path("/", false, false).unwrap();

    let mut state = State { google_token: None };

    loop {
        match handle_message(&our, &mut state) {
            Ok(_) => {}
            Err(e) => {
                println!("error: {:?}", e);
            }
        };
    }
}
