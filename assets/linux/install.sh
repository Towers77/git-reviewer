#!/bin/sh
# Installs Git Reviewer for the current user: the binary in ~/.local/bin, the launcher
# entry and its icon in ~/.local/share. Run it from the extracted folder.
# `install.sh --uninstall` removes the same three files.
set -eu

id=io.github.towers77.git-reviewer
src=$(dirname "$0")
bin="$HOME/.local/bin/git-reviewer"
desktop="$HOME/.local/share/applications/$id.desktop"
icon="$HOME/.local/share/icons/hicolor/scalable/apps/$id.svg"

if [ "${1-}" = --uninstall ]; then
  rm -f "$bin" "$desktop" "$icon"
  echo "Removed Git Reviewer."
  exit 0
fi

install -Dm755 "$src/bin/git-reviewer" "$bin"
install -Dm644 "$src/share/icons/hicolor/scalable/apps/$id.svg" "$icon"
# Exec gets the absolute path: ~/.local/bin is often not on the session's PATH until
# the next login, and GNOME and Cinnamon hide an entry whose Exec isn't on PATH.
mkdir -p "$(dirname "$desktop")"
sed "s|^Exec=.*|Exec=\"$bin\"|" "$src/share/applications/$id.desktop" > "$desktop"
echo "Installed Git Reviewer to $bin"
