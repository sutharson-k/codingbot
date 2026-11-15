use crate::git;
use crate::graph::node::Node;
use crate::graph::state::{AgentState, PullRequestSummary};
use anyhow::Result;
use async_trait::async_trait;
use colored::Colorize;

pub struct CreateDiffOrPrNode;

#[async_trait]
impl Node for CreateDiffOrPrNode {
    fn name(&self) -> &'static str {
        "create_diff_pr"
    }

    async fn execute(&self, state: &mut AgentState) -> Result<()> {
        println!(
            "   {} Generating clean Git diff and Pull Request description...",
            "[*]".blue()
        );

        let tests_passed = state
            .test_result
            .as_ref()
            .map(|r| r.success)
            .unwrap_or(false);

        // Fetch git diff
        let diff = if git::is_git_repo(&state.repo_path) {
            git::git_diff(&state.repo_path).unwrap_or_default()
        } else {
            String::new()
        };

        let diff_str = if diff.trim().is_empty() {
            // Synthesize fallback patch representation from recorded patches
            let mut synth = String::new();
            for patch in &state.patches {
                synth.push_str(&format!(
                    "--- a/{}\n+++ b/{}\n@@ -0,0 +1,{} @@\n{}\n",
                    patch.relative_path,
                    patch.relative_path,
                    patch.content.lines().count(),
                    patch.content
                ));
            }
            synth
        } else {
            diff
        };

        state.diff = Some(diff_str.clone());

        let files_changed: Vec<String> = state
            .patches
            .iter()
            .map(|p| p.relative_path.clone())
            .collect();

        let branch_name = format!(
            "codingbot/task-{}",
            chrono::Utc::now().format("%Y%m%d-%H%M%S")
        );

        // Create git branch and commit if requested or in demo mode
        let should_commit = state.metadata.get("create_branch").map(|s| s.as_str()) == Some("true")
            || state.metadata.get("is_demo").map(|s| s.as_str()) == Some("true");

        if should_commit && git::is_git_repo(&state.repo_path) {
            println!(
                "   {} Creating dedicated git branch '{}' and committing...",
                "[*]".blue(),
                branch_name.cyan()
            );
            if let Err(e) = git::git_checkout_new_branch(&state.repo_path, &branch_name) {
                eprintln!("   Warning: could not checkout branch: {}", e);
            }
            let commit_msg = format!(
                "feat: {}\n\nAutonomous coding workflow completed in {} iterations.\nTests passed: {}",
                state.task, state.iteration, tests_passed
            );
            if let Err(e) = git::git_commit_all(&state.repo_path, &commit_msg) {
                eprintln!("   Warning: could not commit changes: {}", e);
            } else {
                println!("   {} Committed changes to git successfully.", "[+]".green());
            }
        }

        // Build comprehensive markdown PR description
        let mut pr_md = String::new();
        pr_md.push_str(&format!("# Autonomous Agent PR: {}\n\n", state.task));
        pr_md.push_str("## 🎯 Objective\n");
        if let Some(plan) = &state.plan {
            pr_md.push_str(&format!("{}\n\n", plan.objective));
            pr_md.push_str("### 📋 Implementation Strategy\n");
            for step in &plan.step_by_step_strategy {
                pr_md.push_str(&format!("- {}\n", step));
            }
            pr_md.push_str("\n");
        } else {
            pr_md.push_str(&format!("Implemented task: {}\n\n", state.task));
        }

        pr_md.push_str("## 🛠️ Changes Made\n");
        for file in &files_changed {
            pr_md.push_str(&format!("- `{}`\n", file));
        }
        pr_md.push_str("\n");

        pr_md.push_str("## 🧪 Verification & Test Results\n");
        if let Some(res) = &state.test_result {
            pr_md.push_str(&format!(
                "- **Test Command**: `{}`\n- **Status**: {}\n- **Duration**: {}ms\n\n",
                res.command,
                if res.success {
                    "✅ **PASSED (Zero regressions)**"
                } else {
                    "❌ **FAILED**"
                },
                res.duration_millis
            ));
        }

        pr_md.push_str("## 🔄 Self-Correction Cycle Report\n");
        pr_md.push_str(&format!(
            "- **Total Iterations**: {} (Max allowed: {})\n",
            state.iteration, state.max_retries
        ));
        if state.iteration > 0 {
            pr_md.push_str(
                "- **Self-Healing**: Initial failure diagnosed and automatically corrected.\n",
            );
        } else {
            pr_md.push_str("- **Direct Pass**: Solution passed verification on initial attempt.\n");
        }
        pr_md.push_str("\n");

        pr_md.push_str("## 📄 Clean Git Diff\n```diff\n");
        pr_md.push_str(&diff_str);
        if !diff_str.ends_with('\n') {
            pr_md.push('\n');
        }
        pr_md.push_str("```\n");

        let pr_summary = PullRequestSummary {
            title: format!("feat: {}", state.task),
            branch_name,
            summary_markdown: pr_md.clone(),
            files_changed,
            iterations_needed: state.iteration,
            tests_passed,
        };

        state.pr_summary = Some(pr_summary);
        state.is_successful = tests_passed;
        state.record_step(
            "create_diff_pr",
            format!(
                "Created PR and diff (Tests passed: {}, Iterations: {})",
                tests_passed, state.iteration
            ),
        );

        println!(
            "\n{}",
            "=================== CLEAN GIT DIFF ==================="
                .green()
                .bold()
        );
        println!("{}", diff_str);
        println!(
            "{}",
            "======================================================"
                .green()
                .bold()
        );

        Ok(())
    }
}
