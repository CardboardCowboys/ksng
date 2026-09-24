use uuid::Uuid;

use crate::app::AppTabInitializer;

#[derive(Clone, PartialEq, Debug)]
pub enum KsngEvent {
  ProjectNew,
  ProjectClose,
  ProjectSave,
  ProjectOpen,
  ProjectOpenId(Uuid),
  ProjectDelete(Uuid),
  ProjectExportVideo,
  Quit,
  Undo,
  Redo,
  AudioDeviceChanged,
  CloseWindow(u64),
  OpenTabWindow(AppTabInitializer),
}
