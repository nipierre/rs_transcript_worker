#[derive(Debug, Deserialize)]
pub struct AuthotLiveInformation {
  pub id: Option<u32>,
  pub language: Option<String>,
  pub language_to: Option<String>,
  pub message: Option<String>,
  pub status: u8,
  pub stream_state: i32,
  pub stream_status: Option<String>,
  pub translation: Option<bool>,
  pub url: Option<String>,
}
