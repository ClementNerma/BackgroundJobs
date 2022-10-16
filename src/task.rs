use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[derive(Clone, Serialize, Deserialize)]
pub struct Task {
    pub name: String,
    pub shell: Option<String>,
    pub cmd: String,
    pub start_dir: Option<PathBuf>,
}
