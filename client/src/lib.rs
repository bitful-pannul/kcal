use std::collections::HashMap;
use std::str::FromStr;

use chrono::{Duration, Utc};
use kinode_process_lib::ProcessId;
use kinode_process_lib::{
    await_message, call_init, get_blob, http, http::send_response, println, Address, Message,
    Request,
};

use serde::{Deserialize, Serialize};

mod gcal;
mod groq;
mod stt;
mod tg;

use gcal::helpers::*;
use tg::*;

pub const LLM_ADDRESS: (&str, &str, &str, &str) = ("our", "openai", "ratatouille", "template.os");
pub const TG_ADDRESS: (&str, &str, &str, &str) = ("our", "tg", "ratatouille", "template.os");
pub const STT_ADDRESS: (&str, &str, &str, &str) =
    ("our", "speech_to_text", "ratatouille", "template.os");
// todo command_center extensibility!

wit_bindgen::generate!({
    path: "wit",
    world: "process",
});

const prompt1: &str = "You are a smart calendar assistant. Your job is to help users manage their schedules by understanding their requests and providing precise calendar actions. You can schedule meetings, list events, and help plan the day efficiently. When a user sends a message, your goal is to interpret their needs and suggest the most relevant calendar actions.";
const prompt2: &str = "Based on the user's request, please clarify the intention using the following formats. If the user wants to schedule one or more events, respond with 'Schedule: [Title], [Description], [Start Time in UTC], [Duration in hours]; ...' for each event. If the user wants to list events for today, respond with 'List: Today'. Ensure your responses are concise and strictly follow these formats for easy parsing by the system.";
const prompt3: &str = "You are an efficient meeting summarizer. Your task is to list the names and times of the next meetings from the user's calendar. Please provide the meeting titles and their start times in UTC, formatted as 'Next Meetings: [Title] at [Start Time in UTC]; ...'. Ensure the response is clear and concise for easy parsing.";

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct State {
    pub google_token: Option<String>,
    pub telegram_token: Option<String>,
    pub openai_token: Option<String>,
    pub groq_token: Option<String>,
    // expiry logic here or server? or fetch upon error?
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
    let Some(msg) = get_last_tg_msg(&message) else {
        return Ok(());
    };
    let id = msg.chat.id;
    let mut text = msg.text.clone().unwrap_or_default();

    if let Some(voice) = msg.voice.clone() {
        let audio = get_file(&voice.file_id)?;
        text += &get_text(audio)?;
    }

    let initial_answer = groq::get_groq_answer(&format!("{} {}", prompt1, text))?;
    println!("initial answer: {:?}", initial_answer);
    let second_answer = groq::get_groq_answer(&format!("{} {}", prompt2, text))?;
    println!("second answer: {:?}", second_answer);
    match parse_intent(&second_answer) {
        Ok(intent) => match intent {
            PrimitiveIntent::Get => {
                if let Some(token) = &state.google_token {
                    let (time_min, time_max) = get_time_24h();
                    let events = fetch_events_from_primary_calendar(token, &time_min, &time_max)?;
                    let json_string = serde_json::to_string(&events)?;

                    let answer = groq::get_groq_answer(&format!("{} {}", prompt3, json_string))?;
                    let _message = send_bot_message(&answer, id);
                    return Ok(());
                }
            }
            PrimitiveIntent::Schedule => {
                if let Some(token) = &state.google_token {
                    let start_time = Utc::now() + Duration::hours(2);
                    let event = create_event("Coffee with Natasha", "", start_time, 1)?;
                    add_event_to_calendar(token, &event)?;
                    let _message = send_bot_message("Event scheduled successfully", id);
                    return Ok(());
                }
            }
        },
        Err(e) => {}
    }

    // let answer = get_groq_answer(&text)?;
    let _message = send_bot_message(&initial_answer, id);
    Ok(())
}

fn parse_intent(response: &str) -> anyhow::Result<PrimitiveIntent> {
    // Logic to parse the initial LLM response to determine the intent
    // temp mocked
    if response.contains("schedule") {
        Ok(PrimitiveIntent::Schedule)
    } else if response.contains("list") {
        Ok(PrimitiveIntent::Get)
    } else {
        Err(anyhow::anyhow!("Unrecognized intent"))
    }
}

// async fn handle_llm_response(response: String, state: &mut State) -> anyhow::Result<()> {
//     if response.starts_with("Schedule:") {
//         let events_details = response.trim_start_matches("Schedule: ").split(';');
//         for details in events_details {
//             let event_parts = details.split(',').collect::<Vec<_>>();
//             if event_parts.len() == 4 {
//                 let title = event_parts[0].trim();
//                 let description = event_parts[1].trim();
//                 let start_time =
//                     Utc.datetime_from_str(event_parts[2].trim(), "%Y-%m-%dT%H:%M:%SZ")?;
//                 let duration = event_parts[3].trim().parse::<i64>()?;
//                 let event =
//                     gcal::helpers::schedule_event(title, description, start_time, duration)?;
//                 gcal::helpers::add_event_to_calendar(&state.google_token.unwrap(), &event)?;
//             }
//         }
//     } else if response == "List: Today" {
//         let (time_min, time_max) = gcal::helpers::get_time_24h();
//         let events = gcal::helpers::fetch_events_from_primary_calendar(
//             &state.google_token.unwrap(),
//             &time_min,
//             &time_max,
//         )?;
//         // Send events back to user or handle them as needed
//     }
//     Ok(())
// }

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
            state.google_token = Some(token);
        }
        CalendarRequest::GetToday => {
            if let Some(token) = &state.google_token {
                let (time_min, time_max) = get_time_24h();
                fetch_events_from_primary_calendar(token, &time_min, &time_max)?;
            }
        }
        CalendarRequest::Schedule => {
            if let Some(token) = &state.google_token {
                let event = create_event("Test Event", "This is a test event", Utc::now(), 1)?;
                add_event_to_calendar(token, &event)?;
            }
        }
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
