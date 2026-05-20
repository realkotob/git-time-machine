use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::Serialize;
use std::process::Command;

#[derive(Debug, Clone, Serialize)]
pub struct GitEntry {
    pub hash: String,
    pub message: String,
    pub timestamp: DateTime<Utc>,
    pub author: String,
    pub relative_time: String,
}

pub struct GitManager {
    repo_path: String,
}

impl GitManager {
    pub fn new() -> Result<Self> {
        // Check if we're in a git repository
        let output = Command::new("git")
            .args(["rev-parse", "--show-toplevel"])
            .output()
            .context("Failed to execute git command. Are you in a git repository?")?;

        if !output.status.success() {
            anyhow::bail!("Not a git repository");
        }

        let repo_path = String::from_utf8(output.stdout)?.trim().to_string();

        Ok(Self { repo_path })
    }

    pub fn get_reflog_entries(&self, show_all: bool) -> Result<Vec<GitEntry>> {
        let limit = if show_all { "1000" } else { "50" };

        let output = Command::new("git")
            .current_dir(&self.repo_path)
            .args([
                "reflog",
                "--format=%H%x00%s%x00%ct%x00%an",
                &format!("-n{}", limit),
            ])
            .output()
            .context("Failed to get git reflog")?;

        if !output.status.success() {
            anyhow::bail!("Failed to read git reflog");
        }

        let reflog_output = String::from_utf8(output.stdout)?;
        let mut entries = Vec::new();

        for line in reflog_output.lines() {
            let parts: Vec<&str> = line.splitn(4, '\x00').collect();
            if parts.len() >= 4 {
                let hash = parts[0].to_string();
                let message = parts[1].to_string();
                let timestamp_str = parts[2];
                let author = parts[3].to_string();

                if let Ok(timestamp_secs) = timestamp_str.parse::<i64>() {
                    let timestamp =
                        DateTime::from_timestamp(timestamp_secs, 0).unwrap_or_else(Utc::now);
                    let relative_time = Self::format_relative_time(&timestamp);

                    entries.push(GitEntry {
                        hash,
                        message,
                        timestamp,
                        author,
                        relative_time,
                    });
                }
            }
        }

        Ok(entries)
    }

    pub fn restore_to_commit(&self, commit_hash: &str) -> Result<()> {
        // Validate hash is hex-only to prevent command injection
        if !commit_hash.chars().all(|c| c.is_ascii_hexdigit()) {
            anyhow::bail!("Invalid commit hash format");
        }

        let output = Command::new("git")
            .current_dir(&self.repo_path)
            .args(["reset", "--hard", commit_hash])
            .output()
            .context("Failed to restore to commit")?;

        if !output.status.success() {
            let error = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("Failed to restore: {}", error);
        }

        Ok(())
    }

    pub fn has_uncommitted_changes(&self) -> Result<bool> {
        let output = Command::new("git")
            .current_dir(&self.repo_path)
            .args(["status", "--porcelain"])
            .output()
            .context("Failed to check git status")?;

        if !output.status.success() {
            anyhow::bail!("Failed to check for uncommitted changes");
        }

        let status_output = String::from_utf8(output.stdout)?;
        Ok(!status_output.trim().is_empty())
    }

    pub fn get_diff_stat(&self, commit_hash: &str) -> Result<String> {
        // Validate hash is hex-only to prevent command injection
        if !commit_hash.chars().all(|c| c.is_ascii_hexdigit()) {
            return Ok("Invalid commit hash format".to_string());
        }

        let output = Command::new("git")
            .current_dir(&self.repo_path)
            .args(["diff", "--stat", "HEAD", commit_hash])
            .output()
            .context("Failed to get diff stat")?;

        if !output.status.success() {
            let error = String::from_utf8_lossy(&output.stderr);
            return Ok(format!("Error getting diff: {}", error));
        }

        let diff_output = String::from_utf8(output.stdout)?;
        if diff_output.trim().is_empty() {
            Ok("No changes between current HEAD and selected commit.".to_string())
        } else {
            Ok(diff_output)
        }
    }

    pub fn get_full_diff(&self, commit_hash: &str) -> Result<String> {
        if !commit_hash.chars().all(|c| c.is_ascii_hexdigit()) {
            return Ok("Invalid commit hash format".to_string());
        }

        let output = Command::new("git")
            .current_dir(&self.repo_path)
            .args(["show", "--no-color", commit_hash])
            .output()
            .context("Failed to get full diff")?;

        if !output.status.success() {
            let error = String::from_utf8_lossy(&output.stderr);
            return Ok(format!("Error getting diff: {}", error));
        }

        let diff_output = String::from_utf8(output.stdout)?;
        if diff_output.trim().is_empty() {
            Ok("No diff available.".to_string())
        } else {
            Ok(diff_output)
        }
    }

