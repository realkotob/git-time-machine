# Roadmap

This roadmap keeps git-time-machine focused on trustworthy local recovery, not on
becoming a full Git client. It should stay small enough that users can understand
what is planned, what is intentionally out of scope, and which tradeoffs matter
before they trust the tool in a repository they care about.

## Product Principles

- Technical truth over hype. The tool should describe Git recovery boundaries
  accurately and avoid implying it can recover data Git can no longer see.
- Safe recovery before convenience. Destructive actions need previews,
  confirmation, and a clear fallback path.
- Local-only Git semantics. The tool should operate on local repository data and
  should not require accounts, network access, telemetry, or cloud state.
- Focused reflog recovery. The goal is a useful reflog and recovery TUI, not a
  replacement for Git, lazygit, gitui, or a hosted backup product.
- Small, reviewable changes. Feature work should land through narrow pull
  requests with regression tests for Git behavior.

## Current Baseline

Recent work established the foundation for safer iteration:

- README, CLI, crate, and landing-page language now describe reflog-based
  recovery boundaries more precisely.
- CI runs `cargo fmt --check`, `cargo clippy -- -D warnings`, and tests.
- Restore now supports hard reset, soft reset, and detached checkout modes.
- Hard reset creates a backup ref under `refs/git-time-machine/backups/`.
- Backup refs can be listed with exact inspect and restore commands.
- The confirmation dialog shows the exact Git command before a restore.
- Full diff preview now compares `HEAD` to the selected target, matching restore
  semantics.
- The TUI has a contextual `?` help overlay for current controls and safety
  reminders.
- The selected full commit hash can be copied with `y` when a platform clipboard
  command is available.
- Search matches commit message, hash, author, and relative time.

## Near-Term Work

These are the next practical improvements, ordered by risk and user value.

### Backup ref management

Hard reset backup refs can now be listed with recovery commands. Future work can
make them easier to restore from or prune inside the TUI.

Acceptance criteria:
- A user can restore from a selected backup ref without copying the ref path.
- Backup prune behavior is explicit, confirmed, and test-covered.

### Mixed reset mode

Add a `git reset --mixed <hash>` restore option for users who want to move `HEAD`
and keep changes unstaged in the working tree. This should use the same command
preview and confirmation model as the existing restore modes.

Acceptance criteria:
- Mixed reset has a distinct keybinding and confirmation text.
- The preview explains how mixed reset differs from soft and hard reset.
- Tests cover the Git command selection and confirmation copy.

### True all-ref reflog mode

The `--all` flag currently expands the number of reflog entries shown. A future
mode should expose actual all-ref reflog data for cases such as deleted branch
recovery.

Acceptance criteria:
- Naming does not confuse "more entries" with "all refs".
- The implementation reads the appropriate reflog data for multiple refs.
- Tests cover entries from more than `HEAD` when available.

## Recovery Workflows

These features are useful but need careful design because they can change user
trust quickly if they feel magical or destructive.

### Stash recovery

Open issue: [#11](https://github.com/dinakars777/git-time-machine/issues/11)

Show stash entries alongside the reflog or in a separate view. Applying a stash
should be previewable, while pop/drop behavior should remain explicit and
confirmed.

### Panic mode

Open issue: [#12](https://github.com/dinakars777/git-time-machine/issues/12)

The useful version of panic mode is not a blind destructive shortcut. It should
help users jump to likely recovery points from the last few minutes, then still
show the target commit, diff, command, and confirmation.

### Deleted branch helper

Make deleted-branch recovery more direct by helping users recreate a branch from
a selected commit. The safest first version can show or copy:

```bash
git branch <branch-name> <commit>
```

The tool should avoid guessing the branch name unless the reflog evidence is
clear.

## Navigation and Scale

These improvements matter as repositories and reflogs get larger:

- Filter by operation type, ref, branch-like text, author, hash, and time range.
- Label reflog entries with ref and operation categories where Git exposes that
  information.
- Cache or lazily load expensive diffs so large repositories remain responsive.
- Add clearer empty states and error messages for non-repository directories,
  shallow history, invalid objects, and missing Git executables.

## Distribution and Trust

Distribution should follow repeatable release quality, not get ahead of it.

- Publish cross-platform release binaries once release automation is stable.
- Add Homebrew distribution only after the binary release flow is repeatable.
- Add shell completions and a man page after the CLI surface settles.
- Refresh the README demo after the restore and backup flows are stable.

## Explicit Non-Goals

git-time-machine should not promise to:

- Recover uncommitted, unstashed file contents.
- Recover commits that were never fetched or created in the local clone.
- Recover objects that Git has garbage-collected.
- Replace Git or become a full terminal Git client.
- Upload repository data, enable cloud backup, or add telemetry by default.

## Issue Policy

Keep the roadmap in this file and create GitHub issues only for the next
implementation slice. Each issue should include acceptance criteria, note the
relevant Git behavior, and identify which regression tests need to exist before
the work is considered done.
