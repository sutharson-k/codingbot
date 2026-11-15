use crate::git;
use crate::graph::node::Node;
use crate::graph::state::{AgentState, IndexedFile, ProjectType, RepoIndex};
use anyhow::Result;
use async_trait::async_trait;
use colored::Colorize;
use std::fs;
use std::path::Path;
use walkdir::WalkDir;

pub struct IndexRepoNode;

#[async_trait]
impl Node for IndexRepoNode {
    fn name(&self) -> &'static str {
        "index_repo"
    }

    async fn execute(&self, state: &mut AgentState) -> Result<()> {
        let repo_path = &state.repo_path;
        qprintln!(state,
            "   {} Indexing repository at '{}'...",
            "[*]".blue(),
            repo_path.display()
        );

        let branch = git::git_current_branch(repo_path).ok();
        let head = git::git_head_commit(repo_path).ok();
        let is_dirty = git::git_is_dirty(repo_path).unwrap_or(false);

        let mut indexed_files = Vec::new();
        let mut project_type = ProjectType::Generic;
        let mut detected_test_cmd = None;

        for entry in WalkDir::new(repo_path)
            .into_iter()
            .filter_entry(|e| !should_ignore(e.path()))
        {
            let entry = match entry {
                Ok(e) => e,
                Err(_) => continue,
            };

            if entry.file_type().is_file() {
                let path = entry.path();
                let rel_path = path
                    .strip_prefix(repo_path)
                    .unwrap_or(path)
                    .to_string_lossy()
                    .replace('\\', "/");

                let ext = path
                    .extension()
                    .and_then(|e| e.to_str())
                    .map(|s| s.to_string());
                let meta = entry.metadata().ok();
                let size_bytes = meta.map(|m| m.len()).unwrap_or(0);

                let is_test = rel_path.contains("test")
                    || rel_path.ends_with("_test.rs")
                    || rel_path.ends_with("test.py");
                let is_entrypoint = rel_path == "src/main.rs"
                    || rel_path == "src/lib.rs"
                    || rel_path == "index.js"
                    || rel_path == "main.py";

                if rel_path == "Cargo.toml" {
                    project_type = ProjectType::Rust;
                    detected_test_cmd = Some("cargo test".to_string());
                } else if rel_path == "package.json" && project_type == ProjectType::Generic {
                    project_type = ProjectType::NodeJs;
                    detected_test_cmd = Some("npm test".to_string());
                } else if (rel_path == "pyproject.toml" || rel_path == "setup.py")
                    && project_type == ProjectType::Generic
                {
                    project_type = ProjectType::Python;
                    detected_test_cmd = Some("pytest".to_string());
                }

                let preview = if size_bytes < 65536
                    && (is_entrypoint
                        || rel_path == "Cargo.toml"
                        || rel_path == "README.md"
                        || is_test)
                {
                    fs::read_to_string(path).ok().map(|content| {
                        content
                            .lines()
                            .take(25)
                            .collect::<Vec<_>>()
                            .join("\n")
                    })
                } else {
                    None
                };

                indexed_files.push(IndexedFile {
                    relative_path: rel_path,
                    extension: ext,
                    size_bytes,
                    is_test,
                    is_entrypoint,
                    preview,
                });
            }
        }

        let summary = format!(
            "Indexed {} files. Project Type: {:?}, Active Branch: {}, Git Dirty: {}",
            indexed_files.len(),
            project_type,
            branch.as_deref().unwrap_or("none"),
            is_dirty
        );

        qprintln!(state, "   {} {}", "[+]".green(), summary);

        state.index = Some(RepoIndex {
            root_path: repo_path.display().to_string(),
            git_branch: branch,
            git_head_commit: head,
            git_dirty: is_dirty,
            total_files: indexed_files.len(),
            files: indexed_files,
            project_type,
            detected_test_command: detected_test_cmd,
            summary: summary.clone(),
        });

        state.record_step("index_repo", summary);
        Ok(())
    }
}

fn should_ignore(path: &Path) -> bool {
    let name = match path.file_name().and_then(|n| n.to_str()) {
        Some(n) => n,
        None => return false,
    };

    name == ".git"
        || name == "target"
        || name == "node_modules"
        || name == ".idea"
        || name == ".vscode"
        || name == "__pycache__"
        || name == ".env"
}
