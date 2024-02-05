use serde::{Deserialize, Serialize};
// use serde_json::Value;

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Root {
    #[serde(rename = "@odata.context")]
    pub odata_context: Option<String>,
    pub value: Vec<Value>,
    #[serde(rename = "@odata.nextLink")]
    pub odata_next_link: Option<String>,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Value {
    #[serde(rename = "@odata.etag")]
    pub odata_etag: Option<String>,
    pub id: Option<String>,
    pub created_date_time: Option<String>,
    pub last_modified_date_time: Option<String>,
    pub change_key: Option<String>,
    pub categories: Vec<Option<String>>,
    pub transaction_id: Option<Option<String>>,
    pub original_start_time_zone: Option<String>,
    pub original_end_time_zone: Option<String>,
    #[serde(rename = "iCalUId")]
    pub i_cal_uid: Option<String>,
    pub reminder_minutes_before_start: i64,
    pub is_reminder_on: bool,
    pub has_attachments: bool,
    pub subject: Option<String>,
    pub body_preview: Option<String>,
    pub importance: Option<String>,
    pub sensitivity: Option<String>,
    pub is_all_day: bool,
    pub is_cancelled: bool,
    pub is_organizer: bool,
    pub response_requested: bool,
    pub series_master_id: Option<Option<String>>,
    pub show_as: Option<String>,
    #[serde(rename = "type")]
    pub type_field: Option<String>,
    pub web_link: Option<String>,
    pub online_meeting_url: Option<Option<String>>,
    pub is_online_meeting: bool,
    pub online_meeting_provider: Option<String>,
    pub allow_new_time_proposals: bool,
    pub occurrence_id: Option<String>,
    pub is_draft: bool,
    pub hide_attendees: bool,
    pub response_status: ResponseStatus,
    pub body: Option<Body>,
    pub start: Start,
    pub end: End,
    pub location: Option<Location>,
    pub locations: Vec<Location2>,
    pub recurrence: Option<Recurrence>,
    pub attendees: Vec<Attendee>,
    pub organizer: Organizer,
    pub online_meeting: Option<OnlineMeeting>,
    #[serde(rename = "calendar@odata.associationLink")]
    pub calendar_odata_association_link: Option<String>,
    #[serde(rename = "calendar@odata.navigationLink")]
    pub calendar_odata_navigation_link: Option<String>,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResponseStatus {
    pub response: Option<String>,
    pub time: Option<String>,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Body {
    pub content_type: Option<String>,
    pub content: Option<String>,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Start {
    pub date_time: Option<String>,
    pub time_zone: Option<String>,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct End {
    pub date_time: Option<String>,
    pub time_zone: Option<String>,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Location {
    pub display_name: Option<Option<String>>,
    pub location_type: Option<Option<String>>,
    pub unique_id: Option<Option<String>>,
    pub unique_id_type: Option<Option<String>>,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Location2 {
    pub display_name: Option<String>,
    pub location_type: Option<String>,
    pub unique_id: Option<String>,
    pub unique_id_type: Option<String>,
    pub location_uri: Option<Option<String>>,
    pub address: Option<Address>,
    pub coordinates: Option<Coordinates>,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Address {
    pub street: Option<String>,
    pub city: Option<String>,
    pub state: Option<String>,
    pub country_or_region: Option<String>,
    pub postal_code: Option<String>,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Coordinates {}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Recurrence {
    pub pattern: Pattern,
    pub range: Range,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Pattern {
    #[serde(rename = "type")]
    pub type_field: Option<String>,
    pub interval: i64,
    pub month: i64,
    pub day_of_month: i64,
    pub days_of_week: Vec<Option<String>>,
    pub first_day_of_week: Option<String>,
    pub index: Option<String>,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Range {
    #[serde(rename = "type")]
    pub type_field: Option<String>,
    pub start_date: Option<String>,
    pub end_date: Option<String>,
    pub recurrence_time_zone: Option<String>,
    pub number_of_occurrences: i64,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Attendee {
    #[serde(rename = "type")]
    pub type_field: Option<String>,
    pub status: Status,
    pub email_address: EmailAddress,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Status {
    pub response: Option<String>,
    pub time: Option<String>,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EmailAddress {
    pub name: Option<String>,
    pub address: Option<String>,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Organizer {
    pub email_address: EmailAddress2,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EmailAddress2 {
    pub name: Option<String>,
    pub address: Option<String>,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OnlineMeeting {
    pub join_url: Option<String>,
    pub quick_dial: Option<Option<String>>,
}
