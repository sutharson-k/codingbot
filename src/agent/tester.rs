use crate::graph::node::Node;
use crate::graph::state::{AgentState, ErrorType, ParsedError, TestRunResult};
use anyhow::Result;
use async_trait::async_trait;
use colored::Colorize;
use std::process::Command;
use std::time::Instant;

pub struct RunTestsNode;

#[async_trait]
impl Node for RunTestsNode {
    fn name(&self) -> &'static str {
        "run_tests"
    }

    async fn execute(&self, state: &mut AgentState) -> Result<()> {
        let test_cmd = if let Some(cmd) = state.metadata.get("test_cmd") {
            cmd.clone()
        } else if let Some(cmd) = state.index.as_ref().and_then(|i| i.detected_test_command.clone()) {
            cmd
        } else {
            "cargo test".to_string()
        };

        qprintln!(state,
            "   {} Executing test suite: '{}' in '{}'...",
            "[*]".blue(),
            test_cmd.yellow(),
            state.repo_path.display()
        );

        let start = Instant::now();
        let (prog, args) = parse_command_str(&test_cmd);

        let output = Command::new(prog)
            .args(&args)
            .current_dir(&state.repo_path)
            .output();

        let duration = start.elapsed().as_millis();

        let (success, exit_code, stdout, stderr) = match output {
            Ok(out) => {
                let code = out.status.code();
                let is_ok = out.status.success();
                let out_str = String::from_utf8_lossy(&out.stdout).to_string();
                let err_str = String::from_utf8_lossy(&out.stderr).to_string();
                (is_ok, code, out_str, err_str)
            }
            Err(e) => (false, None, String::new(), format!("Failed to spawn test command: {}", e)),
        };

        let parsed_errors = parse_diagnostics(&stdout, &stderr);

        if success {
            qprintln!(state,
                "   {} Tests PASSED in {}ms! All test cases green.",
                "[PASS]".green().bold(),
                duration
            );
        } else {
            qprintln!(state,
                "   {} Tests FAILED (exit code: {:?}) in {}ms",
                "[FAIL]".red().bold(),
                exit_code,
                duration
            );
            if !state.quiet {
                for err in &parsed_errors {
                    println!(
                        "      {} [{:?}] {}",
                        "!".red(),
                        err.error_type,
                        err.message.trim()
                    );
                }
            }
        }

        let result = TestRunResult {
            command: test_cmd,
            success,
            exit_code,
            stdout,
            stderr,
            duration_millis: duration,
            parsed_errors,
        };

        state.test_result = Some(result);
        state.record_step(
            "run_tests",
            format!(
                "Ran tests: status = {}",
                if success { "PASSED" } else { "FAILED" }
            ),
        );

        Ok(())
    }
}

fn parse_command_str(cmd: &str) -> (String, Vec<String>) {
    let parts: Vec<&str> = cmd.split_whitespace().collect();
    if parts.is_empty() {
        ("cargo".to_string(), vec!["test".to_string()])
    } else {
        let prog = parts[0].to_string();
        let args = parts[1..].iter().map(|s| s.to_string()).collect();
        (prog, args)
    }
}

fn parse_diagnostics(stdout: &str, stderr: &str) -> Vec<ParsedError> {
    let mut errors = Vec::new();

    for line in stderr.lines() {
        if line.contains("error[E") || line.starts_with("error:") {
            errors.push(ParsedError {
                file: None,
                line: None,
                message: line.to_string(),
                error_type: ErrorType::Compilation,
            });
        }
    }

    for line in stdout.lines() {
        if line.contains("FAILED") || line.contains("panicked at") || line.contains("assertion `left == right` failed") {
            errors.push(ParsedError {
                file: None,
                line: None,
                message: line.to_string(),
                error_type: ErrorType::Assertion,
            });
        }
    }

    if errors.is_empty() && (!stdout.is_empty() || !stderr.is_empty()) {
        errors.push(ParsedError {
            file: None,
            line: None,
            message: "Non-zero exit code during test run".to_string(),
            error_type: ErrorType::Unknown,
        });
    }

    errors
}
