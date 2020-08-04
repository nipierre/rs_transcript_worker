use mcai_worker_sdk::{Body, Div, Head, Paragraph, Span, TimeExpression, TimeUnit, Ttml};
use std::{
  convert::TryFrom,
  io::{Error, ErrorKind},
};
use tokio_tungstenite::tungstenite::protocol::Message;

#[derive(Debug, Deserialize)]
pub struct WebsocketResponse {
  pub message: String,
  pub id: Option<String>,
  #[serde(rename = "type")]
  pub kind: Option<String>,
  pub quality: Option<String>,
  pub reason: Option<String>,
  pub metadata: Option<Metadata>,
}

impl TryFrom<Message> for WebsocketResponse {
  type Error = std::io::Error;
  fn try_from(value: Message) -> Result<Self, Self::Error> {
    if let Message::Text(text) = value {
      serde_json::from_str(&text).map_err(|e| Error::new(ErrorKind::InvalidData, e.to_string()))
    } else {
      Err(Error::new(ErrorKind::InvalidData, "bad message format"))
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
  pub fn generate_ttml(&self, time_offset: Option<f32>, sequence_number: usize) -> Ttml {
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

    Ttml {
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
