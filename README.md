# Git Reviewer

A lightweight desktop app for reviewing and committing changes across many git repos and
their worktrees, in a single window. It's modeled on VS Code's Source Control panel, for
people whose work spans several repos at once, without keeping an editor window open for
each one.

Runs on Windows, macOS (Apple silicon) and Linux (x64). Built in Rust with
[iced](https://iced.rs).

## Features

- **Every repo in one sidebar.** Point the app at a folder: each repo directly inside it
  shows up with its worktrees. Each branch shows how far it is ahead of or behind its base.
  Repos you're not using can be archived into a collapsible section at the bottom.
- **Changes like in VS Code:** staged and unstaged changes, stage or unstage per file,
  discard with a confirmation, and commit with `Ctrl+Enter` (`⌘+Enter` on macOS).
- **Whole-file diffs** with the changes inline, a line-number gutter that stays in place
  when you scroll sideways, and syntax highlighting (Catppuccin Mocha colors).
- **Remote work:** fetch all repos, sync, and publish a branch. Remove a worktree, or delete
  a branch once it's fully merged into its base.
- **Auto-refresh:** files are watched, so changes made in an editor or the terminal show
  up on their own.
- **Resizable panels**, with widths remembered across restarts.

Git Reviewer runs the `git` command-line tool for everything it does, so your hooks,
commit signing, credential helpers and git config behave exactly as they do in the
terminal.

## Install

Download the latest version from the
[Releases page](https://github.com/Towers77/git-reviewer/releases):

| Platform              | File                                    |
| --------------------- | --------------------------------------- |
| macOS (Apple silicon) | `git-reviewer-<version>-macos-arm64.zip` |
| Windows (x64)         | `git-reviewer-<version>-windows-x64.exe` |
| Linux (x64)           | `git-reviewer-<version>-linux-x64.tar.gz` |

You need [git](https://git-scm.com) installed and on your `PATH`. On macOS, the git that
comes with Xcode's command-line tools works.

The builds aren't code-signed, so each OS warns you the first time:

- **macOS:** unzip, then move **Git Reviewer** into **Applications** and open it. macOS
  blocks it the first time. Go to **System Settings → Privacy & Security**, scroll down,
  click **Open Anyway** and confirm. Each new version asks once more.
- **Windows:** run the `.exe`. If SmartScreen says "Windows protected your PC", click
  **More info → Run anyway**.

On **Linux**, extract it and run its install script, which puts **Git Reviewer** in your
app menu (the files go in `~/.local`):

```sh
tar -xzf git-reviewer-<version>-linux-x64.tar.gz
./git-reviewer-<version>-linux-x64/install.sh
```

`install.sh --uninstall` removes it again. Or skip the script and run `bin/git-reviewer`
from the extracted folder. It needs glibc 2.35
or newer (Ubuntu 22.04, Debian 12, Fedora 36 or later). **Choose folder…** opens your
desktop's folder picker through xdg-desktop-portal, which GNOME, KDE, Cinnamon and most
other desktops come with.

To update, quit the app and replace it with the new download. Your settings are kept.

## Getting started

On first launch, click **Choose folder…** and pick the folder that holds your repos (for
example `~/repos`). Every git repo directly inside it appears in the sidebar. The app
remembers the folder, and you can change it later from the top bar.

You can also pass the folder on the command line, e.g. `git-reviewer ~/repos`.

## Configuration

Settings live in `config.toml`:

| OS      | Folder                                     |
| ------- | ------------------------------------------ |
| Windows | `%APPDATA%\git-reviewer\`                  |
| macOS   | `~/Library/Application Support/git-reviewer/` |
| Linux   | `~/.config/git-reviewer/`                  |

```toml
root = "C:\\Users\\me\\repos"     # the folder whose repos the app shows

[bases]                            # optional: compare a repo against this branch
web-client = "origin/development"  # instead of its origin/HEAD
```

By default each repo's branches are compared with `origin/HEAD`, the remote's default
branch. Use `[bases]` for repos where work merges into a different branch. The key is the
repo's folder name.

The same folder holds `state.toml`, where the app remembers panel widths and archived
repos on its own. You don't need to edit it.

## Building from source

You need a stable [Rust toolchain](https://rustup.rs) (edition 2024).

```sh
git clone https://github.com/Towers77/git-reviewer.git
cd git-reviewer
cargo run --release -- ~/repos
```

On Linux, no libraries beyond a C linker are needed to build: X11, Wayland and the GPU
drivers are loaded when the app starts.

## Contributing

Contributions are welcome. See [CONTRIBUTING.md](CONTRIBUTING.md).

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT license ([LICENSE-MIT](LICENSE-MIT))

at your option.

Unless you explicitly state otherwise, any contribution intentionally submitted for
inclusion in the work by you, as defined in the Apache-2.0 license, shall be dual licensed
as above, without any additional terms or conditions.

Bundled assets keep their own licenses:

- the Fira Code and Inter fonts: SIL Open Font License, in `assets/fonts/`;
- the Lucide icons: ISC, in `assets/icons/Lucide-LICENSE.txt`;
- the Catppuccin file icons: MIT, in `assets/icons/files/LICENSE`;
- the Catppuccin Mocha syntax theme: MIT, in `assets/themes/Catppuccin-MIT.txt`.
