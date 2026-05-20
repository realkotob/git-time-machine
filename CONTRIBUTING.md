# Contributing to git-time-machine

Thanks for wanting to contribute! Here are a few guidelines to keep things smooth.

## Before You Start

- Check the [Issues](https://github.com/dinakars777/git-time-machine/issues) page for open tasks. Issues labeled `good first issue` are great starting points.
- Comment on the issue you want to work on so others know it's taken.

## Pull Request Guidelines

- **One issue per PR.** Keep PRs focused on a single feature or fix. If you want to tackle multiple issues, open separate PRs for each.
- **Link the issue.** Include `Closes #<number>` in your PR description so the issue gets closed automatically when merged.
- **Rebase on latest main.** Before opening your PR, make sure your branch is up to date with `main` to avoid merge conflicts.

## Code Quality

- Run `cargo check` before submitting to make sure everything compiles.
- Run `cargo fmt` to keep formatting consistent.
- Run `cargo clippy` to catch common mistakes.
- If you're adding a new dependency to `Cargo.toml`, explain why it's needed in the PR description.

## What Happens After You Submit

- CI will automatically run `cargo check`, `cargo build`, and `cargo test` on your PR.
- If CI fails, check the logs and push a fix.
- Once reviewed and approved, your PR will be squash-merged into `main`.

## Questions?

Open a [Discussion](https://github.com/dinakars777/git-time-machine/discussions) or comment on the relevant issue. Happy hacking!
