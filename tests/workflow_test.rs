use anyhow::Result;
use async_trait::async_trait;
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;

use codingbot::agent::build_autonomous_coding_graph;
use codingbot::git;
use codingbot::graph::engine::{StateGraph, END_NODE};
use codingbot::graph::node::Node;
use codingbot::graph::state::AgentState;
use codingbot::llm::HeuristicEngine;

struct IncrementNode {
    name: &'static str,
}

#[async_trait]
impl Node for IncrementNode {
    fn name(&self) -> &'static str {
        self.name
    }

    async fn execute(&self, state: &mut AgentState) -> Result<()> {
        let count: usize = state
            .metadata
            .get("counter")
            .and_then(|c| c.parse().ok())
            .unwrap_or(0);
        state
            .metadata
            .insert("counter".to_string(), (count + 1).to_string());
        state.record_step(self.name, format!("Incremented counter to {}", count + 1));
        Ok(())
    }
}

#[tokio::test]
async fn test_state_graph_basic_flow() -> Result<()> {
    let mut graph = StateGraph::new();

    graph.add_node("step1", Box::new(IncrementNode { name: "step1" }));
    graph.add_node("step2", Box::new(IncrementNode { name: "step2" }));

    graph.set_entry_point("step1");
    graph.add_edge("step1", "step2");
    graph.add_edge("step2", END_NODE);

    let state = AgentState::new(PathBuf::from("."), "test flow", 3);
    let final_state = graph.run(state).await?;

    let counter = final_state.metadata.get("counter").unwrap();
    assert_eq!(counter, "2");
    assert_eq!(final_state.step_history.len(), 2);
    Ok(())
}

#[tokio::test]
async fn test_state_graph_conditional_loop() -> Result<()> {
    let mut graph = StateGraph::new();

    graph.add_node("inc", Box::new(IncrementNode { name: "inc" }));
    graph.set_entry_point("inc");

    // Loop until counter reaches 3
    graph.add_conditional_edge("inc", |state: &AgentState| {
        let count: usize = state
            .metadata
            .get("counter")
            .and_then(|c| c.parse().ok())
            .unwrap_or(0);
        if count < 3 {
            "inc".to_string()
        } else {
            END_NODE.to_string()
        }
    });

    let state = AgentState::new(PathBuf::from("."), "test loop", 5);
    let final_state = graph.run(state).await?;

    let counter = final_state.metadata.get("counter").unwrap();
    assert_eq!(counter, "3");
    assert_eq!(final_state.step_history.len(), 3);
    Ok(())
}

#[tokio::test]
async fn test_autonomous_coding_cycle_end_to_end() -> Result<()> {
    // 1. Create a temporary sandboxed Git repository
    let temp_dir = tempfile::tempdir()?;
    let repo_path = temp_dir.path().to_path_buf();

    git::git_init(&repo_path)?;

    let _ = std::process::Command::new("git")
        .args(["config", "user.name", "Test Agent"])
        .current_dir(&repo_path)
        .output();
    let _ = std::process::Command::new("git")
        .args(["config", "user.email", "agent@test.local"])
        .current_dir(&repo_path)
        .output();

    let cargo_toml = r#"[package]
name = "demo_crate"
version = "0.1.0"
edition = "2021"

[dependencies]
"#;
    fs::write(repo_path.join("Cargo.toml"), cargo_toml)?;
    fs::create_dir_all(repo_path.join("src"))?;

    let initial_lib = r#"pub fn add(a: i32, b: i32) -> i32 {
    a + b
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_add() {
        assert_eq!(add(2, 2), 4);
    }
}
"#;
    fs::write(repo_path.join("src").join("lib.rs"), initial_lib)?;
    git::git_commit_all(&repo_path, "initial commit")?;

    // 2. Set up autonomous coding agent with deliberate self-correction trigger
    let task = "Implement fibonacci function with tests";
    let mut state = AgentState::new(repo_path.clone(), task, 3);
    state
        .metadata
        .insert("is_demo".to_string(), "true".to_string());
    state
        .metadata
        .insert("create_branch".to_string(), "true".to_string());

    let llm = Arc::new(HeuristicEngine::new());
    let graph = build_autonomous_coding_graph(llm);

    // 3. Execute graph
    let final_state = graph.run(state).await?;

    // 4. Verify results
    assert!(final_state.is_successful, "Agent workflow should succeed");
    assert!(
        final_state.iteration >= 1,
        "Should have triggered self-correction loop"
    );
    assert!(final_state.diff.is_some(), "Git diff should be generated");
    assert!(
        final_state.pr_summary.is_some(),
        "Pull Request summary should be produced"
    );

    let pr = final_state.pr_summary.unwrap();
    assert!(pr.tests_passed);
    assert!(pr.summary_markdown.contains("Autonomous Agent PR"));
    assert!(pr.summary_markdown.contains("Self-Correction Cycle Report"));

    Ok(())
}
