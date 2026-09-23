use egui::{ComboBox, Ui};
use klib::objects::{
  attachment::{AttachmentReader, AttachmentResolver},
  audio::AudioFileSource,
  event::{EventType, EventValue},
  track::EventList,
};
use ksng_ml_ipc::packet::{AudioCodec, AudioOptions, HtdemucsTask, host_start_task::Task};
use uuid::Uuid;

use crate::{
  KsngContext,
  fs::KsngAttachmentResolver,
  ml::{
    ui::{model_select_dropdown, task::TaskModal},
    worker::WorkerManager,
  },
};

pub struct HtdemucsTab {
  selected_model: Option<(Uuid, String)>,
  selected_codec: AudioCodec,
  bitrate: u32,
}

impl Default for HtdemucsTab {
  fn default() -> Self {
    Self {
      selected_model: Default::default(),
      selected_codec: AudioCodec::Flac,
      bitrate: Default::default(),
    }
  }
}

impl HtdemucsTab {
  pub fn htdemucs_tab(&mut self, app: &KsngContext, ui: &mut Ui, worker: &WorkerManager) {
    let project_ref = app.project.borrow();
    let Some(project) = &*project_ref else {
      ui.label("No project loaded.");
      return;
    };
    let Some(source_event) = app
      .selection
      .selected_events()
      .iter()
      .flat_map(|id| {
        project
          .file
          .tracks
          .iter()
          .flat_map(|t| t.events.find_id(*id))
      })
      .find(|ev| ev.event_type == EventType::AudioClip)
    else {
      ui.label("Select an audio event to continue.");
      return;
    };

    if app.selection.selected_events().len() > 1 {
      ui.label("Only one event may be selected.");
      return;
    }

    self.selected_model = model_select_dropdown(
      ui,
      "htdemucs#model_select",
      "Model",
      self.selected_model.as_ref().map(|p| (p.0, p.1.as_str())),
      worker,
      "Htdemucs",
    );

    let mut codec = self.selected_codec;

    ComboBox::new("htdemucs#codec_select", "Codec")
      .selected_text(format!("{:?}", self.selected_codec))
      .show_ui(ui, |ui| {
        ui.selectable_value(&mut codec, AudioCodec::Aac, "AAC");
        ui.selectable_value(&mut codec, AudioCodec::Flac, "FLAC");
        ui.selectable_value(&mut codec, AudioCodec::Mp3, "MP3");
        ui.selectable_value(&mut codec, AudioCodec::Wav, "WAV");
      });

    self.selected_codec = codec;

    if self.bitrate == 0 && codec == AudioCodec::Aac {
      self.bitrate = 256000;
    } else if self.bitrate == 0 && codec == AudioCodec::Mp3 {
      self.bitrate = 320000;
    }

    if codec == AudioCodec::Aac || codec == AudioCodec::Mp3 {
      ui.label("Bit Rate");
      let mut bitrate = self.bitrate.to_string();
      ui.text_edit_singleline(&mut bitrate);
      if let Ok(bitrate) = bitrate.parse::<u32>() {
        self.bitrate = bitrate;
      }
    }

    if let Some((id, name)) = self.selected_model.as_ref() {
      ui.label(format!(
        "The selected audio clip will be separated into stems by the model {name}"
      ));

      let resolver = KsngAttachmentResolver {};
      let input_path = match &source_event.value {
        Some(EventValue::AudioClip { file, .. }) => match &file.source {
          AudioFileSource::Path(path_buf) => path_buf.clone(),
          AudioFileSource::Attachment(uuid) => {
            let Some(attachment) = project.file.attachments.iter().find(|a| a.id == *uuid) else {
              log::error!("Could not find attachment for ID {uuid:?}");
              return;
            };

            match resolver.read(attachment) {
              AttachmentReader::Path(path_buf) => path_buf,
              AttachmentReader::Stream(_read) => todo!(),
            }
          }
        },
        _ => return,
      };
      let Some(input_path) = input_path.to_str().map(|s| s.to_string()) else {
        return;
      };

      if ui.button("Start").clicked() {
        let out_id = Uuid::new_v4();
        let temp = std::env::temp_dir();
        let Some(vocals_path) = temp
          .join(format!("{}-vocals", out_id))
          .to_str()
          .map(|s| s.to_string())
        else {
          return;
        };
        let Some(drums_path) = temp
          .join(format!("{}-drums", out_id))
          .to_str()
          .map(|s| s.to_string())
        else {
          return;
        };
        let Some(bass_path) = temp
          .join(format!("{}-bass", out_id))
          .to_str()
          .map(|s| s.to_string())
        else {
          return;
        };
        let Some(other_path) = temp
          .join(format!("{}-other", out_id))
          .to_str()
          .map(|s| s.to_string())
        else {
          return;
        };

        let task_id = worker.tasks.write().unwrap().start_task(
          "Stem Separation".to_owned(),
          *id,
          crate::ml::tasks::TaskExtraData::Htdemucs {
            source_event_id: source_event.id,
          },
          Task::Htdemucs(HtdemucsTask {
            input_path,
            output_path_vocals: vocals_path,
            output_path_drums: drums_path,
            output_path_bass: bass_path,
            output_path_other: other_path,
            codec: self.selected_codec.into(),
            audio_options: Some(AudioOptions {
              options_str: String::default(),
              bit_rate: self.bitrate,
            }),
          }),
        );

        app.modals.add(TaskModal::new(task_id));
      }
    }
  }
}
