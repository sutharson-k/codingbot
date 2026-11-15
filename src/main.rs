use clap::{Parser, Subcommand};
use colored::Colorize;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use codingbot::agent::build_autonomous_coding_graph;
use codingbot::git;
use codingbot::graph::state::AgentState;
use codingbot::llm::{HeuristicEngine, LlmClient, OpenAiClient};

#[derive(Parser, Debug)]
#[command(
    name = "codingbot",
    author = "Antigravity Agent",
    version = "0.1.0",
    about = "Autonomous coding agent workflow in Rust using a LangGraph-style state machine"
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Run the autonomous coding agent on a target repository
    Run {
        /// Path to the target Git repository (defaults to current directory)
        #[arg(short, long, default_value = ".")]
        repo: PathBuf,

        /// Task description / instruction for the agent
        #[arg(short, long)]
        task: String,

        /// LLM Provider: 'heuristic' (offline/mock) or 'openai' (API endpoint)
        #[arg(short, long, default_value = "heuristic")]
        provider: String,

        /// API key for OpenAI / OpenAI-compatible endpoint
        #[arg(long, env = "OPENAI_API_KEY")]
        api_key: Option<String>,

        /// Base URL for OpenAI-compatible API (e.g., http://localhost:11434/v1 for Ollama)
        #[arg(long, env = "OPENAI_BASE_URL")]
        base_url: Option<String>,

        /// LLM Model name
        #[arg(short, long, env = "CODINGBOT_MODEL")]
        model: Option<String>,

        /// Custom test command (e.g., "cargo test", "npm test")
        #[arg(long)]
        test_cmd: Option<String>,

        /// Maximum self-correction retry attempts
        #[arg(long, default_value_t = 3)]
        max_retries: usize,

        /// Automatically checkout a new Git branch and commit the verified changes
        #[arg(long)]
        create_branch: bool,

        /// File path to save the generated Pull Request markdown
        #[arg(long)]
        export_pr: Option<PathBuf>,

        /// File path to save the generated git diff
        #[arg(long)]
        export_diff: Option<PathBuf>,
    },

    /// Run an end-to-end self-contained autonomous demo with simulated self-correction
    Demo,

    /// Display the LangGraph state machine diagram and node topology
    Graph,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Graph => {
            print_graph_visualization();
        }
        Commands::Demo => {
            run_autonomous_demo().await?;
        }
        Commands::Run {
            repo,
            task,
            provider,
            api_key,
            base_url,
            model,
            test_cmd,
            max_retries,
            create_branch,
            export_pr,
            export_diff,
        } => {
            let llm: Arc<dyn LlmClient> = match provider.to_lowercase().as_str() {
                "openai" => {
                    let key = api_key.unwrap_or_else(|| {
                        eprintln!("Warning: No API key provided, falling back to heuristic engine.");
                        String::new()
                    });
                    if key.is_empty() {
                        Arc::new(HeuristicEngine::new())
                    } else {
                        Arc::new(OpenAiClient::new(key, base_url, model))
                    }
                }
                _ => Arc::new(HeuristicEngine::new()),
            };

            let mut state = AgentState::new(repo.clone(), task, max_retries);

            if let Some(cmd) = test_cmd {
                state.metadata.insert("test_cmd".to_string(), cmd);
            }
            if create_branch {
                state
                    .metadata
                    .insert("create_branch".to_string(), "true".to_string());
            }

            let graph = build_autonomous_coding_graph(llm);
            let final_state = graph.run(state).await?;

            handle_results(&final_state, export_pr.as_deref(), export_diff.as_deref())?;
        }
    }

    Ok(())
}

