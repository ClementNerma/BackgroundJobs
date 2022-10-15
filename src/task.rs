use std::{
    path::PathBuf,
    process::Child,
    sync::{Arc, Mutex, RwLock},
};

use serde::{Deserialize, Serialize};

#[derive(Clone, Serialize, Deserialize)]
pub struct Task {
    pub name: String,
    pub shell: Option<String>,
    pub cmd: String,
    pub start_dir: Option<PathBuf>,
    pub result: Option<TaskResult>,
    pub output: Arc<Mutex<Vec<String>>>,

    #[serde(skip_serializing, skip_deserializing)]
    pub child_handle: Arc<RwLock<Option<Child>>>,
}

#[derive(Clone, Serialize, Deserialize)]
pub enum TaskResult {
    Success,
    Failed { code: Option<i32> },
    RunnerFailed { message: String },
}
