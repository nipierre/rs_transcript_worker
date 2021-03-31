use mcai_worker_sdk::prelude::*;
use std::convert::TryFrom;
use tokio_tungstenite::tungstenite::protocol::Message;

#[derive(Debug, Serialize, Deserialize)]
pub struct WebsocketResponse {
  pub message: String,
  pub id: Option<String>,
  #[serde(rename = "type")]
  pub kind: Option<String>,
  pub quality: Option<String>,
  pub reason: Option<String>,
  pub metadata: Option<Metadata>,
  pub results: Option<Vec<Results>>,
}

impl TryFrom<Message> for WebsocketResponse {
  type Error = MessageError;
  fn try_from(value: Message) -> Result<Self> {
    if let Message::Text(text) = value {
      serde_json::from_str(&text)
        .map_err(|e| MessageError::RuntimeError(format!("Invalid data: {}", e.to_string())))
    } else {
      Err(MessageError::RuntimeError(format!("Bad message format")))
    }
  }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Metadata {
  pub start_time: f64,
  pub end_time: f64,
  pub transcript: String,
}

impl Metadata {
  pub fn generate_ttml(&self, time_offset: Option<f32>, sequence_number: usize) -> EbuTtmlLive {
    let begin = Some(TimeExpression::OffsetTime {
      offset: time_offset.unwrap_or(0f32) + self.start_time as f32,
      unit: TimeUnit::Milliseconds,
    });

    let end = Some(TimeExpression::OffsetTime {
      offset: time_offset.unwrap_or(0f32) + self.end_time as f32,
      unit: TimeUnit::Milliseconds,
    });

    let spans = vec![Span {
      content: self.transcript.clone(),
    }];

    let body = Body {
      divs: vec![Div {
        paragraphs: vec![Paragraph {
          spans,
          begin,
          end,
          ..Default::default()
        }],
      }],
      ..Default::default()
    };

    EbuTtmlLive {
      language: Some("fr-FR".to_string()),
      sequence_identifier: Some("LiveSubtitle".to_string()),
      sequence_number: Some(sequence_number as u64),
      clock_mode: Some("local".to_string()),
      time_base: Some("clock".to_string()),
      head: Head::default(),
      body,
    }
  }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Results {
  pub alternatives: Vec<Alternatives>,
  pub start_time: f64,
  pub end_time: f64,
  #[serde(rename = "type")]
  pub kind: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Alternatives {
  pub confidence: f64,
  pub content: String,
}
