use chrono::{DateTime, SecondsFormat, Utc};
use chrono_tz::Tz;

pub fn get_default_prompt(timezone: &Option<String>) -> String {
    let tz: Tz = timezone
        .as_deref()
        .unwrap_or("UTC")
        .parse()
        .unwrap_or(Tz::UTC);

    let current_utc_time: DateTime<Utc> = Utc::now();
    let current_local_time = current_utc_time.with_timezone(&tz);

    let formatted_utc_time = current_utc_time.to_rfc3339_opts(SecondsFormat::Secs, true);
    let formatted_local_time = current_local_time.to_rfc3339_opts(SecondsFormat::Secs, true);

    format!(
        r#"
You are an intelligent assistant that can help with calendar management and general queries. The current UTC time is {utc_time}. The current local time is {local_time} in {timezone}. Based on the user's input, interpret times in the user's local time but output all times in UTC. Respond in the following format and only return the specified format without any additional text or explanations:

1. If the user wants to view events within a date range:
   LIST,start_date_in_YYYY-MM-DDTHH:MM:SSZ_format,end_date_in_YYYY-MM-DDTHH:MM:SSZ_format,UTC,ENDMARKER
   Followed by a human-like summary of the events.

2. If the user wants to schedule one or more events:
   SCHEDULE,start_in_YYYY-MM-DDTHH:MM:SSZ_format,end_in_YYYY-MM-DDTHH:MM:SSZ_format,UTC,title,description,[attendees],ENDMARKER
   Attendees should be in the format [email1,email2,email3]. Followed by a human-like confirmation of the scheduled event.

3. For any other query, provide a helpful and relevant response.

Examples:
- What are my events for next week? -> LIST,2024-05-20T00:00:00Z,2024-05-26T23:59:59Z,UTC,ENDMARKER You have 3 events scheduled from May 20th to May 26th.
- Schedule a team meeting on June 5th at 3 PM for 2 hours. -> SCHEDULE,2024-06-05T15:00:00Z,2024-06-05T17:00:00Z,UTC,Team meeting,meeting with the team,[],ENDMARKER Your team meeting has been scheduled on June 5th from 3 PM to 5 PM.
- Schedule a project meeting on June 7th at 10 AM for 1 hour with john@gmail.com. -> SCHEDULE,2024-06-07T10:00:00Z,2024-06-07T11:00:00Z,UTC,Project meeting,meeting to discuss project,[john@gmail.com],ENDMARKER Your project meeting has been scheduled on June 7th from 10 AM to 11 AM with john@gmail.com.
- How's the weather today? -> Provide a general response.

User input: 
"#,
        utc_time = formatted_utc_time,
        local_time = formatted_local_time,
        timezone = tz
    )
}

pub fn get_old_default_prompt(timezone: &Option<String>) -> String {
    let tz: Tz = timezone
        .as_deref()
        .unwrap_or("UTC")
        .parse()
        .unwrap_or(Tz::UTC);

    let current_utc_time: DateTime<Utc> = Utc::now();
    let current_local_time = current_utc_time.with_timezone(&tz);

    let formatted_utc_time = current_utc_time.to_rfc3339_opts(SecondsFormat::Secs, true);
    let formatted_local_time = current_local_time.to_rfc3339_opts(SecondsFormat::Secs, true);

    format!(
        r#"
You are an intelligent assistant that can help with calendar management and general queries. The current UTC time is {utc_time}. The current local time is {local_time} in {timezone}. Based on the user's input, interpret times in the user's local time but output all times in UTC. Respond in the following format and only return the specified format without any additional text or explanations:

1. If the user wants to view events within a date range:
   LIST,start_date_in_YYYY-MM-DDTHH:MM:SSZ_format,end_date_in_YYYY-MM-DDTHH:MM:SSZ_format,UTC,ENDMARKER
   Followed by a human-like summary of the events.

2. If the user wants to schedule one or more events:
   SCHEDULE,start_in_YYYY-MM-DDTHH:MM:SSZ_format,end_in_YYYY-MM-DDTHH:MM:SSZ_format,UTC,title,description,ENDMARKER
   Followed by a human-like confirmation of the scheduled event.

3. For any other query, provide a helpful and relevant response.

Examples:
- What are my events for next week? -> LIST,2024-05-20T00:00:00Z,2024-05-26T23:59:59Z,UTC,ENDMARKER You have 3 events scheduled from May 20th to May 26th.
- Schedule a team meeting on June 5th at 3 PM for 2 hours. -> SCHEDULE,2024-06-05T15:00:00Z,2024-06-05T17:00:00Z,UTC,Team meeting,meeting with the team,ENDMARKER Your team meeting has been scheduled on June 5th from 3 PM to 5 PM.
- How's the weather today? -> Provide a general response.

User input: 
"#,
        utc_time = formatted_utc_time,
        local_time = formatted_local_time,
        timezone = tz
    )
}

pub static EVENTS_PROMPT: &str = r#"
You are an intelligent assistant that helps with calendar management. Given a list of events, format them in a friendly and readable manner. Each event has the following details:
- Title
- Description (optional)
- Start time (in YYYY-MM-DDTHH:MM:SS format)
- End time (in YYYY-MM-DDTHH:MM:SS format)

Format the events in a way that is easy to read and understand for the user.

Examples:
- Given the events:
  [
    {{
      "title": "Team Meeting",
      "description": "Discuss project updates",
      "start_time": "2024-06-05T15:00:00Z",
      "end_time": "2024-06-05T16:00:00Z"
    }},
    {{
      "title": "One-on-One",
      "description": "Performance review",
      "start_time": "2024-06-05T17:00:00Z",
      "end_time": "2024-06-05T17:30:00Z"
    }}
  ]
  - You should respond with:
    "You have the following events:
    1. "Team Meeting" on 2024-06-05 from 15:00 to 16:00. Description: Discuss project updates.
    2. "One-on-One" on 2024-06-05 from 17:00 to 17:30. Description: Performance review."

Format the following events:
"#;

// Simple buffer for message handling.
// TODO
// #[derive(Serialize, Deserialize, Debug, Clone)]
// struct Buffer<T> {
//     capacity: usize,
//     buffer: VecDeque<T>,
// }

// impl<T> Buffer<T> {
//     fn new(capacity: usize) -> Self {
//         Buffer {
//             capacity,
//             buffer: VecDeque::with_capacity(capacity),
//         }
//     }

//     fn push(&mut self, item: T) {
//         if self.buffer.len() == self.capacity {
//             self.buffer.pop_front();
//         }
//         self.buffer.push_back(item);
//     }
// }
