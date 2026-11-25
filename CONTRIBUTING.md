# Contributing to Git Reviewer

Thanks for helping out. Bug reports, ideas and pull requests are all welcome.

## Reporting bugs and suggesting features

Open an [issue](https://github.com/Towers77/git-reviewer/issues) using one of the
templates. For bugs, include your OS, the app version (from the Releases file name, or
**Get Info** on macOS) and the steps to reproduce. If the bug involves a particular repo
state (a detached HEAD, a rebase in progress, a worktree, …), describe how to set up that
state.

For a bigger change, please open an issue first, so we can agree on the approach before
you put work into it.

## Branches

- **`development`** is where all work lands. It's the default branch.
- **`main`** holds released versions only. Merging `development` into `main` publishes a
  release, and only the maintainer does that.

So: **branch from `development`, and open your pull request against `development`.** A
check fails any other pull request into `main`.

## Making a change

You need a stable [Rust toolchain](https://rustup.rs) and git.

```sh
git clone https://github.com/<you>/git-reviewer.git
cd git-reviewer
git switch development
git switch -c my-change
cargo run -- /path/to/a/folder/with/repos
```

Before you push, run what CI runs:

```sh
cargo fmt --all
cargo clippy --all-targets -- -D warnings
cargo test
```

CI also builds the app for Windows, macOS and Linux on every pull request. The builds
are attached to the CI run if you want to try them.

### Code guidelines

- **All git logic lives in `src/git/`, with no UI code.** The UI (`src/app.rs`,
  `src/app/`) calls into it. Git is never run from `view()`. Its results are cached and
  refreshed explicitly, and the work runs off the UI thread.
- **The app talks to git by running the `git` CLI**, not through a library, so hooks,
  signing and credentials behave as they do in the terminal. Keep it that way.
- **Destructive actions** (discarding changes, removing a worktree) ask for confirmation
  first.
- Add tests for parsing and other logic that doesn't need the UI. The existing tests
  feed git's output in as text (see `src/git/status.rs`), so they don't need a real
  repo.

### Commits and pull requests

- Commit messages and PR titles follow
  [Conventional Commits](https://www.conventionalcommits.org): `feat: …`, `fix: …`,
  `refactor: …`, `chore: …`.
- Keep a pull request to one change. Explain what it does and why, and how you tested
  it. The PR template asks for this.
- Don't bump the version in `Cargo.toml`. That happens in the release pull request.

## License

By contributing, you agree that your contributions are dual licensed under the
[MIT](LICENSE-MIT) and [Apache 2.0](LICENSE-APACHE) licenses, like the rest of the
project, without any additional terms or conditions.
