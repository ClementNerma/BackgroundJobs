use std::{
    process::Child,
    sync::{Arc, Mutex},
};

use serde::{Deserialize, Serialize};

use crate::task::Task;

#[derive(Clone, Serialize, Deserialize)]
pub struct TaskWrapper {
    pub task: Task,
    pub state: Arc<Mutex<TaskState>>,
}

impl TaskWrapper {
    pub fn new(task: Task) -> Self {
        Self {
            task,
            state: Arc::new(Mutex::new(TaskState::new())),
        }
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct TaskState {
    pub status: TaskStatus,
    pub output: Vec<String>,
}

impl TaskState {
    pub fn new() -> Self {
        Self {
            status: TaskStatus::NotStartedYet,
            output: vec![],
        }
    }
}

#[derive(Serialize, Deserialize)]
pub enum TaskStatus {
    NotStartedYet,
    Running {
        #[serde(skip_serializing, skip_deserializing)]
        child: Option<Child>,
    },
    Success,
    Failed {
        code: Option<i32>,
    },
    RunnerFailed {
        message: String,
    },
}

impl Clone for TaskStatus {
    fn clone(&self) -> Self {
        match self {
            Self::NotStartedYet => Self::NotStartedYet,
            Self::Running { child: _ } => Self::Running { child: None },
            Self::Success => Self::Success,
            Self::Failed { code } => Self::Failed { code: code.clone() },
            Self::RunnerFailed { message } => Self::RunnerFailed {
                message: message.clone(),
            },
        }
    }
}

impl TaskStatus {
    pub fn is_running(&self) -> bool {
        matches!(self, TaskStatus::Running { child: _ })
    }

    pub fn is_completed(&self) -> bool {
        match self {
            TaskStatus::NotStartedYet | TaskStatus::Running { child: _ } => false,
            TaskStatus::Success
            | TaskStatus::Failed { code: _ }
            | TaskStatus::RunnerFailed { message: _ } => true,
        }
    }

    pub(super) fn get_child(&mut self) -> Option<&mut Child> {
        match self {
            TaskStatus::Running { child } => Some(child.as_mut().unwrap()),
            _ => None,
        }
    }
}
