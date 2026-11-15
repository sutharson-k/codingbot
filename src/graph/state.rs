use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

/// Complete state container passed between nodes in the autonomous coding agent graph.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentState {
    /// Absolute or relative path to the target repository
    pub repo_path: PathBuf,

    /// User's instruction / task goal
    pub task: String,

    /// Repository indexing data
    pub index: Option<RepoIndex>,

    /// Planned changes produced by the planner node
    pub plan: Option<TaskPlan>,

    /// Generated file modifications / patches
    pub patches: Vec<CodePatch>,

    /// Results of the latest test suite run
    pub test_result: Option<TestRunResult>,

    /// Current self-correction iteration count (starts at 0)
    pub iteration: usize,

    /// Maximum allowed self-correction retries
    pub max_retries: usize,

    /// Final git diff generated
    pub diff: Option<String>,

    /// Generated Pull Request summary
    pub pr_summary: Option<PullRequestSummary>,

    /// Audit trail of state transitions and step descriptions
    pub step_history: Vec<StateStepRecord>,

    /// Indicates whether the overall workflow finished successfully
    pub is_successful: bool,

    /// Failure or termination reason if any
    pub error_message: Option<String>,

    /// Generic key-value store for node scratchpad data
    pub metadata: HashMap<String, String>,
}

impl AgentState {
    pub fn new(repo_path: PathBuf, task: impl Into<String>, max_retries: usize) -> Self {
        Self {
            repo_path,
            task: task.into(),
            index: None,
            plan: None,
            patches: Vec::new(),
            test_result: None,
            iteration: 0,
            max_retries,
            diff: None,
            pr_summary: None,
            step_history: Vec::new(),
            is_successful: false,
            error_message: None,
            metadata: HashMap::new(),
        }
    }

    pub fn record_step(&mut self, node: &str, description: impl Into<String>) {
        self.step_history.push(StateStepRecord {
            step_number: self.step_history.len() + 1,
            node_name: node.to_string(),
            description: description.into(),
            timestamp: chrono::Utc::now().to_rfc3339(),
        });
    }
}

/// Audit log record for each node step
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateStepRecord {
    pub step_number: usize,
    pub node_name: String,
    pub description: String,
    pub timestamp: String,
}

/// Metadata extracted during the repository indexing phase
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepoIndex {
    pub root_path: String,
    pub git_branch: Option<String>,
    pub git_head_commit: Option<String>,
    pub git_dirty: bool,
    pub total_files: usize,
    pub files: Vec<IndexedFile>,
    pub project_type: ProjectType,
    pub detected_test_command: Option<String>,
    pub summary: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ProjectType {
    Rust,
    NodeJs,
    Python,
    Go,
    Generic,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexedFile {
    pub relative_path: String,
    pub extension: Option<String>,
    pub size_bytes: u64,
    pub is_test: bool,
    pub is_entrypoint: bool,
    /// Truncated snippet or outline (first few lines or symbol signatures)
    pub preview: Option<String>,
}

/// High-level change plan
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskPlan {
    pub objective: String,
    pub files_to_touch: Vec<String>,
    pub files_to_create: Vec<String>,
    pub step_by_step_strategy: Vec<String>,
    pub test_strategy: String,
    pub rationale: String,
}

/// A specific file patch or creation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodePatch {
    pub relative_path: String,
    pub action: PatchAction,
    pub content: String,
    pub rationale: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum PatchAction {
    Create,
    Modify,
    Delete,
}

/// Test run execution outcome
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestRunResult {
    pub command: String,
    pub success: bool,
    pub exit_code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
    pub duration_millis: u128,
    pub parsed_errors: Vec<ParsedError>,
}

/// Parsed compiler error or test failure diagnostic
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParsedError {
    pub file: Option<String>,
    pub line: Option<usize>,
    pub message: String,
    pub error_type: ErrorType,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ErrorType {
    Compilation,
    Assertion,
    Runtime,
    Timeout,
    Unknown,
}

/// Pull Request summary artifact
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PullRequestSummary {
    pub title: String,
    pub branch_name: String,
    pub summary_markdown: String,
    pub files_changed: Vec<String>,
    pub iterations_needed: usize,
    pub tests_passed: bool,
}
