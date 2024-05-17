use crate::gcal::*;
use chrono::{DateTime, Duration, Utc};
use kinode_process_lib::http;
use std::{collections::HashMap, str::FromStr};
use url::Url;

pub fn create_event(
    summary: &str,
    description: &str,
    start_time: DateTime<Utc>,
    duration_hours: i64,
) -> anyhow::Result<Event> {
    let end_time = start_time + Duration::hours(duration_hours);

    let event = Event {
        summary: Some(summary.to_string()),
        description: Some(description.to_string()),
        start: Some(EventCalendarDate {
            date: None,
            date_time: Some(start_time.to_rfc3339()),
            time_zone: None,
        }),
        end: Some(EventCalendarDate {
            date: None, // Set to None unless it's an all-day event
            date_time: Some(end_time.to_rfc3339()),
            time_zone: None,
        }),
        ..Default::default()
    };
    Ok(event)
}

pub fn add_event_to_calendar(token: &str, event: &Event) -> anyhow::Result<()> {
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

pub fn fetch_events_from_primary_calendar(
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

    // pure json to_string might be better to give to model
    let _json: serde_json::Value = serde_json::from_slice(&res.body())?;

    let events: Events = serde_json::from_slice(&res.body())?;

    Ok(events)
}

pub fn get_time_24h() -> (String, String) {
    let now: DateTime<Utc> = Utc::now();
    let time_min = now.format("%Y-%m-%dT%H:%M:%SZ").to_string(); // UTC time, no milliseconds
    let time_max = (now + Duration::hours(24))
        .format("%Y-%m-%dT%H:%M:%SZ")
        .to_string(); // 24 hours from now, UTC time, no milliseconds

    (time_min, time_max)
}

// NOTE: using "primary" calendar instead for now
// pub fn list_calendars(token: &str) -> anyhow::Result<()> {
//     let url =
//         Url::from_str("https://www.googleapis.com/calendar/v3/users/me/calendarList").unwrap();

//     let headers = HashMap::from([
//         ("Authorization".to_string(), format!("Bearer {}", token)),
//         ("Content-Type".to_string(), "application/json".to_string()),
//     ]);
//     let body = Vec::new(); // No body for GET request

//     let res = http::send_request_await_response(http::Method::GET, url, Some(headers), 5, body)?;
//     let json: serde_json::Value = serde_json::from_slice(&res.body())?;
//     Ok(())
// }