fn print_graph_visualization() {
    let dummy_llm = Arc::new(HeuristicEngine::new());
    let graph = build_autonomous_coding_graph(dummy_llm);

    println!(
        "\n{}",
        "================================================================"
            .cyan()
            .bold()
    );
    println!(
        "{}",
        "  LANGGRAPH STATE MACHINE WORKFLOW TOPOLOGY"
            .green()
            .bold()
    );
    println!(
        "{}",
        "================================================================"
            .cyan()
            .bold()
    );

    println!(
        "\n{}\n",
        r#"
      [ START ]
          │
          ▼
    ┌───────────────┐
    │  index_repo   │  (Scans files, git metadata, project types, test command)
    └───────┬───────┘
          │
          ▼
    ┌───────────────┐
    │ plan_changes  │  (Generates structured architectural plan & test strategy)
    └───────┬───────┘
          │
          ▼
    ┌───────────────┐
 ┌─►│ generate_code │  (Synthesizes idiomatic code patches & writes to disk)
 │  └───────┬───────┘
 │        │
 │        ▼
 │  ┌───────────────┐
 │  │   run_tests   │  (Executes test suite, captures exit code & compiler errors)
 │  └───────┬───────┘
 │        │
 │        ▼
 │      { Decision Router }
 │        ├── [Tests Passed / All Green] ─────────────────┐
 │        └── [Tests Failed & Iteration < MaxRetries]     │
 │                    │                                   │
 │                    ▼                                   │
 │             ┌──────────────┐                           │
 └─────────────┤ self_correct │                           │
               └──────────────┘                           │
                                                          │
                                                          ▼
                                                 ┌────────────────┐
                                                 │ create_diff_pr │
                                                 └────────┬───────┘
                                                          │
                                                          ▼
                                                       [ END ]
"#
        .bright_white()
    );

    println!("{}\n", "Mermaid Diagram Definition:".yellow().bold());
    println!("```mermaid\n{}\n```", graph.to_mermaid());
}

async fn run_autonomous_demo() -> anyhow::Result<()> {
    println!(
        "\n{}",
        "🚀 INITIALIZING AUTONOMOUS AGENT END-TO-END DEMO"
            .cyan()
            .bold()
    );

    let temp_dir = tempfile::tempdir()?;
    let repo_path = temp_dir.path().to_path_buf();

    println!(
        "   {} Creating sandboxed Git repository at '{}'...",
        "[*]".blue(),
        repo_path.display()
    );

    // Initialize git repo
    git::git_init(&repo_path)?;

    // Configure local git user for the demo repo
    let _ = std::process::Command::new("git")
        .args(["config", "user.name", "CodingBot Agent"])
        .current_dir(&repo_path)
        .output();
    let _ = std::process::Command::new("git")
        .args(["config", "user.email", "agent@codingbot.local"])
        .current_dir(&repo_path)
        .output();

    // Create Cargo.toml in the demo repo
    let cargo_toml = r#"[package]
name = "math_lib"
version = "0.1.0"
edition = "2021"

[dependencies]
"#;
    fs::write(repo_path.join("Cargo.toml"), cargo_toml)?;

    // Create src/lib.rs with an initial simple function
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

    // Initial git commit
    git::git_commit_all(&repo_path, "initial commit")?;

    println!(
        "   {} Sandbox initialized with passing base test.",
        "[+]".green()
    );

    // Define task for autonomous agent
    let task = "Implement the Fibonacci function fibonacci(n: u64) -> u64 with comprehensive unit tests for n=0, 1, 2, 5, 10";

    let mut state = AgentState::new(repo_path.clone(), task, 3);
    state
        .metadata
        .insert("is_demo".to_string(), "true".to_string());
    state
        .metadata
        .insert("create_branch".to_string(), "true".to_string());

    let llm = Arc::new(HeuristicEngine::new());
    let graph = build_autonomous_coding_graph(llm);

    let final_state = graph.run(state).await?;

    println!(
        "\n{}",
        "🎉 DEMO COMPLETED SUCCESSFULLY!".green().bold()
    );
    if let Some(pr) = &final_state.pr_summary {
        println!("\n{}", "Generated Pull Request Preview:".cyan().bold());
        println!("{}", pr.summary_markdown);
    }

    Ok(())
}

fn handle_results(
    state: &AgentState,
    export_pr: Option<&Path>,
    export_diff: Option<&Path>,
) -> anyhow::Result<()> {
    if state.is_successful {
        println!(
            "\n{} Autonomous cycle succeeded! All test verifications passed.",
            "SUCCESS:".green().bold()
        );
    } else {
        println!(
            "\n{} Autonomous cycle finished with issues. (Passed: {})",
            "STATUS:".yellow().bold(),
            state.is_successful
        );
    }

    if let Some(pr) = &state.pr_summary {
        if let Some(path) = export_pr {
            fs::write(path, &pr.summary_markdown)?;
            println!("   {} Saved Pull Request to '{}'", "[+]".green(), path.display());
        }
    }

    if let Some(diff) = &state.diff {
        if let Some(path) = export_diff {
            fs::write(path, diff)?;
            println!("   {} Saved Git diff to '{}'", "[+]".green(), path.display());
        }
    }

    Ok(())
}
