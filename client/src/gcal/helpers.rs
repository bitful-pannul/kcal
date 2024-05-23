use crate::{gcal::*, groq, prompts::EVENTS_PROMPT};
use chrono::{DateTime, Duration, Utc};
use kinode_process_lib::http;
use std::{collections::HashMap, str::FromStr};
use url::Url;

pub fn create_event(
    summary: &str,
    description: &str,
    start_time: &str,
    end_time: &str,
    timezone: Option<String>,
    attendees: Vec<EventAttendees>,
    meeting: bool,
) -> anyhow::Result<Event> {
    let event_id = rand::random::<u64>().to_string();
    let conference_data = if meeting {
        Some(EventConferenceData {
            create_request: Some(EventCreateConferenceRequest {
                request_id: event_id,
                ..Default::default()
            }),
            ..Default::default()
        })
    } else {
        None
    };

    let attendees = if attendees.is_empty() {
        None
    } else {
        Some(attendees)
    };

    let event = Event {
        summary: Some(summary.to_string()),
        description: Some(description.to_string()),
        start: Some(EventCalendarDate {
            date: None,
            date_time: Some(start_time.to_string()),
            time_zone: timezone.clone(),
        }),
        end: Some(EventCalendarDate {
            date: None,
            date_time: Some(end_time.to_string()),
            time_zone: timezone,
        }),
        attendees,
        conference_data,
        ..Default::default()
    };
    Ok(event)
}

pub fn schedule_event(token: &str, event: &Event, schedule_meeting: bool) -> anyhow::Result<Event> {
    let base_url = "https://www.googleapis.com/calendar/v3/calendars/primary/events";
    let url = if schedule_meeting {
        Url::from_str(&format!("{}?conferenceDataVersion=1", base_url))?
    } else {
        Url::from_str(base_url)?
    };
    let headers = HashMap::from([
        ("Authorization".to_string(), format!("Bearer {}", token)),
        ("Content-Type".to_string(), "application/json".to_string()),
    ]);

    let body = serde_json::to_vec(event)?;
    let res = http::send_request_await_response(http::Method::POST, url, Some(headers), 30, body)?;

    if res.status().is_success() {
        let event: Event = serde_json::from_slice(&res.body())?;
        Ok(event)
    } else {
        let err_msg = format!(
            "Failed to create event: {}",
            String::from_utf8_lossy(res.body())
        );
        Err(anyhow::anyhow!(err_msg))
    }
}

pub fn process_schedule_request(token: &str, response: &str) -> anyhow::Result<String> {
    let cleaned_response = response
        .trim()
        .trim_matches('"')
        .replace("\\n", " ")
        .replace("\\r", " ");

    if let Some((command, human_like_response)) = cleaned_response.split_once("ENDMARKER") {
        let command = command.trim();
        let human_like_response = human_like_response.trim();

        if command.starts_with("SCHEDULE_REQUEST") {
            let parts: Vec<&str> = command.split(',').collect();
            if parts.len() < 6 {
                return Err(anyhow::anyhow!("Invalid SCHEDULE_REQUEST command format"));
            }

            let start = parts[1].trim();
            let end = parts[2].trim();
            let title = parts[3].trim();
            let description = parts[4].trim();

            let event = create_event(
                title,
                description,
                start,
                end,
                None,
                vec![], // todo email parsing with longer context.
                true,
            )?;
            let event = schedule_event(token, &event, true)?;
            if let Some(meet) = event.hangout_link {
                return Ok(format!("{}, link: {}", human_like_response, meet));
            }
            return Ok(human_like_response.to_string());
        } else if command.starts_with("INCOMPLETE_REQUEST") {
            let parts: Vec<&str> = command.split(',').collect();
            if parts.len() < 2 {
                return Err(anyhow::anyhow!("Invalid INCOMPLETE_REQUEST command format"));
            }

            let missing_info = parts[1].trim();
            return Ok(format!(
                "Incomplete request. Please provide the following missing information: {}",
                missing_info
            ));
        } else if command.starts_with("REJECTED_REQUEST") {
            let parts: Vec<&str> = command.split(',').collect();
            if parts.len() < 2 {
                return Err(anyhow::anyhow!("Invalid REJECTED_REQUEST command format"));
            }

            let reason = parts[1].trim();
            return Ok(format!("Request rejected. Reason: {}", reason));
        }
    }

    Ok(response.to_string())
}

pub fn process_response(token: &str, response: &str) -> anyhow::Result<String> {
    let cleaned_response = response
        .trim()
        .trim_matches('"')
        .replace("\n", " ")
        .replace("\r", " ");

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
            let _timezone = parts[3].trim();

            let events = get_events_from_primary_calendar(token, start_date, end_date)?;
            let filtered_events = events
                .items
                .iter()
                .map(|e| e.into())
                .collect::<Vec<SimpleEvent>>();

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

            let attendees = if let Some(attendees_str) = parts.get(6) {
                attendees_str
                    .trim_matches(&['[', ']'][..])
                    .split(',')
                    .filter(|s| !s.trim().is_empty())
                    .map(|email| EventAttendees {
                        email: email.trim().to_string(),
                        ..Default::default()
                    })
                    .collect::<Vec<_>>()
            } else {
                vec![]
            };

            let meeting = !attendees.is_empty();

            let event = create_event(
                title,
                description,
                start,
                end,
                Some(timezone.into()),
                attendees,
                meeting,
            )?;
            schedule_event(token, &event, meeting)?;
            return Ok(human_like_response.to_string());
        }
    }

    Ok(response.to_string())
}

pub fn get_events_from_primary_calendar(
    token: &str,
    time_min: &str,
    time_max: &str,
) -> anyhow::Result<Events> {
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

    let events: Events = serde_json::from_slice(&res.body())?;

    Ok(events)
}

pub fn _get_time_24h() -> (String, String) {
    let now: DateTime<Utc> = Utc::now();
    let time_min = now.format("%Y-%m-%dT%H:%M:%SZ").to_string(); // UTC time, no milliseconds
    let time_max = (now + Duration::hours(24))
        .format("%Y-%m-%dT%H:%M:%SZ")
        .to_string(); // 24 hours from now, UTC time, no milliseconds

    (time_min, time_max)
}

pub fn _get_primary_calendar(token: &str) -> anyhow::Result<calendar::Calendar> {
    let url = Url::from_str("https://www.googleapis.com/calendar/v3/calendars/primary").unwrap();

    let headers = HashMap::from([
        ("Authorization".to_string(), format!("Bearer {}", token)),
        ("Content-Type".to_string(), "application/json".to_string()),
    ]);

    let res = http::send_request_await_response(http::Method::GET, url, Some(headers), 5, vec![])?;
    let cal: calendar::Calendar = serde_json::from_slice(&res.body())?;
    Ok(cal)
}

pub fn get_timezone(token: &str) -> anyhow::Result<String> {
    let url = Url::from_str("https://www.googleapis.com/calendar/v3/users/me/settings/timezone")?;

    let headers = HashMap::from([
        ("Authorization".to_string(), format!("Bearer {}", token)),
        ("Content-Type".to_string(), "application/json".to_string()),
    ]);

    let res = http::send_request_await_response(http::Method::GET, url, Some(headers), 5, vec![])?;
    let json: serde_json::Value = serde_json::from_slice(&res.body())?;

    let timezone = json
        .get("value")
        .ok_or(anyhow::anyhow!("No timezone found"))?
        .as_str()
        .ok_or(anyhow::anyhow!("Invalid timezone format"))?;

    Ok(timezone.to_string())
}
