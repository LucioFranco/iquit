use futures::{future, Async, Future, Poll};
use std::sync::{
    atomic::{AtomicBool, AtomicUsize, Ordering},
    Arc, Mutex,
};
use std::time::Duration;
use tokio_sync::task::AtomicTask;

mod work;

fn main() {
    let mut rt = tokio::runtime::Runtime::new().unwrap();

    let shutdown = Shutdown::new();

    for _ in 0..10 {
        rt.spawn(work::Thing::new(shutdown.register()));
    }

    println!("sleeping main thread");
    std::thread::sleep(Duration::from_secs(2));

    println!("shutting down");
    // You can select this to give it a timeout
    rt.block_on(shutdown.shutdown()).unwrap();

    println!("Were all done!");
}

pub struct Shutdown {
    inner: Arc<Inner>,
}

pub struct Handle {
    inner: Arc<Inner>,
    waiter: Arc<AtomicTask>,
}

pub struct HandleFuture {
    handle: Option<Handle>,
}

struct Inner {
    count: AtomicUsize,
    shutdown: Arc<AtomicTask>,
    handles: Mutex<Vec<Arc<AtomicTask>>>,
    started: AtomicBool,
}

impl Shutdown {
    pub fn new() -> Self {
        let count = AtomicUsize::new(0);
        let shutdown = Arc::new(AtomicTask::new());
        let handles = Mutex::new(Vec::new());
        let started = AtomicBool::new(false);

        let inner = Inner {
            count,
            shutdown,
            handles,
            started,
        };

        Shutdown {
            inner: Arc::new(inner),
        }
    }

    pub fn register(&self) -> Handle {
        let waiter = self.inner.increment();

        Handle {
            inner: self.inner.clone(),
            waiter,
        }
    }

    pub fn shutdown(self) -> impl Future<Item = (), Error = ()> {
        {
            let handles = self.inner.handles.lock().unwrap();
            for handle in &*handles {
                handle.notify();
            }

            self.inner.started.store(true, Ordering::Relaxed);
        }

        future::poll_fn(move || {
            self.inner.shutdown.register();

            let count = self.inner.count();

            println!("count {}", count);

            if count > 0 {
                Ok(Async::NotReady)
            } else {
                Ok(Async::Ready(()))
            }
        })
    }
}

impl Handle {
    pub fn register(&self) -> Handle {
        let waiter = self.inner.increment();

        Handle {
            inner: self.inner.clone(),
            waiter,
        }
    }

    pub fn into_future(self) -> HandleFuture {
        HandleFuture { handle: Some(self) }
    }
}

impl Drop for Handle {
    fn drop(&mut self) {
        println!("dropping handle");
        self.inner.decrement();
    }
}

impl Future for HandleFuture {
    type Item = Handle;
    type Error = ();

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        let handle = if let Some(handle) = self.handle.take() {
            handle.waiter.register();

            if handle.inner.started.load(Ordering::Acquire) {
                return Ok(Async::Ready(handle));
            } else {
                handle
            }
        } else {
            panic!("POLLED AFTER READY")
        };

        self.handle = Some(handle);

        Ok(Async::NotReady)
    }
}

impl Inner {
    pub fn increment(&self) -> Arc<AtomicTask> {
        let task = Arc::new(AtomicTask::new());
        self.count.fetch_add(1, Ordering::Relaxed);
        let mut handles = self.handles.lock().unwrap();
        handles.push(task.clone());

        task
    }

    pub fn decrement(&self) {
        self.count.fetch_sub(1, Ordering::Acquire);
        self.shutdown.notify();
    }

    pub fn count(&self) -> usize {
        self.count.load(Ordering::Acquire)
    }
}