    fn format_relative_time(timestamp: &DateTime<Utc>) -> String {
        let now = Utc::now();
        let duration = now.signed_duration_since(*timestamp);

        let seconds = duration.num_seconds();
        let minutes = duration.num_minutes();
        let hours = duration.num_hours();
        let days = duration.num_days();

        if seconds < 60 {
            format!("{}s ago", seconds)
        } else if minutes < 60 {
            format!("{}m ago", minutes)
        } else if hours < 24 {
            format!("{}h ago", hours)
        } else if days < 7 {
            format!("{}d ago", days)
        } else if days < 30 {
            format!("{}w ago", days / 7)
        } else if days < 365 {
            format!("{}mo ago", days / 30)
        } else {
            format!("{}y ago", days / 365)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        fs,
        path::{Path, PathBuf},
        process::Command,
        time::{SystemTime, UNIX_EPOCH},
    };

    struct TestRepo {
        path: PathBuf,
    }

    impl TestRepo {
        fn new(name: &str) -> Self {
            let nonce = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system clock should be after UNIX epoch")
                .as_nanos();
            let path = std::env::temp_dir().join(format!(
                "git-time-machine-{name}-{}-{nonce}",
                std::process::id()
            ));
            fs::create_dir_all(&path).expect("test repo directory should be created");

            git(&path, &["init"]);
            git(&path, &["config", "user.name", "Test User"]);
            git(&path, &["config", "user.email", "test@example.com"]);
            git(&path, &["config", "core.logAllRefUpdates", "true"]);

            Self { path }
        }

        fn manager(&self) -> GitManager {
            GitManager {
                repo_path: self.path.to_string_lossy().into_owned(),
            }
        }

        fn write_file(&self, contents: &str) {
            fs::write(self.path.join("file.txt"), contents).expect("test file should be written");
        }

        fn commit(&self, message: &str) -> String {
            git(&self.path, &["add", "file.txt"]);
            git(&self.path, &["commit", "-m", message]);
            git(&self.path, &["rev-parse", "HEAD"]).trim().to_string()
        }
    }

    impl Drop for TestRepo {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }

    fn git(repo: &Path, args: &[&str]) -> String {
        let output = Command::new("git")
            .current_dir(repo)
            .args(args)
            .output()
            .expect("git command should run");

        assert!(
            output.status.success(),
            "git {args:?} failed\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );

        String::from_utf8(output.stdout).expect("git stdout should be UTF-8")
    }

    #[test]
    fn get_reflog_entries_returns_recent_commits_first() {
        let repo = TestRepo::new("reflog-order");
        repo.write_file("first\n");
        let first_hash = repo.commit("first commit");
        repo.write_file("second\n");
        let second_hash = repo.commit("second commit");

        let entries = repo
            .manager()
            .get_reflog_entries(false)
            .expect("reflog entries should load");

        assert!(entries.len() >= 2, "expected at least 2 reflog entries");
        assert_eq!(entries[0].hash, second_hash);
        assert!(entries[0].message.contains("second commit"));
        assert_eq!(entries[0].author, "Test User");
        assert!(entries
            .iter()
            .any(|entry| entry.hash == first_hash && entry.message.contains("first commit")));
    }

    #[test]
    fn has_uncommitted_changes_tracks_dirty_worktree() {
        let repo = TestRepo::new("dirty-worktree");
        repo.write_file("committed\n");
        repo.commit("initial commit");

        let manager = repo.manager();
        assert!(!manager
            .has_uncommitted_changes()
            .expect("clean worktree status should load"));

        repo.write_file("changed\n");

        assert!(manager
            .has_uncommitted_changes()
            .expect("dirty worktree status should load"));
    }

    #[test]
    fn get_diff_stat_compares_head_to_selected_commit() {
        let repo = TestRepo::new("diff-stat");
        repo.write_file("first\n");
        let first_hash = repo.commit("first commit");
        repo.write_file("second\n");
        repo.commit("second commit");

        let diff = repo
            .manager()
            .get_diff_stat(&first_hash)
            .expect("diff stat should load");

        assert!(diff.contains("file.txt"), "diff stat was: {diff}");
    }

    #[test]
    fn invalid_hashes_do_not_run_git_operations() {
        let repo = TestRepo::new("invalid-hashes");
        repo.write_file("first\n");
        repo.commit("initial commit");

        let manager = repo.manager();

        assert!(manager.restore_to_commit("HEAD;rm-rf").is_err());
        assert_eq!(
            manager
                .get_diff_stat("HEAD;rm-rf")
                .expect("invalid diff stat should return message"),
            "Invalid commit hash format"
        );
        assert_eq!(
            manager
                .get_full_diff("HEAD;rm-rf")
                .expect("invalid full diff should return message"),
            "Invalid commit hash format"
        );
    }
}
