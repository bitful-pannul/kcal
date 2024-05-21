use kinode_process_lib::ProcessId;
use kinode_process_lib::{
    await_message, call_init, get_blob, http, http::send_response, println, Address, Message,
    Request,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::str::FromStr;

mod gcal;
mod groq;
mod prompts;
mod stt;
mod tg;

use gcal::helpers::*;
use tg::*;

use crate::gcal::SimpleEvent;
use crate::prompts::{get_default_prompt, EVENTS_PROMPT};

pub const LLM_ADDRESS: (&str, &str, &str, &str) = ("our", "openai", "ratatouille", "template.os");
pub const TG_ADDRESS: (&str, &str, &str, &str) = ("our", "tg", "ratatouille", "template.os");
pub const STT_ADDRESS: (&str, &str, &str, &str) =
    ("our", "speech_to_text", "ratatouille", "template.os");

wit_bindgen::generate!({
    path: "wit",
    world: "process",
});

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct State {
    pub google_token: Option<String>,
    pub telegram_token: Option<String>,
    pub openai_token: Option<String>,
    pub groq_token: Option<String>,
    pub timezone: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
enum CalendarRequest {
    // forwarded/accepted to/from oauth kinode
    GenerateUrl { target: String },
    Token { token: String },
    AddApis(Tokens),
    // temporary test commands
    GetToday,
    Schedule,
    RefreshToken,
}

#[derive(Debug, Serialize, Deserialize)]
struct Tokens {
    telegram: Option<String>,
    openai: Option<String>,
    groq: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
enum OauthResponse {
    GenerateUrl,
    Url { url: String },
    RefreshToken { token: String },
    Error { error: String },
}

#[derive(Debug, Serialize, Deserialize)]
enum PrimitiveIntent {
    Get,
    Schedule,
}

// for UI?
#[derive(Debug, Serialize, Deserialize)]
enum CalendarResponse {
    State { state: State },
    Error { error: String },
}

pub fn handle_telegram_message(message: &Message, state: &mut State) -> anyhow::Result<()> {
    let Some(token) = &state.google_token else {
        return Err(anyhow::anyhow!("No google token found"));
    };
    let Some(msg) = get_last_tg_msg(&message) else {
        return Ok(());
    };
    let id = msg.chat.id;
    let mut text = msg.text.clone().unwrap_or_default();

    // if voice_message, use STT process to transcribe
    if let Some(voice) = msg.voice.clone() {
        let audio = get_file(&voice.file_id)?;
        text += &get_text(audio)?;
    }

    let llm_answer =
        groq::get_groq_answer(&format!("{} {}", get_default_prompt(&state.timezone), text))?;
    println!("initial answer: {:?}", llm_answer);

    let initial_answer = process_response(token, &llm_answer)?;

    let _message = send_bot_message(&initial_answer, id);
    Ok(())
}

fn process_response(token: &str, response: &str) -> anyhow::Result<String> {
    let cleaned_response = response
        .trim()
        .trim_matches('"')
        .replace("\n", " ")
        .replace("\r", " ");
    println!("Cleaned response: {:?}", cleaned_response);

    if let Some((command, human_like_response)) = cleaned_response.split_once("ENDMARKER") {
        let command = command.trim();
        let human_like_response = human_like_response.trim();

        if command.starts_with("LIST") {
            let parts: Vec<&str> = command.split(',').collect();
            if parts.len() < 4 {
                return Err(anyhow::anyhow!("Invalid LIST command format"));
            }
            let start_date = parts[1].trim();
            let end_date = parts[2].trim();
            let timezone = parts[3].trim();

            let events = get_events_from_primary_calendar(token, start_date, end_date)?;
            let filtered_events = events
                .items
                .iter()
                .map(|e| e.into())
                .collect::<Vec<SimpleEvent>>();
            println!("got some events: {:?}", filtered_events);

            let llm_events =
                groq::get_groq_answer(&format!("{} {:?}", EVENTS_PROMPT, filtered_events))?;

            return Ok(llm_events);
        } else if command.starts_with("SCHEDULE") {
            let parts: Vec<&str> = command.split(',').collect();
            if parts.len() < 5 {
                return Err(anyhow::anyhow!("Invalid SCHEDULE command format"));
            }
            let start = parts[1].trim();
            let end = parts[2].trim();
            let timezone = parts[3].trim();
            let title = parts.get(4).map(|s| s.trim()).unwrap_or("Untitled Event");
            let description = parts
                .get(5)
                .map(|s| s.trim())
                .unwrap_or("No description provided");

            let event = create_event(title, description, start, end, Some(timezone.into()))?;
            schedule_event(token, &event)?;
            return Ok(human_like_response.to_string());
        }
    }

    Ok(response.to_string())
}

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
        } else if incoming.path()? == "/submit_config" {
            let Some(blob) = get_blob() else {
                return Err(anyhow::anyhow!("Failed to get blob"));
            };
            let json = serde_json::from_slice::<serde_json::Value>(&blob.bytes)?;

            let mut tokens = Tokens {
                telegram: json
                    .get("telegram")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string()),
                openai: json
                    .get("openai")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string()),
                groq: json
                    .get("groq")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string()),
            };
            if let Some(telegram_token) = tokens.telegram.take() {
                state.telegram_token = Some(telegram_token.clone());
                init_tg(&telegram_token)?;
                let _ = subscribe();
            }
            if let Some(openai_token) = tokens.openai.take() {
                state.openai_token = Some(openai_token.clone());
                stt::init_stt(&openai_token)?;
            }
            if let Some(groq_token) = tokens.groq.take() {
                state.groq_token = Some(groq_token.clone());
                groq::init_groq(&groq_token)?;
            }

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
        return Ok(());
    }
    let mut tg_address = Address::from(TG_ADDRESS);
    // temp fix, make better
    tg_address.node = our.node.clone();

    if msg.source() == &tg_address {
        handle_telegram_message(&msg, state)?;
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
        CalendarRequest::RefreshToken => {
            let target: Address = Address::new::<String, ProcessId>(
                "fake.dev".to_string(),
                ProcessId::from_str("oauth:ratatouille:template.os")?,
            );

            let resp = Request::new()
                .target(target)
                .body(serde_json::to_vec(&CalendarRequest::RefreshToken)?)
                .send_and_await_response(5)??;

            let res = serde_json::from_slice::<OauthResponse>(resp.body())?;
            println!("got response: {:?}", res);
        }
        CalendarRequest::AddApis(mut tokens) => {
            if let Some(telegram_token) = tokens.telegram.take() {
                state.telegram_token = Some(telegram_token.clone());
                init_tg(&telegram_token)?;
                let _ = subscribe();
            }
            if let Some(openai_token) = tokens.openai.take() {
                state.openai_token = Some(openai_token.clone());
                stt::init_stt(&openai_token)?;
            }
            if let Some(groq_token) = tokens.groq.take() {
                state.groq_token = Some(groq_token.clone());
                groq::init_groq(&groq_token)?;
            }
        }
        CalendarRequest::Token { token } => {
            // todo: verify if it's from the right place too.
            state.google_token = Some(token.clone());
            let timezone = get_timezone(&token)?;
            state.timezone = Some(timezone);
        }
        CalendarRequest::GetToday => {
            if let Some(token) = &state.google_token {
                let (time_min, time_max) = get_time_24h();
                get_events_from_primary_calendar(token, &time_min, &time_max)?;
            }
        }
        CalendarRequest::Schedule => if let Some(token) = &state.google_token {},
    };

    Ok(())
}

call_init!(init);
fn init(our: Address) {
    println!("client begin");

    http::serve_index_html(&our, "client-ui/", true, false, vec!["/"]).unwrap();
    http::bind_http_path("/status", true, false).unwrap();
    http::bind_http_path("/generate", true, false).unwrap();
    http::bind_http_path("/submit_config", true, false).unwrap();

    let mut state = State {
        google_token: None,
        telegram_token: None,
        openai_token: None,
        groq_token: None,
        timezone: None,
    };

    loop {
        match handle_message(&our, &mut state) {
            Ok(_) => {}
            Err(e) => {
                println!("error: {:?}", e);
            }
        };
    }
}
