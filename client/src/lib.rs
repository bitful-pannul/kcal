use std::collections::HashMap;
use std::str::FromStr;

use chrono::{DateTime, Duration, Utc};
use frankenstein::GetFileParams;
use frankenstein::{ChatId, Message as TgMessage, SendMessageParams, UpdateContent};
use kinode_process_lib::ProcessId;
use kinode_process_lib::{
    await_message, call_init, get_blob, http, http::send_response, println, Address, Message,
    Request,
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
    GenerateUrl { target: String },
    Token { token: String },
    // temporary test commands
    GetToday,
    ListCalendars,
    Schedule,
}

#[derive(Debug, Serialize, Deserialize)]
enum OauthResponse {
    GenerateUrl,
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
// then spawn tg/groqAI.whisper?
// then mirror tg interface?
// then talk, implement context + basic gets for calendars.
// maybe choose calendar to use in the beginning or something?

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
            return Ok(());
        } else if incoming.path()? == "/generate" {
            let Some(blob) = get_blob() else {
                return Err(anyhow::anyhow!("Failed to get blob"));
            };
            let json = serde_json::from_slice::<serde_json::Value>(&blob.bytes)?;

            let target_str = json
                .get("target")
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow::anyhow!("Failed to get target"))?;

            let target = Address::new::<String, ProcessId>(
                target_str.to_string(),
                ProcessId::from_str("oauth:ratatouille:template.os")?,
            );

            let resp = Request::new()
                .target(target)
                .body(serde_json::to_vec(&OauthResponse::GenerateUrl)?)
                .send_and_await_response(5)??;

            let res = serde_json::from_slice::<OauthResponse>(resp.body())?;
            if let OauthResponse::Url { url } = res {
                let headers =
                    HashMap::from([("Content-Type".to_string(), "application/json".to_string())]);
                send_response(
                    http::StatusCode::OK,
                    Some(headers),
                    serde_json::to_vec(&OauthResponse::Url { url })?,
                );
            }
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
        return Ok(());
    }

    match serde_json::from_slice::<CalendarRequest>(msg.body())? {
        CalendarRequest::GenerateUrl { target } => {
            // todo cleanup
            let target: Address = Address::new::<String, ProcessId>(
                target,
                ProcessId::from_str("oauth:ratatouille:template.os")?,
            );

            let url = Request::new()
                .target(target)
                .body(serde_json::to_vec(&OauthResponse::GenerateUrl)?)
                .send_and_await_response(5)??;

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
        CalendarRequest::Schedule => {
            if let Some(token) = &state.google_token {
                let event = schedule_event("Test Event", "This is a test event", Utc::now(), 1)?;
                add_event_to_calendar(token, &event)?;
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
    Ok(())
}

fn schedule_event(
    summary: &str,
    description: &str,
    start_time: DateTime<Utc>,
    duration_hours: i64,
) -> anyhow::Result<gcal::Event> {
    let end_time = start_time + Duration::hours(duration_hours);

    let event = gcal::Event {
        summary: Some(summary.to_string()),
        description: Some(description.to_string()),
        start: Some(gcal::EventCalendarDate {
            date: None,
            date_time: Some(start_time.to_rfc3339()),
            time_zone: None,
        }),
        end: Some(gcal::EventCalendarDate {
            date: None, // Set to None unless it's an all-day event
            date_time: Some(end_time.to_rfc3339()),
            time_zone: None,
        }),
        ..Default::default()
    };
    Ok(event)
}

fn add_event_to_calendar(token: &str, event: &gcal::Event) -> anyhow::Result<()> {
    let url = Url::from_str("https://www.googleapis.com/calendar/v3/calendars/primary/events")?;
    let headers = HashMap::from([
        ("Authorization".to_string(), format!("Bearer {}", token)),
        ("Content-Type".to_string(), "application/json".to_string()),
    ]);

    let body = serde_json::to_vec(event)?;
    let res = http::send_request_await_response(http::Method::POST, url, Some(headers), 30, body)?;

    if res.status().is_success() {
        println!("Event created successfully.");
        Ok(())
    } else {
        let err_msg = format!(
            "Failed to create event: {}",
            String::from_utf8_lossy(res.body())
        );
        Err(anyhow::anyhow!(err_msg))
    }
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

    let events: gcal::Events = serde_json::from_slice(&res.body())?;

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
    Ok(())
}

call_init!(init);
fn init(our: Address) {
    println!("client begin");

    http::serve_index_html(&our, "client-ui/", true, false, vec!["/"]).unwrap();
    http::bind_http_path("/status", true, false).unwrap();
    http::bind_http_path("/generate", true, false).unwrap();

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
