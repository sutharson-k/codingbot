use crate::graph::node::Node;
use crate::graph::state::{AgentState, TaskPlan};
use crate::llm::LlmClient;
use anyhow::{Context, Result};
use async_trait::async_trait;
use colored::Colorize;
use std::sync::Arc;

pub struct PlanNode {
    llm: Arc<dyn LlmClient>,
}

impl PlanNode {
    pub fn new(llm: Arc<dyn LlmClient>) -> Self {
        Self { llm }
    }
}

#[async_trait]
impl Node for PlanNode {
    fn name(&self) -> &'static str {
        "plan_changes"
    }

    async fn execute(&self, state: &mut AgentState) -> Result<()> {
        println!(
            "   {} Formulating architecture plan for task: '{}'...",
            "[*]".blue(),
            state.task
        );

        let index_summary = state
            .index
            .as_ref()
            .map(|i| {
                let file_list: Vec<String> = i
                    .files
                    .iter()
                    .take(30)
                    .map(|f| format!("- {} ({} bytes)", f.relative_path, f.size_bytes))
                    .collect();
                format!(
                    "Repository Type: {:?}\nTotal Files: {}\nKey Files:\n{}",
                    i.project_type,
                    i.total_files,
                    file_list.join("\n")
                )
            })
            .unwrap_or_else(|| "No repository index available.".to_string());

        let system_prompt = r#"You are an autonomous senior software architect and coding planner.
Given a user task and a repository index, formulate a concise, robust implementation and testing plan.
Output your response as pure JSON matching this schema:
{
  "objective": "High level goal",
  "files_to_touch": ["relative/path/to/file"],
  "files_to_create": ["relative/path/to/new_file"],
  "step_by_step_strategy": ["step 1", "step 2"],
  "test_strategy": "How tests will be executed and validated",
  "rationale": "Why this design was chosen"
}"#;

        let user_prompt = format!(
            "CREATE PLAN FOR TASK:\n{}\n\nREPOSITORY CONTEXT:\n{}",
            state.task, index_summary
        );

        let response = self
            .llm
            .complete(system_prompt, &user_prompt)
            .await
            .context("LLM planning request failed")?;

        let cleaned_json = extract_json_block(&response);
        let plan: TaskPlan = serde_json::from_str(&cleaned_json).unwrap_or_else(|_| {
            // Sensible fallback if model outputs free text
            TaskPlan {
                objective: state.task.clone(),
                files_to_touch: vec!["src/lib.rs".to_string()],
                files_to_create: vec![],
                step_by_step_strategy: vec![
                    "Inspect repository structure".to_string(),
                    "Implement changes corresponding to the user request".to_string(),
                    "Run unit tests to verify solution".to_string(),
                ],
                test_strategy: "Execute cargo test".to_string(),
                rationale: "Default heuristic plan".to_string(),
            }
        });

        println!(
            "   {} Plan formulated: {} (touching {} files)",
            "[+]".green(),
            plan.objective.bold(),
            plan.files_to_touch.len() + plan.files_to_create.len()
        );

        for step in &plan.step_by_step_strategy {
            println!("      {} {}", "•".cyan(), step);
        }

        state.plan = Some(plan);
        state.record_step(
            "plan_changes",
            format!("Plan created for task: {}", state.task),
        );
        Ok(())
    }
}

fn extract_json_block(text: &str) -> String {
    let text = text.trim();
    if let Some(start) = text.find("```json") {
        let remainder = &text[start + 7..];
        if let Some(end) = remainder.find("```") {
            return remainder[..end].trim().to_string();
        }
    } else if let Some(start) = text.find("```") {
        let remainder = &text[start + 3..];
        if let Some(end) = remainder.find("```") {
            return remainder[..end].trim().to_string();
        }
    } else if let (Some(start), Some(end)) = (text.find('{'), text.rfind('}')) {
        if start < end {
            return text[start..=end].to_string();
        }
    }
    text.to_string()
}
