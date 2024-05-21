use crate::gcal::*;
use chrono::{DateTime, Duration, Utc};
use core::time;
use kinode_process_lib::http;
use std::{collections::HashMap, str::FromStr};
use url::Url;

pub fn create_event(
    summary: &str,
    description: &str,
    start_time: &str,
    end_time: &str,
    timezone: Option<String>,
) -> anyhow::Result<Event> {
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
        ..Default::default()
    };
    Ok(event)
}

pub fn schedule_event(token: &str, event: &Event) -> anyhow::Result<()> {
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

pub fn get_time_24h() -> (String, String) {
    let now: DateTime<Utc> = Utc::now();
    let time_min = now.format("%Y-%m-%dT%H:%M:%SZ").to_string(); // UTC time, no milliseconds
    let time_max = (now + Duration::hours(24))
        .format("%Y-%m-%dT%H:%M:%SZ")
        .to_string(); // 24 hours from now, UTC time, no milliseconds

    (time_min, time_max)
}

pub fn get_primary_calendar(token: &str) -> anyhow::Result<calendar::Calendar> {
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
    let primary_cal = get_primary_calendar(token)?;
    Ok(primary_cal.time_zone)
}
