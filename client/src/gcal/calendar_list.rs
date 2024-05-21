use crate::gcal::sendable::{QueryParams, Sendable};
use crate::gcal::DefaultReminder;
use crate::gcal::{CalendarAccessRole, ConferenceProperties};
use serde_derive::{Deserialize, Serialize};

/*
 * from: https://developers.google.com/calendar/api/v3/reference/calendarList#resource
 */

/// CalendarListClient is the method of accessing the calendar list. You must provide it with a
/// Google Calendar client.

fn default_entry_kind() -> Option<String> {
    Some("calendar#calendarListEntry".to_string())
}

fn default_list_kind() -> Option<String> {
    Some("calendar#calendarList".to_string())
}

/// CalendarListItem is a single calendar returned by CalendarList, do not confuse this with
/// Calendar.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CalendarListItem {
    #[serde(default = "default_entry_kind")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
    pub id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub etag: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub location: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub summary_override: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub time_zone: Option<String>,
    pub access_role: CalendarAccessRole,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub background_color: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub foreground_color: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub conference_properties: Option<ConferenceProperties>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deleted: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hidden: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub primary: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub selected: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub notification_settings: Option<NotificationSettings>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub default_reminders: Vec<DefaultReminder>,

    #[serde(skip)]
    query_string: QueryParams,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NotificationSettings {
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub notifications: Vec<NotificationSetting>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NotificationSetting {
    pub method: NotificationSettingMethod,
    #[serde(rename = "type")]
    pub typ: NotificationSettingType,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum NotificationSettingMethod {
    #[serde(rename = "email")]
    EMail,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum NotificationSettingType {
    EventCreation,
    EventChange,
    EventCancellation,
    EventResponse,
    Agenda,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct CalendarList {
    #[serde(default = "default_list_kind")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub etag: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_sync_token: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub items: Vec<CalendarListItem>,

    #[serde(skip)]
    query_string: QueryParams,
}

impl Sendable for CalendarListItem {
    fn path(&self, _action: Option<String>) -> String {
        format!("users/me/calendarList/{}", self.id)
    }

    fn query(&self) -> QueryParams {
        self.query_string.clone()
    }
}

impl Sendable for CalendarList {
    fn path(&self, _action: Option<String>) -> String {
        String::from("users/me/calendarList")
    }

    fn query(&self) -> QueryParams {
        self.query_string.clone()
    }
}
