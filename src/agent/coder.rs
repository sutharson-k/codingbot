use crate::graph::node::Node;
use crate::graph::state::{AgentState, CodePatch, PatchAction};
use crate::llm::LlmClient;
use anyhow::{Context, Result};
use async_trait::async_trait;
use colored::Colorize;
use std::fs;
use std::sync::Arc;

pub struct CodeGenerationNode {
    llm: Arc<dyn LlmClient>,
}

impl CodeGenerationNode {
    pub fn new(llm: Arc<dyn LlmClient>) -> Self {
        Self { llm }
    }
}

#[async_trait]
impl Node for CodeGenerationNode {
    fn name(&self) -> &'static str {
        "generate_code"
    }

    async fn execute(&self, state: &mut AgentState) -> Result<()> {
        println!(
            "   {} Synthesizing code patches (Iteration {}/{})...",
            "[*]".blue(),
            state.iteration,
            state.max_retries
        );

        let plan = state
            .plan
            .as_ref()
            .context("Cannot generate code: missing task plan")?;

        let target_files = if !plan.files_to_touch.is_empty() {
            &plan.files_to_touch
        } else if !plan.files_to_create.is_empty() {
            &plan.files_to_create
        } else {
            &vec!["src/lib.rs".to_string()]
        };

        let mut generated_patches = Vec::new();

        for rel_path in target_files {
            let full_path = state.repo_path.join(rel_path);
            let existing_content = fs::read_to_string(&full_path).unwrap_or_default();

            let system_prompt = r#"You are an expert autonomous software engineer.
Generate clean, bug-free, idiomatic code that satisfies the plan and user task.
Include thorough unit tests (e.g., #[cfg(test)] in Rust).
Return the complete file content enclosed within markdown code fences (e.g. ```rust ... ```)."#;

            let is_demo = state.metadata.get("is_demo").map(|s| s.as_str()) == Some("true")
                && state.iteration == 0;

            let prompt_tag = if is_demo {
                "DEMO_SELF_CORRECT_TRIGGER"
            } else {
                "STANDARD_GENERATION"
            };

            let user_prompt = format!(
                "TASK: {}\nOBJECTIVE: {}\nFILE TO UPDATE: {}\nTAG: {}\n\nEXISTING CONTENT:\n{}\n\nGENERATE CODE for {}",
                state.task, plan.objective, rel_path, prompt_tag, existing_content, rel_path
            );

            let llm_reply = self
                .llm
                .complete(system_prompt, &user_prompt)
                .await
                .context("LLM code generation call failed")?;

            let code = extract_code_block(&llm_reply);

            // Ensure parent directory exists
            if let Some(parent) = full_path.parent() {
                fs::create_dir_all(parent)
                    .with_context(|| format!("Failed to create parent dir: {:?}", parent))?;
            }

            // Write generated code to disk
            fs::write(&full_path, &code)
                .with_context(|| format!("Failed to write generated code to {:?}", full_path))?;

            println!(
                "   {} Wrote {} bytes to '{}'",
                "[+]".green(),
                code.len(),
                rel_path.cyan()
            );

            generated_patches.push(CodePatch {
                relative_path: rel_path.clone(),
                action: if existing_content.is_empty() {
                    PatchAction::Create
                } else {
                    PatchAction::Modify
                },
                content: code,
                rationale: format!("Code generated for plan: {}", plan.objective),
            });
        }

        state.patches = generated_patches;
        state.record_step(
            "generate_code",
            format!(
                "Generated code for {} files at iteration {}",
                target_files.len(),
                state.iteration
            ),
        );

        Ok(())
    }
}

fn extract_code_block(text: &str) -> String {
    let trimmed = text.trim();
    if let Some(start) = trimmed.find("```") {
        // Skip language tag line
        let remainder = &trimmed[start + 3..];
        let after_lang = if let Some(newline) = remainder.find('\n') {
            &remainder[newline + 1..]
        } else {
            remainder
        };

        if let Some(end) = after_lang.rfind("```") {
            return after_lang[..end].trim().to_string();
        }
    }
    trimmed.to_string()
}
