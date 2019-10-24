use super::{Handle, HandleFuture};
use futures::{try_ready, Future, Poll};
use std::time::{Duration, Instant};
use tokio::timer::Delay;

/// This future simulates waiting for the shutdown signal,
/// in practice this would be select against some other future.
pub struct Thing {
    pub handle: HandleFuture,
    pub delay: Option<Delay>,
    drop_handle: Option<Handle>,
}

impl Thing {
    pub fn new(handle: Handle) -> Self {
        Self {
            handle: handle.into_future(),
            delay: None,
            drop_handle: None,
        }
    }
}

impl Future for Thing {
    type Item = ();
    type Error = ();

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        if let Some(delay) = &mut self.delay {
            try_ready!(delay.poll().map_err(drop));
        } else {
            let handle = try_ready!(self.handle.poll());

            // This is to show that we can drop the returned handle
            // so if we select this we can get a value back we can use
            // this is the same handle aka it does not register a new task.
            self.drop_handle = Some(handle);

            println!("shutting down");

            let deadline = Instant::now() + Duration::from_secs(1);
            self.delay = Some(Delay::new(deadline));
        }

        // lets let the shutdown future that this task is done
        let drop_handle = self.drop_handle.take().unwrap();
        println!("Dropping handle");
        drop(drop_handle);

        Ok(().into())
    }
}
