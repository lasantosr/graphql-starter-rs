use std::{cell::RefCell, collections::VecDeque, io, sync::Arc};

use parking_lot::{ReentrantMutex, ReentrantMutexGuard};
use tokio::sync::broadcast::{self, Sender};
use tokio_stream::wrappers::BroadcastStream;
use tracing_subscriber::fmt::MakeWriter;

#[derive(Clone)]
/// A [MakeWriter] that intercepts logs before writing them and allows both to accumulate them in a list or stream to
/// subscribers.
///
/// This interceptor can be cloned cheaply, as it contains an [Arc] inside, and will point to the same logs.
pub struct MakeWriterInterceptor {
    inner: Arc<Inner>,
}

/// Inner struct for [MakeWriterInterceptor]
struct Inner {
    stdout: io::Stdout,
    accumulate: usize,
    events: ReentrantMutex<RefCell<VecDeque<String>>>,
    stream_tx: Option<Sender<String>>,
}

impl MakeWriterInterceptor {
    /// Builds a new [MakeWriterInterceptor] that will write to [Stdout](io::Stdout)
    pub fn new(accumulate: usize, stream_buffer: usize) -> Self {
        Self {
            inner: Arc::new(Inner {
                stdout: io::stdout(),
                accumulate,
                events: ReentrantMutex::new(RefCell::new(VecDeque::with_capacity(accumulate))),
                stream_tx: if stream_buffer > 0 {
                    Some(broadcast::channel(stream_buffer).0)
                } else {
                    None
                },
            }),
        }
    }

    /// Retrieves the last events accumulated by this interceptor
    pub fn get_last_events(&self) -> VecDeque<String> {
        let events_guard = self.inner.events.lock();
        let events = events_guard.borrow();
        events.clone()
    }

    /// Subscribes to events until the returned stream is closed
    ///
    /// This method will return [None] only if the writer has been initialized with `stream_buffer = 0`
    pub fn subscribe_to_events(&self) -> Option<BroadcastStream<String>> {
        if let Some(tx) = &self.inner.stream_tx {
            let rx = tx.subscribe();
            Some(BroadcastStream::new(rx))
        } else {
            None
        }
    }
}

impl<'a> MakeWriter<'a> for MakeWriterInterceptor {
    type Writer = WriterInterceptor<'a, io::StdoutLock<'a>>;

    fn make_writer(&'a self) -> Self::Writer {
        WriterInterceptor {
            inner: self.inner.stdout.lock(),
            accumulate: self.inner.accumulate,
            events: self.inner.events.lock(),
            stream_tx: self.inner.stream_tx.clone(),
        }
    }
}

/// A Writer interceptor for [MakeWriterInterceptor]
pub struct WriterInterceptor<'a, W: io::Write> {
    inner: W,
    accumulate: usize,
    events: ReentrantMutexGuard<'a, RefCell<VecDeque<String>>>,
    stream_tx: Option<Sender<String>>,
}

impl<'a, W: io::Write> WriterInterceptor<'a, W> {
    fn intercept_line(&mut self, line: &str) {
        // Push the event to the vec if accumulate is enabled
        if self.accumulate > 0 {
            let mut lines = self.events.borrow_mut();
            if lines.len() >= self.accumulate {
                lines.pop_front();
            }
            lines.push_back(line.to_owned());
        }
        // If stream capabilities are enabled, send the log
        if let Some(tx) = &mut self.stream_tx {
            if tx.receiver_count() > 0 && tx.send(line.to_owned()).is_err() {
                eprintln!("Couldn't send a log event to the stream")
            }
        }
    }
}

impl<'a, W: io::Write> io::Write for WriterInterceptor<'a, W> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        if let Ok(line) = std::str::from_utf8(buf) {
            self.intercept_line(line);
        };
        self.inner.write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use tokio_stream::{wrappers::errors::BroadcastStreamRecvError, StreamExt};
    use tracing_subscriber::prelude::*;

    use super::*;

    #[tokio::test]
    async fn test_writer() -> anyhow::Result<()> {
        // Create the interceptor
        let make_writer = MakeWriterInterceptor::new(2, 2);

        // Install a subscriber using the interceptor writer
        tracing_subscriber::registry()
            .with(
                tracing_subscriber::fmt::layer()
                    .compact()
                    .with_writer(make_writer.clone()),
            )
            .init();

        // Log three events
        tracing::info!("event-#1");
        tracing::info!("event-#2");
        tracing::info!("event-#3");

        // Retrieve the last events, which should accumulate the last two only
        let events = make_writer.get_last_events();
        assert_eq!(2, events.len());
        assert!(events.front().unwrap().contains("event-#2"));
        assert!(events.back().unwrap().contains("event-#3"));

        // Subscribe to live events
        let mut events_tail = make_writer.subscribe_to_events().unwrap();

        // Log three more events
        tracing::info!("event-#4");
        tracing::info!("event-#5");
        tracing::info!("event-#6");
        tracing::info!("event-#7");

        // As we didn't listen to any event until now, the first ones will be lagged
        assert!(matches!(
            events_tail.next().await.unwrap().err().unwrap(),
            BroadcastStreamRecvError::Lagged(2)
        ));
        assert!(events_tail.next().await.unwrap().unwrap().contains("event-#6"));
        assert!(events_tail.next().await.unwrap().unwrap().contains("event-#7"));

        // If we listen to them at the same time, we can read more that the buffer of two
        tokio::spawn(async {
            tracing::info!("event-#8");
            tokio::time::sleep(Duration::from_millis(1)).await;
            tracing::info!("event-#9");
            tokio::time::sleep(Duration::from_millis(1)).await;
            tracing::info!("event-#10");
        });

        assert!(events_tail.next().await.unwrap().unwrap().contains("event-#8"));
        assert!(events_tail.next().await.unwrap().unwrap().contains("event-#9"));
        assert!(events_tail.next().await.unwrap().unwrap().contains("event-#10"));

        Ok(())
    }
}
