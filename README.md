# 🕰️ git-time-machine

> **Browse Git reflog visually and recover reachable local history with a TUI**

`git-time-machine` makes git reflog visual and interactive.

<div align="center">

[![Crates.io](https://img.shields.io/crates/v/git-time-machine.svg)](https://crates.io/crates/git-time-machine)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

### [🌐 View Landing Page](https://dinakars777.github.io/git-time-machine/)

</div>

## ✨ The Problem

You just:
- 💥 Moved HEAD and need to find the previous state
- 🗑️ Deleted a local branch and need the last reachable commit
- 🤦 Ran several Git operations and cannot remember the sequence
- 😱 Need to inspect what changed before choosing a recovery point

**Current solution:** Read through `git reflog`, copy hashes, compare candidates, and run the right Git command manually.

**Better:** `git-time-machine` 🎯

## 🚀 Demo

![Demo GIF](demo.gif)

*Navigate your local reflog, preview changes, and restore deliberately.*

## 📦 Installation

### Cargo (Recommended)
```bash
cargo install git-time-machine
```

### From Source
```bash
git clone https://github.com/dinakars777/git-time-machine
cd git-time-machine
cargo install --path .
```

## 🎮 Usage

```bash
# Launch in any git repository
git-time-machine

# Show up to 1000 reflog entries (default: last 50)
git-time-machine --all

# Export the current reflog view as JSON
git-time-machine --export-json
```

### Controls

| Key | Action |
|-----|--------|
| `↑` / `k` | Move up |
| `↓` / `j` | Move down |
| `Home` / `End` | Jump to first/last entry |
| `gg` / `G` | Jump to first/last entry (vim-style) |
| `PgUp` / `PgDn` | Jump 10 entries |
| `Space` | Toggle diff panel |
| `d` | Switch between diff summary and full diff |
| `t` | Toggle relative/absolute timestamps |
| `/` | Search/filter commits by message |
| `Esc` | Clear active filter, or quit if no filter is active |
| `Enter` | Confirm restore to selected state |
| `q` | Quit |

## 🎯 Features

- ✅ **Visual Timeline** - See recent reflog entries at a glance
- ✅ **Relative Timestamps** - "5m ago", "2h ago", "yesterday"
- ✅ **Diff Preview** - Compare the selected entry before restoring
- ✅ **Search/Filter** - Filter commit messages with multi-word search
- ✅ **JSON Export** - Export the reflog timeline for automation
- ✅ **Vim Keybindings** - j/k and gg/G navigation
- ✅ **Beautiful TUI** - Built with Ratatui
- ✅ **Lightning Fast** - Written in Rust
- ✅ **Zero Config** - Just works

## 🔥 Use Cases

### Scenario 1: "I Don't Know What I Did, But It's Broken"
```bash
# You ran a bunch of git commands, something broke
# You don't remember the exact sequence
# git reflog shows 50+ cryptic entries

# With git-time-machine:
git-time-machine
# Visually scan the timeline
# See "2h ago - before I started messing around"
# Preview the diff, then press Enter if it is the right state
```

**Why not `git reflog`?** You'd need to:
1. Read through cryptic hashes and messages
2. Guess which one is "before you broke it"
3. Manually `git reset --hard <hash>`
4. Hope you picked the right one
5. Repeat if wrong

### Scenario 2: Recovering Lost Work After Complex Operations
```bash
# You did: rebase, merge, reset, amend, rebase again
# Now you need code from 3 operations ago
# But you don't remember the exact hash

# With git-time-machine:
git-time-machine
# Scroll through visual timeline
# See relative timestamps: "15m ago", "1h ago"
# Find the state you need
# Preview the diff, then press Enter to restore if it is the right state
```

**Why not `git reflog | grep`?** You'd need to:
1. Know what to grep for
2. Parse timestamps manually
3. Cross-reference multiple entries
4. Still guess which hash is correct

### Scenario 3: Accidental Branch Deletion
```bash
# Deleted a branch recently but forgot the commit hash
# git branch -D feature-branch

# With git-time-machine:
git-time-machine --all
# Scroll back through your history
# Find "checkout: moving from feature-branch"
# Copy the short hash displayed in the UI
# Hit 'q' to exit, then run: git branch feature-branch <hash>
```

**Why not `git reflog --all`?** You'd need to:
1. Scroll through hundreds of lines of text
2. Find the right branch name in the noise
3. Extract the hash manually
4. Remember the git commands to restore it

### Scenario 4: "Undo" After You've Already Committed
```bash
# You committed to the wrong branch
# Then made 3 more commits
# Then realized the mistake
# git revert won't help - you need to go back in time

# With git-time-machine:
git-time-machine
# Find "before I committed to wrong branch"
# Press Enter
# Cherry-pick the commits to the right branch
```

**Why not `git reset`?** You'd need to:
1. Count how many commits back
2. Remember if it's `--soft`, `--mixed`, or `--hard`
3. Hope you counted right
4. Manually re-apply commits if you messed up

## ⚠️ Recovery Boundaries

`git-time-machine` is a reflog browser. It can only help with history that Git can still see locally.

| Situation | Can it help? | Notes |
|-----------|--------------|-------|
| You moved `HEAD` with reset, rebase, merge, amend, or checkout | Usually | If the target commit is still in the local reflog, you can inspect and restore it. |
| You deleted a local branch | Usually | If the branch tip is still reachable from your local reflog, you can find the commit hash and recreate the branch manually. |
| You force-pushed a remote branch | Sometimes | Only if the commits existed in your local clone before the force-push. It cannot recover commits that were never fetched locally. |
| You lost uncommitted, unstashed files | No | Git reflog tracks refs, not arbitrary working-tree file contents. |
| Git garbage-collected the commit | No | If Git has permanently removed the object, this tool cannot bring it back. |

## 🛠️ How It Works

`git-time-machine` is a wrapper around `git reflog` that:

1. Parses your reflog history
2. Displays it in an interactive TUI
3. Lets you preview and restore reachable local states
4. Executes `git reset --hard <hash>` when you press Enter

**It's just Git under the hood** - useful, but not magic. The restore action is destructive and should be confirmed only after previewing the target state.

## 🤔 Why Not Just Use Git Commands?

**You absolutely can!** But here's the reality:

| Task | With git commands | With git-time-machine |
|------|------------------|---------------------|
| Find state from "before I broke it" | `git reflog`, scan 50+ lines, guess hash, `git reset --hard <hash>`, hope it's right | Scroll, preview, confirm |
| Recover a deleted branch's commit hash | `git reflog`, search manually, find hash, `git checkout -b`, verify | Expanded reflog view, scroll, copy hash |
| Undo complex operation sequence | Remember exact commands, count commits, pick right reset flag | Visual timeline, select the "before" state |
| Explore "what if" scenarios | Multiple `git reset` attempts, risk losing more work | Navigate, preview, and confirm from one interface |

**git-time-machine doesn't replace git** - it makes reflog actually usable for humans who:
- Don't memorize commit hashes
- Don't want to grep through 200 lines of text
- Want to see their history visually
- Need to undo mistakes quickly without googling

Think of it as `git reflog` with a visual interface for recovery decisions.

## 🤝 Contributing

Contributions welcome!

**Ideas for future versions:**
- [ ] Safer restore modes: soft reset, mixed reset, checkout preview
- [ ] "Panic mode" - undo last N minutes
- [ ] Branch visualization
- [ ] Stash recovery
- [ ] Copy selected commit hash to clipboard
- [ ] True all-ref reflog mode for deleted branch recovery

## 📝 License

MIT © [Dinakar Sarbada](https://github.com/dinakars777)

## 🌟 Star History

If this saved you once, give it a star! ⭐

---

**Made with ❤️ and Rust** | [Report Bug](https://github.com/dinakars777/git-time-machine/issues) | [Request Feature](https://github.com/dinakars777/git-time-machine/issues)
