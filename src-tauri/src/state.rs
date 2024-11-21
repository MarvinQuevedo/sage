use std::sync::Arc;
use tokio::sync::Mutex;

/// A simple wrapper around Arc<Mutex<T>> to provide a similar interface to Tauri's State
pub struct State<T>(Arc<Mutex<T>>);

impl<T> State<T> {
    /// Create a new State instance
    pub fn new(inner: T) -> Self {
        Self(Arc::new(Mutex::new(inner)))
    }

    /// Get access to the inner value through a lock
    pub async fn lock(&self) -> tokio::sync::MutexGuard<'_, T> {
        self.0.lock().await
    }
}

impl<T> Clone for State<T> {
    fn clone(&self) -> Self {
        Self(Arc::clone(&self.0))
    }
}

// Optional: Implement From/Into for Arc<Mutex<T>> if needed
impl<T> From<Arc<Mutex<T>>> for State<T> {
    fn from(inner: Arc<Mutex<T>>) -> Self {
        Self(inner)
    }
}

impl<T> From<State<T>> for Arc<Mutex<T>> {
    fn from(state: State<T>) -> Self {
        state.0
    }
}