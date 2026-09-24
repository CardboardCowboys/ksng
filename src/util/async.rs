use std::{
  pin::Pin,
  task::{Context, Poll, Waker},
};

use futures_util::{FutureExt, future::BoxFuture};

pub enum AsyncResult<T> {
  Pending,
  Complete(T),
}

pub struct AsyncValue<T> {
  future: BoxFuture<'static, T>,
  result: AsyncResult<T>,
  context: Context<'static>,
  complete: bool,
}

impl<T> AsyncValue<T> {
  pub fn new<F>(f: F) -> AsyncValue<T>
  where
    F: Future<Output = T> + Send + 'static,
  {
    let future = Box::pin(f);

    AsyncValue {
      future,
      result: AsyncResult::Pending,
      context: Context::from_waker(Waker::noop()),
      complete: false,
    }
  }

  pub fn poll(&mut self) -> &AsyncResult<T> {
    if self.complete {
      return &self.result;
    }

    match self.future.as_mut().poll(&mut self.context) {
      Poll::Ready(v) => {
        self.result = AsyncResult::Complete(v);
        self.complete = true;
      }
      Poll::Pending => {}
    };

    &self.result
  }
}
