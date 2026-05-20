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

#[derive(Debug, Clone, Serialize)]
pub struct BackupRef {
    pub name: String,
    pub hash: String,
    pub subject: String,
    pub created_at: Option<DateTime<Utc>>,
    pub relative_time: String,
}

impl BackupRef {
    pub fn short_hash(&self) -> String {
        self.hash.chars().take(7).collect()
    }

    pub fn inspect_command(&self) -> String {
        format!("git show --stat --oneline {}", self.name)
    }

    pub fn restore_command(&self) -> String {
        format!("git reset --hard {}", self.name)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RestoreMode {
    HardReset,
    SoftReset,
    Checkout,
}

impl RestoreMode {
    pub fn label(self) -> &'static str {
        match self {
            Self::HardReset => "hard reset",
            Self::SoftReset => "soft reset",
            Self::Checkout => "checkout",
        }
    }

    pub fn command(self, commit_hash: &str) -> String {
        match self {
            Self::HardReset => format!("git reset --hard {commit_hash}"),
            Self::SoftReset => format!("git reset --soft {commit_hash}"),
            Self::Checkout => format!("git checkout {commit_hash}"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct RestoreOutcome {
    pub mode: RestoreMode,
    pub backup_ref: Option<String>,
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

    pub fn list_backup_refs(&self) -> Result<Vec<BackupRef>> {
        let output = Command::new("git")
            .current_dir(&self.repo_path)
            .args([
                "for-each-ref",
                "refs/git-time-machine/backups",
                "--format=%(refname)%00%(objectname)%00%(subject)",
            ])
            .output()
            .context("Failed to list git-time-machine backup refs")?;

        if !output.status.success() {
            let error = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("Failed to list git-time-machine backup refs: {}", error);
        }

        let refs_output = String::from_utf8(output.stdout)?;
        let mut backup_refs = Vec::new();
        for line in refs_output.lines() {
            let parts: Vec<&str> = line.splitn(3, '\x00').collect();
            if parts.len() < 3 {
                continue;
            }

            let name = parts[0].to_string();
            let hash = parts[1].to_string();
            let subject = parts[2].to_string();
            let created_at = Self::backup_ref_created_at(&name);
            let relative_time = created_at
                .as_ref()
                .map(Self::format_relative_time)
                .unwrap_or_else(|| "unknown".to_string());

            backup_refs.push(BackupRef {
                name,
                hash,
                subject,
                created_at,
                relative_time,
            });
        }

        backup_refs.sort_by(|a, b| {
            b.created_at
                .cmp(&a.created_at)
                .then_with(|| b.name.cmp(&a.name))
        });

        Ok(backup_refs)
    }

    pub fn restore_to_commit(
        &self,
        commit_hash: &str,
        mode: RestoreMode,
    ) -> Result<RestoreOutcome> {
        Self::validate_commit_hash(commit_hash)?;

        let backup_ref = if mode == RestoreMode::HardReset {
            Some(self.create_backup_ref()?)
        } else {
            None
        };

        let args = match mode {
            RestoreMode::HardReset => vec!["reset", "--hard", commit_hash],
            RestoreMode::SoftReset => vec!["reset", "--soft", commit_hash],
            RestoreMode::Checkout => vec!["checkout", commit_hash],
        };
        let output = Command::new("git")
            .current_dir(&self.repo_path)
            .args(args)
            .output()
            .with_context(|| format!("Failed to run {}", mode.command(commit_hash)))?;

        if !output.status.success() {
            let error = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("Failed to {}: {}", mode.label(), error);
        }

        Ok(RestoreOutcome { mode, backup_ref })
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
        if Self::validate_commit_hash(commit_hash).is_err() {
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
        if Self::validate_commit_hash(commit_hash).is_err() {
            return Ok("Invalid commit hash format".to_string());
        }

        let output = Command::new("git")
            .current_dir(&self.repo_path)
            .args(["diff", "--no-color", "HEAD", commit_hash])
            .output()
            .context("Failed to get full diff")?;

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

    fn validate_commit_hash(commit_hash: &str) -> Result<()> {
        if !commit_hash.is_empty() && commit_hash.chars().all(|c| c.is_ascii_hexdigit()) {
            Ok(())
        } else {
            anyhow::bail!("Invalid commit hash format")
        }
    }

    fn create_backup_ref(&self) -> Result<String> {
        let output = Command::new("git")
            .current_dir(&self.repo_path)
            .args(["rev-parse", "HEAD"])
            .output()
            .context("Failed to read current HEAD before hard reset")?;

        if !output.status.success() {
            let error = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("Failed to read current HEAD before hard reset: {}", error);
        }

        let current_head = String::from_utf8(output.stdout)?.trim().to_string();
        Self::validate_commit_hash(&current_head)?;
        let short_head = &current_head[..7.min(current_head.len())];
        let backup_ref = format!(
            "refs/git-time-machine/backups/{}-{}",
            Utc::now().timestamp_millis(),
            short_head
        );

        let output = Command::new("git")
            .current_dir(&self.repo_path)
            .args(["update-ref", &backup_ref, "HEAD"])
            .output()
            .context("Failed to create hard-reset backup ref")?;

        if !output.status.success() {
            let error = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("Failed to create hard-reset backup ref: {}", error);
        }

        Ok(backup_ref)
    }

    fn backup_ref_created_at(ref_name: &str) -> Option<DateTime<Utc>> {
        let backup_name = ref_name.strip_prefix("refs/git-time-machine/backups/")?;
        let (timestamp_millis, _) = backup_name.split_once('-')?;
        let timestamp_millis = timestamp_millis.parse::<i64>().ok()?;
        DateTime::from_timestamp_millis(timestamp_millis)
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
    fn get_full_diff_compares_head_to_selected_commit() {
        let repo = TestRepo::new("full-diff");
        repo.write_file("first\n");
        let first_hash = repo.commit("first commit");
        repo.write_file("second\n");
        repo.commit("second commit");

        let diff = repo
            .manager()
            .get_full_diff(&first_hash)
            .expect("full diff should load");

        assert!(diff.contains("diff --git"), "full diff was: {diff}");
        assert!(diff.contains("-second"), "full diff was: {diff}");
        assert!(diff.contains("+first"), "full diff was: {diff}");
    }

    #[test]
    fn hard_reset_creates_backup_ref_before_resetting() {
        let repo = TestRepo::new("hard-reset");
        repo.write_file("first\n");
        let first_hash = repo.commit("first commit");
        repo.write_file("second\n");
        let second_hash = repo.commit("second commit");

        let outcome = repo
            .manager()
            .restore_to_commit(&first_hash, RestoreMode::HardReset)
            .expect("hard reset should restore");
        let backup_ref = outcome
            .backup_ref
            .expect("hard reset should create backup ref");

        assert_eq!(outcome.mode, RestoreMode::HardReset);
        assert_eq!(git(&repo.path, &["rev-parse", "HEAD"]).trim(), first_hash);
        assert_eq!(
            git(&repo.path, &["rev-parse", &backup_ref]).trim(),
            second_hash
        );
        assert_eq!(
            fs::read_to_string(repo.path.join("file.txt"))
                .expect("file should exist after hard reset"),
            "first\n"
        );
    }

    #[test]
    fn list_backup_refs_returns_recovery_commands_for_hard_reset_backups() {
        let repo = TestRepo::new("list-backups");
        repo.write_file("first\n");
        let first_hash = repo.commit("first commit");
        repo.write_file("second\n");
        let second_hash = repo.commit("second commit");

        let outcome = repo
            .manager()
            .restore_to_commit(&first_hash, RestoreMode::HardReset)
            .expect("hard reset should restore");

        let backups = repo
            .manager()
            .list_backup_refs()
            .expect("backup refs should load");

        assert_eq!(backups.len(), 1);
        assert_eq!(backups[0].name, outcome.backup_ref.unwrap());
        assert_eq!(backups[0].hash, second_hash);
        assert_eq!(backups[0].short_hash(), second_hash[..7]);
        assert!(backups[0].subject.contains("second commit"));
        assert!(backups[0].created_at.is_some());
        assert_eq!(
            backups[0].inspect_command(),
            format!("git show --stat --oneline {}", backups[0].name)
        );
        assert_eq!(
            backups[0].restore_command(),
            format!("git reset --hard {}", backups[0].name)
        );
    }

    #[test]
    fn soft_reset_moves_head_without_changing_worktree() {
        let repo = TestRepo::new("soft-reset");
        repo.write_file("first\n");
        let first_hash = repo.commit("first commit");
        repo.write_file("second\n");
        repo.commit("second commit");

        let outcome = repo
            .manager()
            .restore_to_commit(&first_hash, RestoreMode::SoftReset)
            .expect("soft reset should restore");

        assert_eq!(outcome.mode, RestoreMode::SoftReset);
        assert!(outcome.backup_ref.is_none());
        assert_eq!(git(&repo.path, &["rev-parse", "HEAD"]).trim(), first_hash);
        assert_eq!(
            fs::read_to_string(repo.path.join("file.txt"))
                .expect("file should exist after soft reset"),
            "second\n"
        );
        assert!(
            git(&repo.path, &["status", "--porcelain"]).contains("M  file.txt"),
            "soft reset should leave the newer change staged"
        );
    }

    #[test]
    fn checkout_moves_to_detached_head_without_backup_ref() {
        let repo = TestRepo::new("checkout");
        repo.write_file("first\n");
        let first_hash = repo.commit("first commit");
        repo.write_file("second\n");
        repo.commit("second commit");

        let outcome = repo
            .manager()
            .restore_to_commit(&first_hash, RestoreMode::Checkout)
            .expect("checkout should restore");

        assert_eq!(outcome.mode, RestoreMode::Checkout);
        assert!(outcome.backup_ref.is_none());
        assert_eq!(git(&repo.path, &["rev-parse", "HEAD"]).trim(), first_hash);
        assert_eq!(
            git(&repo.path, &["rev-parse", "--abbrev-ref", "HEAD"]).trim(),
            "HEAD"
        );
    }

    #[test]
    fn invalid_hashes_do_not_run_git_operations() {
        let repo = TestRepo::new("invalid-hashes");
        repo.write_file("first\n");
        repo.commit("initial commit");

        let manager = repo.manager();

        assert!(manager
            .restore_to_commit("HEAD;rm-rf", RestoreMode::HardReset)
            .is_err());
        assert!(manager
            .restore_to_commit("", RestoreMode::HardReset)
            .is_err());
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
