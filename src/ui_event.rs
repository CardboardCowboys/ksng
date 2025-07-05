use uuid::Uuid;

#[derive(Copy, Clone, PartialEq, Eq, Debug, Hash)]
pub enum KsngEvent {
  ProjectNew,
  ProjectClose,
  ProjectSave,
  ProjectOpen,
  ProjectOpenId(Uuid),
  ProjectDelete(Uuid),
  Quit,
}
