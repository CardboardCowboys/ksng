use std::cell::RefCell;

use uuid::Uuid;

struct ActiveLock {
  entry_id: Uuid,
  object_id: Uuid,
  reason: String,
}

#[derive(Clone, Copy)]
pub struct LockHandle(Uuid);

/// Handles locked objects that can't be edited.
#[derive(Default)]
pub struct Locker {
  locked: RefCell<Vec<ActiveLock>>,
}

impl Locker {
  pub fn clear(&self) {
    self.locked.borrow_mut().clear();
  }

  /// Locks the object with the given ID, making it unable to be edited until
  /// unlocked. Returns a handle that should be used to unlock it.
  pub fn lock(&self, object_id: Uuid, why: impl Into<String>) -> LockHandle {
    let entry_id = Uuid::new_v4();
    self.locked.borrow_mut().push(ActiveLock {
      entry_id,
      object_id,
      reason: why.into(),
    });
    LockHandle(entry_id)
  }

  /// Unlocks the object locked with the given `LockHandle`.
  pub fn unlock(&self, handle: LockHandle) {
    self.locked.borrow_mut().retain(|l| l.entry_id != handle.0);
  }

  /// Checks if an object with the given ID is currently locked.
  pub fn is_locked(&self, id: Uuid) -> bool {
    self.locked.borrow().iter().any(|l| l.object_id == id)
  }

  /// Returns the reason that the given object is locked, if it is in fact
  /// locked.
  pub fn lock_reason(&self, id: Uuid) -> Option<String> {
    self
      .locked
      .borrow()
      .iter()
      .find(|l| l.object_id == id)
      .map(|l| l.reason.clone())
  }
}
