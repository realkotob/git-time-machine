# GitHub-Inspired Improvement Ideas

Date: 2026-05-31

Scope: improve `git-time-machine` by studying adjacent Git recovery, Git TUI, and stash/reflog tools on GitHub. Ideas may borrow product patterns, but should not copy implementation code.

## Local Baseline

`git-time-machine` is currently a focused Rust/Ratatui reflog recovery TUI. It already supports:

- visual HEAD reflog timeline
- relative and absolute timestamps
- diff summary and full diff previews from `HEAD` to the selected target
- hard reset, soft reset, and detached checkout restore modes
- backup refs before hard reset
- backup-ref listing with recovery commands
- contextual help overlay
- copy selected commit hash
- search/filter over commit messages, hashes, authors, and relative times
- JSON export
- real Git repo regression tests

Open project issues at research time lined up with the best outside ideas:

- #18: copy selected commit hash to clipboard (implemented in this slice)
- #12: panic mode / quick undo last N minutes
- #11: stash recovery support

## Repositories Reviewed

| Repo | Why It Matters | Relevant Ideas |
| --- | --- | --- |
| [jesseduffield/lazygit](https://github.com/jesseduffield/lazygit) | Mature terminal Git UI with reflog-backed undo/redo and clear limitation messaging. | Walk users through undo action-by-action, mark undo/redo entries in reflog, offer compare-two-commits flow, keep limitations explicit. |
| [arxanas/git-branchless](https://github.com/arxanas/git-branchless) | High-quality model for repository-state undo beyond a single `HEAD` reflog. | Long-term idea: explain when single-ref reflog is insufficient, detect abandoned/rewrite states, maybe provide a future event-log mode only if it stays local and transparent. |
| [wfxr/forgit](https://github.com/wfxr/forgit) | Lightweight fzf Git wrapper with an interactive reflog viewer and copy bindings. | Add copy hash/ref keybinding, preview toggle ergonomics, optional external diff pager guidance, per-command configuration only where it helps. |
| [bigH/git-fuzzy](https://github.com/bigH/git-fuzzy) | Fzf Git UI with simultaneous list and diff search plus visible command headers. | Improve search so users can search messages and diff contents; show the exact Git command behind preview/restore actions. |
| [gitui-org/gitui](https://github.com/gitui-org/gitui) | Rust terminal Git UI focused on speed, keyboard control, help, stash, search, and async UI. | Add contextual help overlay, keep large-repo responsiveness in mind, consider lazy diff loading/caching before all-ref mode expands volume. |
| [jonas/tig](https://github.com/jonas/tig) | Long-running text-mode Git browser with dedicated `reflog`, `refs`, and `stash` views. | Treat reflog, refs, stash, and backup refs as separate views rather than crowding one timeline. |
| [septcoco/git-unfuck](https://github.com/septcoco/git-unfuck) | Small recovery-first tool that scans reflog plus dangling objects and explains disasters. | Add plain-English recovery diagnostics and a "show recoverable things" report; keep safety boundaries honest. |
| [vlensys/stashpilot](https://github.com/vlensys/stashpilot) | Focused stash TUI with apply/pop/drop/create and confirmations. | For #11, start with stash list + diff preview + apply, then add pop/drop only with strong confirmations. |
| [senkentarou/gss](https://github.com/senkentarou/gss) | Rust stash TUI with push/pop modes, filtering, partial-file stash, copy diff, and tests. | Add stash search, copy stash diff, and real Git integration tests if stash mode lands. |

## Second Research Pass

After choosing the first implementation slice, a second pass focused on the exact UX patterns:

- forgit exposes a copy binding for commit hashes/stash IDs/worktree paths and documents that unsupported Linux clipboards need a configured command.
- GitUI lists context-based help as a core feature so users do not have to memorize every keybinding.
- git-fuzzy emphasizes searching across log and diff context, plus making underlying Git commands visible.

That pushed the first implementation slice toward contextual help, copy selected hash, and a small search upgrade. Backup-ref TUI, true all-ref reflog, stash recovery, and panic mode remain better as separate follow-up PRs.

## Ranked Survivors

### 1. Contextual Help Overlay

Inspired by GitUI and the keybinding-heavy tools.

Current headers are dense and will get harder to scan as mixed reset, copy hash, stash, backups, and all-ref mode land. Add `?` to open a help overlay showing context-specific actions for the current view: timeline, diff, confirmation, search, backup refs, stash. This is low-risk and reduces pressure to keep every command in the header.

Why it fits: improves trust and discoverability without broadening the product.

### 2. Copy Selected Hash / Ref

Inspired by forgit and already tracked by #18.

Add `y` to copy the selected commit hash, backup ref, or stash ref depending on the active view. Use platform-specific clipboard commands behind a small abstraction and show a non-failing message when clipboard support is unavailable.

Why it fits: direct recovery workflows often end in a manual Git command. Copying the exact target is useful even when the user does not restore inside the TUI.

### 3. Search Upgrade: Message, Hash, Operation, Ref, Time

Inspired by forgit/git-fuzzy filtering and the current roadmap's scale section.

The current search is commit-message only. Expand it into field-aware filtering: message, short/full hash, reflog operation text, ref/branch-like text, author, and relative time bucket. Keep simple text search as the default; introduce prefixes only when needed, e.g. `op:rebase`, `hash:abc123`, `ref:feature`.

Why it fits: improves the core "find the state before I broke it" task.

### 4. True All-Ref Reflog View

Inspired by Tig's separate `reflog` and `refs` views, and already planned in the roadmap.

Replace the current `--all` semantics with a clearly named mode that reads reflogs for multiple refs. Show the ref name and operation category per entry. Avoid calling a 1000-entry HEAD reflog "all refs".

Why it fits: deleted branch recovery becomes more real, and the language becomes more technically honest.

### 5. Stash Recovery View

Inspired by stashpilot and gss; already tracked by #11.

Start narrow: list stashes, preview selected stash diff, apply without dropping. Then add pop/drop only after confirmation copy and tests are solid. Do not merge stash entries into the main HEAD reflog timeline at first; use a separate view.

Why it fits: stash is a Git reflog-adjacent recovery surface, but destructive stash actions need a different mental model than commit restore.

### 6. Panic Mode as Guided Triage, Not Blind Undo

Inspired by lazygit undo and git-unfuck, and already tracked by #12.

Implement "last N minutes" as a filtered candidate mode. Show likely recovery points, exact command, diff preview, uncommitted-change warning, and backup ref behavior before any restore. Avoid a one-key destructive rollback.

Why it fits: users in recovery mode want speed, but the repo's product principles put safety before convenience.

### 7. Plain-English Recovery Diagnostics

Inspired by git-unfuck.

Add a non-mutating diagnostic command or view: "what just happened?" It can summarize recent reflog operations, current branch/detached state, ORIG_HEAD, active rebase/merge/cherry-pick state, available backup refs, and likely next safe actions.

Why it fits: this is valuable even when the tool cannot recover data. It reinforces the "technical truth over hype" principle.

### 8. Backup Ref View Inside The TUI

Inspired by the separate-view pattern in Tig/GitUI.

Promote `--list-backups` into a TUI view where users can inspect backup refs, preview diff from current HEAD to backup, copy restore command, restore from backup, and prune selected backup refs with confirmation.

Why it fits: the project already creates backup refs; making them first-class completes that safety loop.

### 9. Better Diff Ergonomics

Inspired by forgit/git-fuzzy and mature Git TUIs.

Add diff content search, optional syntax-colored external pager guidance, and maybe file-level diff navigation. Keep the built-in default pure and portable; document optional integrations like `delta` instead of making them required.

Why it fits: choosing the correct recovery point depends on seeing the right diff quickly.

### 10. Responsiveness Guardrails

Inspired by GitUI's performance focus.

Before all-ref/stash scale up the number of selectable items, add lazy diff loading, cached preview results keyed by target and diff mode, and clear loading/error states for expensive diffs.

Why it fits: recovery tools lose trust quickly when the UI freezes.

## Rejected Or Deferred

- Full Git client features such as staging, committing, pushing, PR integration, branch management, and rebase editing. LazyGit/GitUI already cover this; it conflicts with the repo's focused recovery non-goal.
- Repository-wide operation log like git-branchless. Strong idea, but it is a different architecture and likely too much until the reflog/stash/backup workflow is excellent.
- Custom keybinding configuration. Useful eventually, but premature while the command surface is still small.
- Directly copying source from any reviewed tool. Product patterns are enough; implementation should stay native to this codebase's Rust/Ratatui structure and safety model.

## Suggested Next PR Sequence

1. Add contextual `?` help overlay. Implemented in this slice.
2. Fix #18 with copy selected hash/ref. Implemented for timeline commit hashes in this slice.
3. Improve search fields and labels. Implemented for message, hash, author, and relative time in this slice.
4. Add backup-ref TUI view using existing backup ref model.
5. Implement true all-ref reflog mode.
6. Start #11 with read-only stash list plus diff preview and apply.
7. Build #12 as a guided last-N-minutes candidate view.
