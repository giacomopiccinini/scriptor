use serde::{Deserialize, Serialize};

/// Configuration for queue, responsible of transcribing fragmenta one after the other
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct QueueConfig {
    // Maximum number of elements in the queue (prevent RAM overloading)
    pub max_queue_elements: usize,
}

impl Default for QueueConfig {
    fn default() -> Self {
        Self {
            max_queue_elements: 10,
        }
    }
}
