#!/usr/bin/env bash
# Install neoarchivist for Claude Code, with kepano's obsidian plugin it depends on.
#
#   curl -fsSL https://raw.githubusercontent.com/ewilazarus/neoarchivist/main/bootstrap.sh | bash
#   curl -fsSL https://raw.githubusercontent.com/ewilazarus/neoarchivist/main/bootstrap.sh | bash -s -- --project
#
# By default the plugins are installed for you, in every project. With --project, run from a
# project's root, they are declared in its .claude/settings.json instead, so everyone who
# opens the project is offered them. --dry-run prints the commands without running them.
#
# It adds the two marketplaces, then installs neoarchivist, which brings obsidian along, or
# updates it if it's already there, so it is safe to run again. It also puts `neoarchivist`
# in ~/.local/bin, for the git checks `/neoarchivist:init --checks` sets up. Then run
# /neoarchivist:init in a project. Needs the `claude` CLI, git and jq.

set -euo pipefail

main() {
  local scope=user dry_run="" arg
  for arg in "$@"; do
    case "$arg" in
      --project) scope=project ;;
      --dry-run) dry_run=1 ;;
      -h | --help) sed -n '2,14p' "${BASH_SOURCE[0]:-/dev/null}" 2>/dev/null | sed 's/^# \{0,1\}//'; return 0 ;;
      *) fail "Unknown option: $arg. Use --project, --dry-run or --help." ;;
    esac
  done

  command -v claude >/dev/null 2>&1 ||
    fail "The claude CLI isn't on your PATH. Install Claude Code first: https://docs.claude.com/en/docs/claude-code"
  command -v jq >/dev/null 2>&1 ||
    fail "jq isn't installed, and this script needs it: install it with your package manager (brew install jq, apt install jq)."
  command -v git >/dev/null 2>&1 ||
    warn "git isn't installed. neoarchivist uses it to number decisions and check the vault."

  if [ "$scope" = project ]; then
    git rev-parse --show-toplevel >/dev/null 2>&1 ||
      warn "This isn't a git repository. --project writes .claude/settings.json here, in $(pwd)."
  fi

  # A project-scope install is listed once per project, with its projectPath.
  local marketplaces plugins here
  here=$(git rev-parse --show-toplevel 2>/dev/null || pwd -P)
  marketplaces=$(listing "claude plugin marketplace list --json" '.[].name')
  plugins=$(listing "claude plugin list --json" \
    '.[] | select(.scope == $scope and (.scope == "user" or .projectPath == $here)) | .id')

  add_marketplace obsidian-skills kepano/obsidian-skills
  add_marketplace neoarchivist ewilazarus/neoarchivist

  if printf '%s\n' "$plugins" | grep -qx 'neoarchivist@neoarchivist'; then
    say "neoarchivist is installed ($scope); updating it."
    run claude plugin marketplace update neoarchivist
    run claude plugin update neoarchivist@neoarchivist --scope "$scope"
  else
    say "Installing neoarchivist ($scope), with the obsidian plugin it depends on."
    run claude plugin install neoarchivist@neoarchivist --scope "$scope"
  fi

  link_cli

  echo
  say "Done. Restart Claude Code, open a project, and run /neoarchivist:init to set up its vault."
  [ "$scope" = user ] ||
    say "Commit .claude/settings.json, so everyone who opens the project is offered the plugins."
}

say() { printf 'neoarchivist: %s\n' "$*"; }
warn() { printf 'neoarchivist: warning: %s\n' "$*" >&2; }
fail() { printf 'neoarchivist: %s\n' "$*" >&2; exit 1; }

# run <command...>: runs it, or only prints it with --dry-run. stdin is closed, so nothing
# reads the rest of this script when it arrives through a pipe.
run() {
  printf '  $ %s\n' "$*"
  [ -n "$dry_run" ] || "$@" </dev/null
}

# listing <command> <jq filter>: the names a JSON listing reports, one per line, or nothing
# when the listing fails, in which case every step simply runs.
listing() {
  $1 </dev/null 2>/dev/null | jq -r --arg scope "$scope" --arg here "${here:-}" "$2" 2>/dev/null || true
}

add_marketplace() {
  local name=$1 repo=$2
  if [ "$scope" = user ] && printf '%s\n' "$marketplaces" | grep -qx "$name"; then
    say "The $name marketplace is already added."
    return 0
  fi
  if [ "$scope" = project ] && [ -f .claude/settings.json ] &&
    jq -e --arg n "$name" '.extraKnownMarketplaces[$n]' .claude/settings.json >/dev/null 2>&1; then
    say "The $name marketplace is already declared in .claude/settings.json."
    return 0
  fi
  say "Adding the $name marketplace ($repo)."
  run claude plugin marketplace add "$repo" --scope "$scope"
}

# Puts `neoarchivist` on the PATH, as a wrapper around the installed plugin's own
# launcher, so git hooks run the same version Claude does. Rerunning this script after an
# update points it at the new version.
link_cli() {
  local install target="$HOME/.local/bin/neoarchivist"
  if [ -n "$dry_run" ]; then
    printf '  $ write %s\n' "$target"
    return 0
  fi
  install=$(claude plugin list --json </dev/null 2>/dev/null |
    jq -r '[.[] | select(.id == "neoarchivist@neoarchivist")][0].installPath // empty' 2>/dev/null || true)
  if [ -z "$install" ] || [ ! -x "$install/bin/neoarchivist" ]; then
    warn "Couldn't find the installed plugin, so neoarchivist isn't on your PATH. The git checks skip until it is."
    return 0
  fi
  mkdir -p "$(dirname "$target")"
  printf '#!/bin/sh\nexec "%s/bin/neoarchivist" "$@"\n' "$install" >"$target"
  chmod +x "$target"
  say "neoarchivist is on your PATH at $target."
  case ":$PATH:" in *":$HOME/.local/bin:"*) ;; *) warn "Add ~/.local/bin to your PATH for git to find it." ;; esac
}

main "$@"
