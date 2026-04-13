use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::mpsc;
use std::time::Duration;

/// Collects file-change events from a channel and batches them together,
/// deduplicating paths that appear multiple times within the debounce window.
pub struct Debouncer {
    rx: mpsc::Receiver<PathBuf>,
    window: Duration,
}

impl Debouncer {
    /// Create a new `Debouncer` that collects events from `rx` and uses
    /// `window_ms` milliseconds as the coalescing window.
    pub fn new(rx: mpsc::Receiver<PathBuf>, window_ms: u64) -> Self {
        Self {
            rx,
            window: Duration::from_millis(window_ms),
        }
    }

    /// Block until the first event arrives, then collect all additional events
    /// that arrive within the debounce window.  Returns `None` only when the
    /// sender has been dropped and no events remain.
    pub fn next_batch(&self) -> Option<Vec<PathBuf>> {
        // Wait for the first event (blocking).
        let first = self.rx.recv().ok()?;

        let mut seen: HashSet<PathBuf> = HashSet::new();
        seen.insert(first);

        // Drain any further events that arrive within the window.
        loop {
            match self.rx.recv_timeout(self.window) {
                Ok(path) => {
                    seen.insert(path);
                }
                Err(mpsc::RecvTimeoutError::Timeout) => break,
                Err(mpsc::RecvTimeoutError::Disconnected) => break,
            }
        }

        Some(seen.into_iter().collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;

    #[test]
    fn debounces_rapid_events() {
        let (tx, rx) = mpsc::channel();
        let debouncer = Debouncer::new(rx, 50);

        let path_a = PathBuf::from("/project/src/main.rs");
        let path_b = PathBuf::from("/project/src/lib.rs");

        // Send 3 events: path_a, path_b, path_a (path_a is a duplicate).
        tx.send(path_a.clone()).unwrap();
        tx.send(path_b.clone()).unwrap();
        tx.send(path_a.clone()).unwrap();

        // Drop the sender so recv_timeout eventually disconnects.
        drop(tx);

        let batch = debouncer.next_batch().expect("expected a batch");

        assert_eq!(
            batch.len(),
            2,
            "expected 2 unique paths, got {}: {:?}",
            batch.len(),
            batch
        );

        let mut sorted = batch.clone();
        sorted.sort();
        assert!(sorted.contains(&path_a));
        assert!(sorted.contains(&path_b));
    }
}
