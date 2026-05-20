# Changelog

## Unreleased

### Added
- Real Git repository regression tests for reflog parsing, dirty-worktree detection, diff previews, and invalid hash handling.
- CI checks for `cargo fmt --check` and `cargo clippy -- -D warnings`.
- Safer restore modes: hard reset, soft reset, and detached checkout.
- Backup refs under `refs/git-time-machine/backups/` before hard resets.
- Confirmation dialog now shows the exact Git command before restore.
- Project roadmap with near-term recovery work, distribution plans, and explicit non-goals.
- `--list-backups` command for finding backup refs and exact recovery commands.

### Changed
- Tightened README, CLI, crate, and landing-page language around what reflog-based recovery can and cannot do.
- Synced documented controls and feature lists with the current `0.3.0` behavior.
- Full diff preview now compares `HEAD` to the selected target, matching restore preview semantics.

## [0.3.0] - 2026-04-10

### Added
- Search/filter mode for reflog entries.
- JSON export with `--export-json`.
- Full diff toggle in the diff panel.
- Relative/absolute timestamp toggle.
- Vim-style `gg` and `G` navigation.

## [0.2.4] - 2026-03-24

### Added
- Shift+J/K vim-style keys now scroll the diff pane (in addition to Shift+↑↓)

### Fixed
- Diff scroll now has upper bound - can't scroll past end of content into blank space
- Updated diff pane title to show "Shift+↑↓ or J/K to scroll"

## [0.2.3] - 2026-03-24

### Added
- Scrollable diff pane - Use Shift+↑/↓ to scroll through large diffs
- Terminal panic recovery - Terminal is properly restored even if the app panics
- Success feedback - Shows confirmation message after successful restore

### Fixed
- Commit messages containing `|` character now parse correctly (switched to null byte delimiter)
- Diff scroll resets to top when navigating to a different commit

## [0.2.2] - 2026-03-24

(Skipped version)

## [0.2.1] - 2026-03-23

### Added
- [y/N] confirmation dialog before destructive reset
- Diff preview pane with Space key (shows `git diff --stat`)
- Uncommitted changes warning in header
- Home/End/PgUp/PgDn keyboard shortcuts
- Conditional confirmation message based on uncommitted changes

### Fixed
- Command injection prevention with hash validation
- Fixed repo_path usage in all git commands
- Removed wrap-around navigation (now clamps at top/bottom)
- Removed duplicate selected_index state
- Fixed string clone in UI rendering

## [0.2.0] - 2026-03-22

### Added
- Initial release with TUI interface
- Reflog navigation with vim-style keybindings
- Visual timeline of git history
- One-key restore to any commit
