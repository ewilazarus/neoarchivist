#!/usr/bin/env bash
# Tests for `neoarchivist obsidian`, against a fake `obsidian` CLI and `pgrep`.

set -eu
. "$(dirname "$0")/lib.sh"

tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT

mkdir -p "$tmp/project/docs" "$tmp/other/docs" "$tmp/bin" "$tmp/bare"
ln -s "$tmp/project/docs" "$tmp/linked"
vault=$(cd "$tmp/project/docs" && pwd -P)
other=$(cd "$tmp/other/docs" && pwd -P)

# The fake CLI reports $FAKE_TARGET as the focused vault, lists $FAKE_KNOWN as the vaults
# Obsidian knows, and logs the commands it runs.
cat >"$tmp/bin/obsidian" <<'EOF'
#!/bin/sh
echo "$*" >>"$FAKE_LOG"
case "$1" in
  vault) [ -z "$FAKE_TARGET" ] || echo "$FAKE_TARGET" ;;
  vaults) printf '%s\n' "$FAKE_KNOWN" | sed 's/^/docs\t/' ;;
esac
EOF
# `pgrep` finds Obsidian when $FAKE_RUNNING is set.
printf '#!/bin/sh\n[ -n "$FAKE_RUNNING" ]\n' >"$tmp/bin/pgrep"
cp "$tmp/bin/pgrep" "$tmp/bare/pgrep"
chmod +x "$tmp/bin/obsidian" "$tmp/bin/pgrep" "$tmp/bare/pgrep"

export FAKE_LOG="$tmp/log" FAKE_TARGET="$vault" FAKE_KNOWN="$vault" FAKE_RUNNING=1
project="$tmp/project"

# run <args>: runs the script with the fake CLI (or the tools in $BIN), and prints its
# output and exit status.
run() { local out status=0; out=$(PATH="${BIN:-$tmp/bin}:/usr/bin:/bin" "$bin" --project "$project" obsidian "$@" 2>&1) || status=$?; printf '%s [%d]' "$out" "$status"; }

echo "obsidian: check"

check "the CLI targets this vault" "The Obsidian CLI targets this vault, $vault. [0]" "$(run check)"
check "through a symlink" "The Obsidian CLI targets this vault, $vault. [0]" \
  "$(FAKE_TARGET="$tmp/linked" run check)"
check "another vault is focused" \
  "The Obsidian CLI targets $other, not $vault, because it talks to the vault focused last. Switching to this vault's window in Obsidian points it here. Use rg and fd until it targets this vault. [1]" \
  "$(FAKE_TARGET="$other" FAKE_KNOWN="$other
$tmp/linked" run check)"
check "this vault isn't known to Obsidian" \
  "Obsidian hasn't opened $vault as a vault yet, and targets $other. Opening the vault folder with Open folder as vault adds it. Use rg and fd until it targets this vault. [1]" \
  "$(FAKE_TARGET="$other" FAKE_KNOWN="$other" run check)"
check "Obsidian isn't running" \
  "Obsidian isn't running, so its CLI can't answer. Use rg and fd until it targets this vault. [1]" \
  "$(FAKE_RUNNING="" run check)"
check "the CLI reports nothing" \
  "The Obsidian CLI didn't report a vault. Use rg and fd until it targets this vault. [1]" \
  "$(FAKE_TARGET="" run check)"
check "the CLI isn't installed" \
  "The Obsidian CLI isn't installed. Use rg and fd until it targets this vault. [1]" \
  "$(BIN="$tmp/bare" run check)"
check "no vault" "No vault at $tmp/none/docs. [1]" "$(project=$tmp/none run check)"

echo "obsidian: reload"

: >"$FAKE_LOG"
check "reloads this vault" "Reloaded Obsidian on $vault. [0]" "$(run reload)"
check "with app:reload" "command id=app:reload" "$(grep command "$FAKE_LOG")"

: >"$FAKE_LOG"
check "won't reload another vault" \
  "Didn't reload. The Obsidian CLI targets $other, not $vault, because it talks to the vault focused last. Switching to this vault's window in Obsidian points it here. Or ask the user to run Reload app without saving from the command palette, in this vault's window. [1]" \
  "$(FAKE_TARGET="$other" run reload)"
check "and runs no command" "" "$(grep command "$FAKE_LOG" || true)"

: >"$FAKE_LOG"
check "nothing to reload when Obsidian isn't running" \
  "Obsidian isn't running, so there's nothing to reload. It reads the settings when it next starts. [0]" \
  "$(FAKE_RUNNING="" run reload)"
check "and nothing is run" "" "$(cat "$FAKE_LOG")"

finish
