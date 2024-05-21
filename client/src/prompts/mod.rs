use chrono::{DateTime, Local, SecondsFormat, Utc};
use kinode_process_lib::println;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

pub static DEFAULT_PROMPT: &str = r#"
You are an intelligent assistant that can help with calendar management and general queries. Based on the user's input, respond in the following format and only return the specified format without any additional text or explanations:

1. If the user wants to view events within a date range:
   LIST,start_date_in_YYYY-MM-DDTHH:MM:SS_format,end_date_in_YYYY-MM-DDTHH:MM:SS_format,timezone,ENDMARKER
   Followed by a human-like summary of the events.

2. If the user wants to schedule one or more events:
   SCHEDULE,start_in_YYYY-MM-DDTHH:MM:SS_format,end_in_YYYY-MM-DDTHH:MM:SS_format,timezone,description,title,ENDMARKER
   Followed by a human-like confirmation of the scheduled event.

3. For any other query, provide a helpful and relevant response.

Examples:
- What are my events for next week? -> LIST,2024-05-20T00:00:00Z,2024-05-26T23:59:59Z,America/Los_Angeles,ENDMARKER You have 3 events scheduled from May 20th to May 26th.
- Schedule a team meeting on June 5th at 3 PM for 2 hours. -> SCHEDULE,2024-06-05T15:00:00-07:00,2024-06-05T17:00:00-07:00,America/Los_Angeles,Team meeting,ENDMARKER Your team meeting has been scheduled on June 5th from 3 PM to 5 PM.
- How's the weather today? -> Provide a general response.

User input: 
"#;

pub static DEFAULT_PROMPT_1: &str = r#"
You are an intelligent assistant that can help with calendar management and general queries. The current time is {current_time}. Based on the user's input, respond in the following format and only return the specified format without any additional text or explanations:

1. If the user wants to view events within a date range:
   LIST,start_date_in_YYYY-MM-DDTHH:MM:SS_format,end_date_in_YYYY-MM-DDTHH:MM:SS_format,timezone,ENDMARKER
   Followed by a human-like summary of the events.

2. If the user wants to schedule one or more events:
   SCHEDULE,start_in_YYYY-MM-DDTHH:MM:SS_format,end_in_YYYY-MM-DDTHH:MM:SS_format,timezone,description,title,ENDMARKER
   Followed by a human-like confirmation of the scheduled event.

3. For any other query, provide a helpful and relevant response.

Examples:
- What are my events for next week? -> LIST,2024-05-20T00:00:00Z,2024-05-26T23:59:59Z,America/Los_Angeles,ENDMARKER You have 3 events scheduled from May 20th to May 26th.
- Schedule a team meeting on June 5th at 3 PM for 2 hours. -> SCHEDULE,2024-06-05T15:00:00-07:00,2024-06-05T17:00:00-07:00,America/Los_Angeles,Team meeting,ENDMARKER Your team meeting has been scheduled on June 5th from 3 PM to 5 PM.
- How's the weather today? -> Provide a general response.

User input: 
"#;

pub fn get_default_prompt() -> String {
    let current_time: DateTime<Local> = Local::now();
    let formatted_time = current_time.to_rfc3339_opts(SecondsFormat::Secs, true);
    println!("formatted time!!! {:?}", formatted_time);
    format!(
        r#"
You are an intelligent assistant that can help with calendar management and general queries. The current time is {current_time}. Based on the user's input, respond in the following format and only return the specified format without any additional text or explanations:

1. If the user wants to view events within a date range:
   LIST,start_date_in_YYYY-MM-DDTHH:MM:SS_format,end_date_in_YYYY-MM-DDTHH:MM:SS_format,timezone,ENDMARKER
   Followed by a human-like summary of the events.

2. If the user wants to schedule one or more events:
   SCHEDULE,start_in_YYYY-MM-DDTHH:MM:SS_format,end_in_YYYY-MM-DDTHH:MM:SS_format,timezone,description,title,ENDMARKER
   Followed by a human-like confirmation of the scheduled event.

3. For any other query, provide a helpful and relevant response.

Examples:
- What are my events for next week? -> LIST,2024-05-20T00:00:00Z,2024-05-26T23:59:59Z,America/Los_Angeles,ENDMARKER You have 3 events scheduled from May 20th to May 26th.
- Schedule a team meeting on June 5th at 3 PM for 2 hours. -> SCHEDULE,2024-06-05T15:00:00-07:00,2024-06-05T17:00:00-07:00,America/Los_Angeles,Team meeting,ENDMARKER Your team meeting has been scheduled on June 5th from 3 PM to 5 PM.
- How's the weather today? -> Provide a general response.

User input: 
"#,
        current_time = formatted_time
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

const prompt1: &str = "You are a smart calendar assistant. Your job is to help users manage their schedules by understanding their requests and providing precise calendar actions. You can schedule meetings, list events, and help plan the day efficiently. When a user sends a message, your goal is to interpret their needs and suggest the most relevant calendar actions.";
const prompt2: &str = "Based on the user's request, please clarify the intention using the following formats. If the user wants to schedule one or more events, respond with 'Schedule: [Title], [Description], [Start Time in UTC], [Duration in hours]; ...' for each event. If the user wants to list events for today, respond with 'List: Today'. Ensure your responses are concise and strictly follow these formats for easy parsing by the system.";
const prompt3: &str = "You are an efficient meeting summarizer. Your task is to list the names and times of the next meetings from the user's calendar. Please provide the meeting titles and their start times in UTC, formatted as 'Next Meetings: [Title] at [Start Time in UTC]; ...'. Ensure the response is clear and concise for easy parsing.";

/// Simple buffer for message handling.
///
#[derive(Serialize, Deserialize, Debug, Clone)]
struct Buffer<T> {
    capacity: usize,
    buffer: VecDeque<T>,
}

impl<T> Buffer<T> {
    fn new(capacity: usize) -> Self {
        Buffer {
            capacity,
            buffer: VecDeque::with_capacity(capacity),
        }
    }

    fn push(&mut self, item: T) {
        if self.buffer.len() == self.capacity {
            self.buffer.pop_front();
        }
        self.buffer.push_back(item);
    }
}
