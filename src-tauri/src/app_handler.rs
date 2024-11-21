use std::sync::Arc;
use tokio::sync::Mutex;
use std::collections::HashMap;

#[derive(Clone)]
pub struct AppHandle {
    events: Arc<Mutex<HashMap<String, Vec<String>>>>
}

impl AppHandle {
    pub fn new() -> Self {
        Self {
            events: Arc::new(Mutex::new(HashMap::new()))
        }
    }

    /// Simulates Tauri's emit function for events
    pub async fn emit<S: Into<String>, T: serde::Serialize>(
        &self,
        event: S,
        payload: T,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let event = event.into();
        let payload = serde_json::to_string(&payload)?;
        
        let mut events = self.events.lock().await;
        events
            .entry(event)
            .or_insert_with(Vec::new)
            .push(payload);
            
        Ok(())
    }

    /// Helper method to get emitted events (for testing)
    pub async fn get_events(&self, event: &str) -> Option<Vec<String>> {
        self.events.lock().await.get(event).cloned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_app_handle() {
        let handle = AppHandle::new();
        
        // Test emitting an event
        handle.emit("test-event", "test payload").await.unwrap();
        
        // Verify the event was stored
        let events = handle.get_events("test-event").await.unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0], "\"test payload\"");
    }
}