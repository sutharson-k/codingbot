use anyhow::{Context, Result};
use std::path::Path;
use std::process::Command;

/// Check if target directory is a Git repository
pub fn is_git_repo(path: &Path) -> bool {
    let output = Command::new("git")
        .args(["rev-parse", "--is-inside-work-tree"])
        .current_dir(path)
        .output();

    matches!(output, Ok(out) if out.status.success())
}

/// Initialize a git repository if not already one
pub fn git_init(path: &Path) -> Result<()> {
    let output = Command::new("git")
        .arg("init")
        .current_dir(path)
        .output()
        .context("Failed to run git init")?;

    if !output.status.success() {
        let err = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("git init failed: {}", err);
    }
    Ok(())
}

/// Get git porcelain status
pub fn git_status(path: &Path) -> Result<String> {
    let output = Command::new("git")
        .args(["status", "--porcelain"])
        .current_dir(path)
        .output()
        .context("Failed to run git status")?;

    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

/// Check if repo has uncommitted changes
pub fn git_is_dirty(path: &Path) -> Result<bool> {
    let status = git_status(path)?;
    Ok(!status.is_empty())
}

/// Get short commit hash of HEAD
pub fn git_head_commit(path: &Path) -> Result<String> {
    let output = Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .current_dir(path)
        .output()
        .context("Failed to run git rev-parse HEAD")?;

    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    } else {
        Ok("none (initial commit pending)".to_string())
    }
}

/// Get current active branch name
pub fn git_current_branch(path: &Path) -> Result<String> {
    let output = Command::new("git")
        .args(["rev-parse", "--abbrev-ref", "HEAD"])
        .current_dir(path)
        .output()
        .context("Failed to get current branch")?;

    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    } else {
        Ok("main".to_string())
    }
}

/// Create and switch to a new branch
pub fn git_checkout_new_branch(path: &Path, branch_name: &str) -> Result<()> {
    let output = Command::new("git")
        .args(["checkout", "-b", branch_name])
        .current_dir(path)
        .output()
        .context("Failed to run git checkout -b")?;

    if !output.status.success() {
        let err = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("Failed to create branch '{}': {}", branch_name, err);
    }
    Ok(())
}

/// Run git diff against HEAD or working tree
pub fn git_diff(path: &Path) -> Result<String> {
    // Diff unstaged and staged
    let output = Command::new("git")
        .args(["diff", "HEAD"])
        .current_dir(path)
        .output();

    if let Ok(out) = output {
        if out.status.success() && !out.stdout.is_empty() {
            return Ok(String::from_utf8_lossy(&out.stdout).to_string());
        }
    }

    // Fallback to plain working tree diff
    let fallback = Command::new("git")
        .args(["diff"])
        .current_dir(path)
        .output()
        .context("Failed to run git diff")?;

    Ok(String::from_utf8_lossy(&fallback.stdout).to_string())
}

/// Stage all changes and commit
pub fn git_commit_all(path: &Path, message: &str) -> Result<()> {
    // git add -A
    let add_out = Command::new("git")
        .args(["add", "-A"])
        .current_dir(path)
        .output()
        .context("Failed to run git add -A")?;

    if !add_out.status.success() {
        let err = String::from_utf8_lossy(&add_out.stderr);
        anyhow::bail!("git add failed: {}", err);
    }

    // git commit -m message
    let commit_out = Command::new("git")
        .args(["commit", "-m", message])
        .current_dir(path)
        .output()
        .context("Failed to run git commit")?;

    if !commit_out.status.success() {
        let err = String::from_utf8_lossy(&commit_out.stderr);
        anyhow::bail!("git commit failed: {}", err);
    }

    Ok(())
}
