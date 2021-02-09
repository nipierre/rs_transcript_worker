#[macro_use]
extern crate serde_derive;
#[macro_use]
extern crate serde_json;

use authot::websocket_response::WebsocketResponse;
use futures::channel::mpsc::{channel, Sender};
use futures_util::{future, pin_mut, StreamExt};
use mcai_worker_sdk::{
    prelude::*, job::JobResult, MessageError, MessageEvent,
};
use stainless_ffmpeg_sys::AVMediaType;
use std::{
  convert::TryFrom,
  sync::{
    atomic::{
      AtomicUsize,
      Ordering::{Acquire, Release},
    },
    mpsc::{Sender as StdSender}, 
    Arc, Mutex,
  },
  thread,
  thread::JoinHandle,
};
use tokio::runtime::Runtime;
use tokio_tungstenite::tungstenite::protocol::Message;

mod authot;

pub mod built_info {
  include!(concat!(env!("OUT_DIR"), "/built.rs"));
}

#[derive(Debug, Default)]
struct TranscriptEvent {
  sequence_number: u64,
  start_time: Option<f32>,
  authot_live_id: Option<usize>,
  audio_source_sender: Option<Arc<Mutex<Sender<Message>>>>,
  sender: Option<Arc<Mutex<StdSender<ProcessResult>>>>,
  ws_thread: Option<JoinHandle<()>>,
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub struct WorkerParameters {
  /// # Authot Live Identifier
  /// Pass an pre-created Authot Live process identifier.
  /// It will skip the creation of a new live process.
  authot_live_id: Option<u32>,
  /// # Authot token
  /// Set the token to access to Authot service.
  authot_token: Option<String>,
  /// # Custom vocabulary
  /// Extend the knowledge of the provider by adding some specific words.
  custom_vocabulary: Option<Vec<String>>,
  /// # Provider
  /// Name of the provider used for the transcription
  provider: String,
  destination_path: String,
  source_path: String,
}

impl MessageEvent<WorkerParameters> for TranscriptEvent {
  fn get_name(&self) -> String {
    "Transcript process worker".to_string()
  }

  fn get_short_description(&self) -> String {
    "Worker to process Transcript using different providers".to_string()
  }

  fn get_description(&self) -> String {
    r#"This worker applies Transcript from audio source into TTML stream."#.to_string()
  }

  fn get_version(&self) -> Version {
    Version::parse(built_info::PKG_VERSION).expect("unable to locate Package version")
  }

  fn init_process(
    &mut self,
    parameters: WorkerParameters,
    format_context: Arc<Mutex<FormatContext>>,
    response_sender: Arc<Mutex<StdSender<ProcessResult>>>,
  ) -> Result<Vec<StreamDescriptor>> {
    let format_context = format_context.lock().unwrap();
    self.start_time = format_context.get_start_time();

    let selected_streams = get_first_audio_stream_id(&format_context)?;

    let cloned_parameters = parameters;

    let (audio_source_sender, audio_source_receiver) = channel(100);

    self.audio_source_sender = Some(Arc::new(Mutex::new(audio_source_sender)));

    let cloned_sender = response_sender.clone();
    let start_time = self.start_time;

    self.sender = Some(response_sender);

    self.ws_thread = Some(thread::spawn(move || {
      let sequence_number = Arc::new(AtomicUsize::new(0));

      let future = async {
        let (_authot, ws_stream) = authot::Authot::new(&cloned_parameters).await;
        let (ws_sender, ws_receiver) = ws_stream.split();

        let send_to_ws = audio_source_receiver.map(Ok).forward(ws_sender);

        let receive_from_ws = {
          ws_receiver.for_each(|event| async {
            let event = event.unwrap();
            trace!("{}", event);
            let event: Result<WebsocketResponse> = WebsocketResponse::try_from(event);

            if let Ok(event) = event {
              if event.message == "AudioAdded" {}
              if event.message == "EndOfTranscript" {
                info!("End of transcript from Authot");
                let result = ProcessResult::end_of_process();
                cloned_sender.lock().unwrap().send(result).unwrap();
              }
              if event.message == "AddTranscript" {
                if let Some(metadata) = event.metadata {
                  let sequence_index = sequence_number.load(Acquire);
                  let result =
                    ProcessResult::new_xml(metadata.generate_ttml(start_time, sequence_index));
                  cloned_sender.lock().unwrap().send(result).unwrap();

                  sequence_number.store(sequence_index + 1, Release);
                }
              }
            } else {
              debug!("receive raw message: {:?}", event);
            }
          })
        };

        pin_mut!(send_to_ws, receive_from_ws);
        future::select(send_to_ws, receive_from_ws).await;
        info!("Ending Authot Live server.");
      };

      let mut runtime = Runtime::new().unwrap();

      runtime.block_on(future);
    }));

    Ok(selected_streams)
  }

  fn process_frame(
    &mut self,
    job_result: JobResult,
    _stream_index: usize,
    process_frame: ProcessFrame,
  ) -> Result<ProcessResult> {
      match process_frame {
        ProcessFrame::AudioVideo(frame) => {
            unsafe {
              trace!(
                "Frame {} samples, {} channels, {} bytes",
                (*frame.frame).nb_samples,
                (*frame.frame).channels,
                (*frame.frame).linesize[0],
              );

              let size = ((*frame.frame).channels * (*frame.frame).nb_samples * 2) as usize;
              let data = Vec::from_raw_parts((*frame.frame).data[0], size, size);
              let message = Message::binary(data.clone());
              std::mem::forget(data);

              if let Some(audio_source_sender) = &mut self.audio_source_sender {
	        audio_source_sender.lock().unwrap().try_send(message.clone()).unwrap()
              }
            }
        }
        _ => {
          return Err(MessageError::RuntimeError(format!(
            "Could not open frame as it was no AudioVideo frame in job {:?}",
            job_result.get_str_job_id()
          )))
        }
    };

    Ok(ProcessResult::empty())
  }

  fn ending_process(&mut self) -> Result<()> {
    if let Some(audio_source_sender) = &mut self.audio_source_sender {
      let data = json!({
        "message": "EndOfStream",
        "last_seq_no": 0
      });

      let message = Message::Text(data.to_string());

      audio_source_sender.lock().unwrap().try_send(message.clone()).unwrap()
    }

    self.ws_thread.take().map(JoinHandle::join);
    Ok(())
  }
}

fn get_first_audio_stream_id(
  format_context: &FormatContext,
) -> Result<Vec<StreamDescriptor>> {
  // select first audio stream index
  for stream_index in 0..format_context.get_nb_streams() {
    if format_context.get_stream_type(stream_index as isize) == AVMediaType::AVMEDIA_TYPE_AUDIO {
      let channel_layouts = vec!["mono".to_string()];
      let sample_formats = vec!["s16".to_string()];
      let sample_rates = vec![16000];
      let filters = vec![AudioFilter::Format(AudioFormat {
            sample_rates,
            channel_layouts,
            sample_formats,
          })];

      let stream_descriptor = StreamDescriptor::new_audio(
        stream_index as usize,
        filters
      );

      return Ok(vec![stream_descriptor]);
    }
  }

  Err(MessageError::RuntimeError(
    "No such audio stream in the source".to_string(),
  ))
}

fn main() {
  let worker = TranscriptEvent::default();
  start_worker(worker);
}
