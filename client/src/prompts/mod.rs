use chrono::{DateTime, SecondsFormat, Utc};
use chrono_tz::Tz;
use kinode_process_lib::Address;

pub fn get_default_prompt(timezone: &Option<String>) -> String {
    let tz: Tz = timezone
        .as_deref()
        .unwrap_or("UTC")
        .parse()
        .unwrap_or(Tz::UTC);

    let current_utc_time: DateTime<Utc> = Utc::now();
    let formatted_utc_time = current_utc_time.to_rfc3339_opts(SecondsFormat::Secs, true);

    format!(
        r#"
You are an intelligent assistant that can help with calendar management and general queries.
The current UTC time is {utc_time}.

Instructions:
1. Parse the user's input to understand their intent and extract relevant information.
2. If the user provides any time-related information, assume it is in their local time zone "{timezone}".
3. Convert all time-related information from the user's local time zone to UTC.
4. Format all times in the YYYY-MM-DDTHH:MM:SSZ format.

Respond in the following format and only return the specified format without any additional text or explanations:

1. If the user wants to view events within a date range:
LIST,start_date_in_YYYY-MM-DDTHH:MM:SSZ_format,end_date_in_YYYY-MM-DDTHH:MM:SSZ_format,UTC,ENDMARKER
Followed by a human-like summary of the events.

2. If the user wants to schedule an event:
SCHEDULE,start_in_YYYY-MM-DDTHH:MM:SSZ_format,end_in_YYYY-MM-DDTHH:MM:SSZ_format,UTC,title,description,[attendees],ENDMARKER
Attendees should be in the format [email1,email2,email3].
Followed by a human-like confirmation of the scheduled event.

3. For any other query, provide a helpful and relevant response.

Assuming the current date is Wednesday, May 22, 2024, and the user's timezone is "America/Los_Angeles", here are some examples:

Input: What's on my calendar for next week?
Output:
LIST,2024-05-27T00:00:00Z,2024-06-02T23:59:59Z,UTC,ENDMARKER
You have 3 events scheduled from May 27th to June 2nd.

Input: Schedule a dentist appointment today at 8pm.
Output:
SCHEDULE,2024-05-23T03:00:00Z,2024-05-23T04:00:00Z,UTC,Dentist Appointment,Regular checkup,[],ENDMARKER
Your dentist appointment has been scheduled for today at 8:00 PM.

User input:
"#,
        utc_time = formatted_utc_time,
        timezone = tz,
    )
}

pub fn get_schedule_prompt(our: &Address, timezone: &Option<String>) -> String {
    let tz: Tz = timezone
        .as_deref()
        .unwrap_or("UTC")
        .parse()
        .unwrap_or(Tz::UTC);

    let current_utc_time: DateTime<Utc> = Utc::now();
    let formatted_utc_time = current_utc_time.to_rfc3339_opts(SecondsFormat::Secs, true);

    let our_node = &our.node;
    format!(
        r#"
        You are an AI assistant helping people schedule events with {our_node}. The current UTC time is {utc_time}, and the user's time zone is "{timezone}".
        Instructions:
        
        Parse the user's input to understand their intent and extract relevant information.
        If the user provides any time-related information, assume it is in their local time zone "{timezone}".
        Convert all time-related information from the user's local time zone to UTC.
        Format all times in the YYYY-MM-DDTHH:MM:SSZ format.
        
        
        Respond in the following format without any additional text or explanations:
        
        If the request is valid and complete:
        SCHEDULE_REQUEST,YYYY-MM-DDTHH:MM:SSZ,YYYY-MM-DDTHH:MM:SSZ,Title,Description,ENDMARKER
        Followed by a human-like confirmation of the scheduled event.
        If the request is incomplete:
        INCOMPLETE_REQUEST,[Specify the missing information],ENDMARKER
        If the request appears to be spam or contains offensive language:
        REJECTED_REQUEST,[Reason for rejection],ENDMARKER
        For any other query, provide a helpful and relevant response, including the user's time zone if applicable.
        
        Assuming the current date is Wednesday, May 22, 2024, and the user's timezone is "America/Los_Angeles", here are some examples:
        Input: I'd like to schedule a meeting, on June 5, 2024, at 2:00 PM EST for 60 minutes. My name is John Doe.
        Output:
        SCHEDULE_REQUEST,2024-06-05T18:00:00Z,2024-06-05T19:00:00Z,Meeting with John Doe,meet John Doe,ENDMARKER
        Your meeting with {our_node} has been scheduled for June 5, 2024, at 2:00 PM EST (11:00 AM PST).
        Input: I want to meet with {our_node} next week.
        Output:
        INCOMPLETE_REQUEST,Please provide the following missing information: event title, proposed date and time, and your name.,ENDMARKER
        Input: What's my local time?
        Output: Your local time zone is America/Los_Angeles.
        User input:
        "#,
        utc_time = formatted_utc_time,
        timezone = tz,
        our_node = our_node,
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
