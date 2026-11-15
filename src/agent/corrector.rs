use crate::graph::node::Node;
use crate::graph::state::{AgentState, CodePatch, PatchAction};
use crate::llm::LlmClient;
use anyhow::{Context, Result};
use async_trait::async_trait;
use colored::Colorize;
use std::fs;
use std::sync::Arc;

pub struct SelfCorrectionNode {
    llm: Arc<dyn LlmClient>,
}

impl SelfCorrectionNode {
    pub fn new(llm: Arc<dyn LlmClient>) -> Self {
        Self { llm }
    }
}

/// Conditional router evaluating whether to self-correct or finish
pub fn evaluation_router(state: &AgentState) -> String {
    let test_success = state
        .test_result
        .as_ref()
        .map(|r| r.success)
        .unwrap_or(false);

    if test_success {
        if !state.quiet {
            println!(
                "   {} Routing to '{}' (All tests green)",
                "[DECISION]".magenta().bold(),
                "create_diff_pr".green()
            );
        }
        "create_diff_pr".to_string()
    } else if state.iteration < state.max_retries {
        if !state.quiet {
            println!(
                "   {} Routing to '{}' (Tests failed, retry {}/{})",
                "[DECISION]".magenta().bold(),
                "self_correct".yellow(),
                state.iteration + 1,
                state.max_retries
            );
        }
        "self_correct".to_string()
    } else {
        if !state.quiet {
            println!(
                "   {} Routing to '{}' (Max retries {} reached without clean pass)",
                "[DECISION]".magenta().bold(),
                "create_diff_pr".red(),
                state.max_retries
            );
        }
        "create_diff_pr".to_string()
    }
}

#[async_trait]
impl Node for SelfCorrectionNode {
    fn name(&self) -> &'static str {
        "self_correct"
    }

    async fn execute(&self, state: &mut AgentState) -> Result<()> {
        state.iteration += 1;

        qprintln!(state,
            "   {} Initiating autonomous self-correction cycle (Attempt {}/{})...",
            "[REPAIR]".yellow().bold(),
            state.iteration,
            state.max_retries
        );

        let test_res = state
            .test_result
            .as_ref()
            .context("Missing test result for self-correction")?;

        let failure_context = format!(
            "Exit Code: {:?}\n\nSTDOUT:\n{}\n\nSTDERR:\n{}",
            test_res.exit_code,
            test_res.stdout.lines().take(40).collect::<Vec<_>>().join("\n"),
            test_res.stderr.lines().take(40).collect::<Vec<_>>().join("\n")
        );

        let plan = state
            .plan
            .as_ref()
            .context("Missing plan for self-correction")?;

        let target_files = if !plan.files_to_touch.is_empty() {
            &plan.files_to_touch
        } else {
            &vec!["src/lib.rs".to_string()]
        };

        let mut updated_patches = Vec::new();

        for rel_path in target_files {
            let full_path = state.repo_path.join(rel_path);
            let current_code = fs::read_to_string(&full_path).unwrap_or_default();

            let system_prompt = r#"You are an expert autonomous software engineer specializing in debugging and self-correction.
Your task is to analyze compiler diagnostics or test assertion failures and rewrite the code to fix all issues.
Preserve test compatibility and ensure all edge cases are correctly handled.
Return the complete corrected file content enclosed in markdown code fences (```rust ... ```)."#;

            let user_prompt = format!(
                "SELF-CORRECT THE CODE\nTASK: {}\nOBJECTIVE: {}\nFILE: {}\n\nFAILURE DIAGNOSTICS:\n{}\n\nCURRENT CODE:\n{}\n\nOUTPUT REPAIRED CODE FOR {}:",
                state.task, plan.objective, rel_path, failure_context, current_code, rel_path
            );

            let llm_reply = self
                .llm
                .complete(system_prompt, &user_prompt)
                .await
                .context("LLM self-correction call failed")?;

            let repaired_code = extract_code_block(&llm_reply);

            fs::write(&full_path, &repaired_code)
                .with_context(|| format!("Failed to write self-corrected code to {:?}", full_path))?;

            qprintln!(state,
                "   {} Repaired file '{}' ({} bytes)",
                "[FIXED]".green(),
                rel_path.cyan(),
                repaired_code.len()
            );

            updated_patches.push(CodePatch {
                relative_path: rel_path.clone(),
                action: PatchAction::Modify,
                content: repaired_code,
                rationale: format!(
                    "Self-corrected after test failure in iteration {}",
                    state.iteration
                ),
            });
        }

        state.patches = updated_patches;
        state.record_step(
            "self_correct",
            format!("Applied self-correction at iteration {}", state.iteration),
        );

        Ok(())
    }
}

fn extract_code_block(text: &str) -> String {
    let trimmed = text.trim();
    if let Some(start) = trimmed.find("```") {
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
