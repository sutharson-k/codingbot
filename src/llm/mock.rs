use super::client::LlmClient;
use anyhow::Result;
use async_trait::async_trait;

/// Autonomous heuristic engine that simulates high-accuracy LLM reasoning for testing,
/// offline environments, and deterministic autonomous loops.
pub struct HeuristicEngine;

impl Default for HeuristicEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl HeuristicEngine {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl LlmClient for HeuristicEngine {
    async fn complete(&self, system_prompt: &str, user_prompt: &str) -> Result<String> {
        let is_planning = system_prompt.contains("planner") || user_prompt.contains("CREATE PLAN");
        let is_coding = system_prompt.contains("coder") || user_prompt.contains("GENERATE CODE");
        let is_correcting = system_prompt.contains("corrector") || user_prompt.contains("SELF-CORRECT");

        if is_planning {
            Ok(r#"{
  "objective": "Implement requested functionality and verify test suite",
  "files_to_touch": ["src/lib.rs"],
  "files_to_create": [],
  "step_by_step_strategy": [
    "Analyze existing codebase structure and function signatures",
    "Implement target logic with correct boundary conditions",
    "Add or update unit tests to assert correct behavior"
  ],
  "test_strategy": "Run cargo test to ensure zero regressions and verify test coverage",
  "rationale": "Directly update the library module with clean idiomatic Rust and dedicated unit tests."
}"#.to_string())
        } else if is_correcting {
            // Self correction: analyze the test error or compiler error
            Ok(r#"```rust
// Self-corrected implementation resolving compiler/test errors
pub fn fibonacci(n: u64) -> u64 {
    match n {
        0 => 0,
        1 => 1,
        _ => {
            let mut a = 0;
            let mut b = 1;
            for _ in 2..=n {
                let temp = a + b;
                a = b;
                b = temp;
            }
            b
        }
    }
}

pub fn add(a: i32, b: i32) -> i32 {
    a + b
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_add() {
        assert_eq!(add(2, 2), 4);
    }

    #[test]
    fn test_fibonacci() {
        assert_eq!(fibonacci(0), 0);
        assert_eq!(fibonacci(1), 1);
        assert_eq!(fibonacci(2), 1);
        assert_eq!(fibonacci(5), 5);
        assert_eq!(fibonacci(10), 55);
    }
}
```"#.to_string())
        } else if is_coding {
            // First code generation attempt
            // In demo mode with deliberate self-correction, or standard generation:
            if user_prompt.contains("DEMO_SELF_CORRECT_TRIGGER") || user_prompt.contains("demo") {
                // Introduce a subtle off-by-one or type bug that the self-corrector will catch and fix!
                Ok(r#"```rust
// Initial implementation (with a deliberate edge-case bug for demonstration of self-correction)
pub fn fibonacci(n: u64) -> u64 {
    // Deliberate bug: returns 0 for n == 1 instead of 1
    match n {
        0 => 0,
        1 => 0, // Bug: should be 1
        _ => fibonacci(n - 1) + fibonacci(n - 2),
    }
}

pub fn add(a: i32, b: i32) -> i32 {
    a + b
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_add() {
        assert_eq!(add(2, 2), 4);
    }

    #[test]
    fn test_fibonacci() {
        assert_eq!(fibonacci(0), 0);
        assert_eq!(fibonacci(1), 1);
        assert_eq!(fibonacci(2), 1);
        assert_eq!(fibonacci(5), 5);
    }
}
```"#.to_string())
            } else {
                // Clean implementation
                Ok(r#"```rust
pub fn fibonacci(n: u64) -> u64 {
    match n {
        0 => 0,
        1 => 1,
        _ => {
            let mut a = 0;
            let mut b = 1;
            for _ in 2..=n {
                let temp = a + b;
                a = b;
                b = temp;
            }
            b
        }
    }
}

pub fn add(a: i32, b: i32) -> i32 {
    a + b
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_add() {
        assert_eq!(add(2, 2), 4);
    }

    #[test]
    fn test_fibonacci() {
        assert_eq!(fibonacci(0), 0);
        assert_eq!(fibonacci(1), 1);
        assert_eq!(fibonacci(5), 5);
    }
}
```"#.to_string())
            }
        } else {
            Ok("Autonomous agent response completed.".to_string())
        }
    }
}
