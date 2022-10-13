use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};

#[derive(Clone, Serialize, Deserialize)]
pub struct Task {
    pub name: String,
    pub shell: Option<String>,
    pub cmd: String,
    pub result: Option<TaskResult>,
    pub output: Arc<Mutex<Vec<String>>>,
}

#[derive(Clone, Serialize, Deserialize)]
pub enum TaskResult {
    Success,
    Failed { code: Option<i32> },
    RunnerFailed { message: String },
}
