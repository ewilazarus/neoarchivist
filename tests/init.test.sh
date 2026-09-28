#!/usr/bin/env bash
# Tests for `neoarchivist init`: what plan reports, what apply writes, and what it keeps.

set -eu
. "$(dirname "$0")/lib.sh"

# section: the CLAUDE.md section, for a vault in docs/.
section() { sed 's/{{vault}}/docs/' "$repo/assets/CLAUDE-section.md"; }
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT

# Keep the user's own git settings, such as a global excludes file, out of the tests.
export GIT_CONFIG_GLOBAL=/dev/null GIT_CONFIG_NOSYSTEM=1

# project <name> [git]: makes an empty project, a git repo if asked, and prints its path.
project() {
  mkdir -p "$tmp/$1"
  [ "${2:-}" != git ] || git -C "$tmp/$1" init -q
  printf '%s\n' "$tmp/$1"
}

# run <project> <args>: runs init.sh in the project, and prints its output and any error.
run() { local p=$1; shift; (cd "$p" && "$bin" --project "$p" init "$@" 2>&1 | sed 's/^neoarchivist: //') || true; }

echo "init: a new project"

p=$(project fresh)
check "plan lists every change" "\
.neoarchivist/config.toml: new file, with vault = \"docs\", memos = \"Memos\", decisions = \"Decisions\"
.claude/settings.json: new file
  add extraKnownMarketplaces.obsidian-skills
  add extraKnownMarketplaces.neoarchivist
  add enabledPlugins.obsidian@obsidian-skills
  add enabledPlugins.neoarchivist@neoarchivist
docs/Memos/: new folder
docs/Decisions/: new folder
CLAUDE.md: new file, with the neoarchivist section
.gitignore: new file
  add docs/.obsidian/workspace.json
  add docs/.obsidian/workspace-mobile.json
  add docs/.obsidian/graph.json
  add docs/.obsidian/app.json
  add docs/.obsidian/appearance.json" \
  "$(run "$p" plan)"
check "plan changes nothing" "" "$(ls -A "$p")"

run "$p" apply >/dev/null
check "settings are written" \
  '{"extraKnownMarketplaces":{"obsidian-skills":{"source":{"source":"github","repo":"kepano/obsidian-skills"}},"neoarchivist":{"source":{"source":"github","repo":"ewilazarus/neoarchivist"}}},"enabledPlugins":{"obsidian@obsidian-skills":true,"neoarchivist@neoarchivist":true}}' \
  "$(jq -c . "$p/.claude/settings.json")"
check "folders are created" "yes" "$([ -d "$p/docs/Memos" ] && [ -d "$p/docs/Decisions" ] && echo yes)"
check "CLAUDE.md is the section" "$(section)" "$(cat "$p/CLAUDE.md")"
check ".gitignore says what it holds" "\
# Obsidian: each user's own panes, graph view, editor preferences and theme
docs/.obsidian/workspace.json
docs/.obsidian/workspace-mobile.json
docs/.obsidian/graph.json
docs/.obsidian/app.json
docs/.obsidian/appearance.json" "$(cat "$p/.gitignore")"
check "a second run changes nothing" \
  "Nothing to change: the project already has everything init sets up." "$(run "$p" apply)"

echo "init: keeping what is there"

p=$(project existing git)
mkdir -p "$p/.claude" "$p/docs/Memos" "$p/docs/Decisions" "$p/docs/.obsidian"
cat >"$p/.claude/settings.json" <<'EOF'
{
  "permissions": { "allow": ["Bash(make test)"] },
  "enabledPlugins": { "neoarchivist@neoarchivist": false, "other@market": true }
}
EOF
echo '{"daily-notes": true, "graph": true}' >"$p/docs/.obsidian/core-plugins.json"
echo '["dataview"]' >"$p/docs/.obsidian/community-plugins.json"
printf '# Project\n\nBuild with make.' >"$p/CLAUDE.md"
printf 'node_modules/\ndocs/.obsidian/workspace*.json\n*.json\n!docs/.obsidian/graph.json\n' >"$p/.gitignore"
printf 'docs/.obsidian/app.json\n' >"$p/.git/info/exclude"
check "plan adds only what is missing, and reports what it keeps" "\
.neoarchivist/config.toml: new file, with vault = \"docs\", memos = \"Memos\", decisions = \"Decisions\"
.claude/settings.json:
  add extraKnownMarketplaces.obsidian-skills
  add extraKnownMarketplaces.neoarchivist
  add enabledPlugins.obsidian@obsidian-skills
  kept enabledPlugins.neoarchivist@neoarchivist, already false
docs/.obsidian/daily-notes.json: new file
  add folder \"Memos/\"
CLAUDE.md: append the neoarchivist section
.gitignore:
  add docs/.obsidian/graph.json
  note: the vault has community plugins; --ignore-plugins ignores them too" \
  "$(run "$p" plan)"

run "$p" apply --ignore-plugins >/dev/null
check "other settings are kept" \
  '{"allow":["Bash(make test)"]} false true' \
  "$(jq -c '.permissions' "$p/.claude/settings.json") $(jq '.enabledPlugins["neoarchivist@neoarchivist"]' "$p/.claude/settings.json") $(jq '.enabledPlugins["other@market"]' "$p/.claude/settings.json")"
check "daily notes point at the journal" '{"folder":"Memos/"}' "$(jq -c . "$p/docs/.obsidian/daily-notes.json")"
check "the section is appended after a blank line" \
  "$(printf '# Project\n\nBuild with make.\n\n'; section)" "$(cat "$p/CLAUDE.md")"
check ".gitignore gets only the uncovered lines, in groups" "\
node_modules/
docs/.obsidian/workspace*.json
*.json
!docs/.obsidian/graph.json

# Obsidian: each user's own panes, graph view, editor preferences and theme
docs/.obsidian/graph.json

# Obsidian: community plugins, installed by each user
docs/.obsidian/plugins/" "$(cat "$p/.gitignore")"
check "a second run changes nothing" "\
.claude/settings.json:
  kept enabledPlugins.neoarchivist@neoarchivist, already false
Nothing to change: the project already has everything init sets up." "$(run "$p" apply --ignore-plugins)"

echo "init: the CLAUDE.md section"

p=$(project marked)
mkdir -p "$p/.claude"
printf '# Project\n\n<!-- neoarchivist:start -->\nOld text.\n<!-- neoarchivist:end -->\n\n## Tests\n\nRun make.\n' >"$p/.claude/CLAUDE.md"
check "a marked section is updated in place" ".claude/CLAUDE.md: update the neoarchivist section" \
  "$(run "$p" plan | grep CLAUDE)"
run "$p" apply >/dev/null
check "only the text between the markers changes" \
  "$(printf '# Project\n\n'; section; printf '\n## Tests\n\nRun make.\n')" "$(cat "$p/.claude/CLAUDE.md"; echo)"
check "an up-to-date section is left alone" "" "$(run "$p" plan | grep CLAUDE || true)"

p=$(project unclosed)
printf '<!-- neoarchivist:start -->\nOld text.\n' >"$p/CLAUDE.md"
check "a start marker without an end is refused" \
  "CLAUDE.md has the neoarchivist start marker but no end marker. Fix it by hand, then run init again." \
  "$(run "$p" plan)"
run "$p" apply >/dev/null
check "before anything is changed" "CLAUDE.md" "$(ls -A "$p")"

p=$(project agents)
printf '# Agents\n' >"$p/AGENTS.md"
check "AGENTS.md alone needs a choice" \
  "CLAUDE.md: needs a choice. The project has AGENTS.md but no CLAUDE.md: --agents-md import creates CLAUDE.md importing AGENTS.md, and --agents-md append adds the section to AGENTS.md." \
  "$(run "$p" plan | grep CLAUDE)"
check "apply refuses without the choice" \
  "The project has AGENTS.md but no CLAUDE.md. Pass --agents-md import or --agents-md append." \
  "$(run "$p" apply)"
check "and changes nothing" "AGENTS.md" "$(ls -A "$p")"
run "$p" apply --agents-md import >/dev/null
check "import creates CLAUDE.md with the import first" \
  "$(printf '@AGENTS.md\n\n'; section)" "$(cat "$p/CLAUDE.md")"

p=$(project agents-append)
printf '# Agents\n' >"$p/AGENTS.md"
run "$p" apply --agents-md append >/dev/null
check "append adds the section to AGENTS.md" "$(printf '# Agents\n\n'; section)" "$(cat "$p/AGENTS.md")"
check "and a later run finds it there" "" "$(run "$p" plan | grep -i 'md:' || true)"

echo "init: --checks"

export GIT_AUTHOR_NAME=t GIT_AUTHOR_EMAIL=t@t GIT_COMMITTER_NAME=t GIT_COMMITTER_EMAIL=t@t
version=$("$bin" --version | cut -d ' ' -f 2)
p=$(project checked git)
git -C "$p" remote add origin git@github.com:someone/project.git
run "$p" apply >/dev/null
check "a plan with --checks adds the hook and the workflow" "\
.git/hooks/pre-commit: new file, runs neoarchivist check --staged
.github/workflows/neoarchivist.yml: new file, runs the check on every push and pull request" \
  "$(run "$p" plan --checks)"
run "$p" apply --checks >/dev/null
check "the hook is executable, and the workflow pins this version" "yes" \
  "$([ -x "$p/.git/hooks/pre-commit" ] && grep -q "NEOARCHIVIST_VERSION: \"$version\"" "$p/.github/workflows/neoarchivist.yml" && echo yes)"
check "a second run changes nothing" \
  "Nothing to change: the project already has everything init sets up." "$(run "$p" apply --checks)"

sed -i.bak 's/NEOARCHIVIST_VERSION: .*/NEOARCHIVIST_VERSION: "0.0.1"/' "$p/.github/workflows/neoarchivist.yml" && rm "$p/.github/workflows/neoarchivist.yml.bak"
check "an older workflow is brought to this version, even without --checks" \
  ".github/workflows/neoarchivist.yml: update to neoarchivist $version" "$(run "$p" plan)"
run "$p" apply >/dev/null

# The hook runs `neoarchivist` from the PATH: this build.
export PATH="$repo/target/debug:$PATH"
git -C "$p" add -A && GIT_AUTHOR_DATE=2020-01-01T12:00:00+00:00 git -C "$p" commit -qm setup
printf '## Summary\n\n- Old day.\n' >"$p/docs/Memos/2020-01-01.md"
git -C "$p" add -A && GIT_AUTHOR_DATE=2020-01-01T12:00:00+00:00 git -C "$p" commit -qm old --no-verify
printf '## Summary\n\n- Old day, rewritten.\n' >"$p/docs/Memos/2020-01-01.md"
git -C "$p" add -A
check "the hook stops a commit that rewrites a past memo" "blocked" \
  "$(git -C "$p" commit -qm rewrite >/dev/null 2>&1 && echo committed || echo blocked)"
check "and says why" yes \
  "$( (cd "$p" && git commit -qm rewrite 2>&1) | grep -q "changes a past memo's prose" && echo yes)"
git -C "$p" reset -q --hard
check "without neoarchivist on the PATH, the hook lets the commit through" "committed" \
  "$(printf '\n- More.\n' >>"$p/docs/Memos/2020-01-01.md"; git -C "$p" add -A
     PATH="$(dirname "$(command -v git)"):/usr/bin:/bin" git -C "$p" commit -qm rewrite >/dev/null 2>&1 && echo committed || echo blocked)"

p=$(project foreign-hook git)
printf '#!/bin/sh\nnpm test\n' >"$p/.git/hooks/pre-commit"
check "someone else's pre-commit hook is kept" "\
.git/hooks/pre-commit:
  kept, it is someone else's; add \`neoarchivist check --staged\` to it
CI:
  note: not on GitHub; run \`neoarchivist check --range <base>..HEAD\` in your CI" \
  "$(run "$p" plan --checks | sed -n '/pre-commit:/,$p')"

echo "init: the vault's settings"

p=$(project named)
check "the folders can be named on a first run" "\
.neoarchivist/config.toml: new file, with vault = \"notes\", memos = \"Journal\", decisions = \"ADR\"
notes/Journal/: new folder
notes/ADR/: new folder" \
  "$(run "$p" plan --vault notes --memos Journal --decisions ADR | grep -E '^(\.neoarchivist|notes/)')"
run "$p" apply --vault notes --memos Journal --decisions ADR >/dev/null
check "and are written to the settings file" 'vault = "notes"|memos = "Journal"|decisions = "ADR"' \
  "$(grep -E '^(vault|memos|decisions) =' "$p/.neoarchivist/config.toml" | paste -sd '|' -)"
check "after which a different name is refused" \
  "The project's \`memos\` is already \"Journal\", in .neoarchivist/config.toml. Change it there, if it should change." \
  "$(run "$p" plan --memos Memos)"
check "a folder outside the project is refused" \
  ".neoarchivist/config.toml: \`vault\` must be a folder inside the project, not \`../elsewhere\`." \
  "$(run "$(project outside)" plan --vault ../elsewhere)"

echo "init: problems"

p=$(project broken)
mkdir -p "$p/.claude"
echo '{ not json' >"$p/.claude/settings.json"
check "invalid settings are refused" \
  ".claude/settings.json isn't valid JSON. Fix it, then run init again." "$(run "$p" plan)"

p=$(project daily)
mkdir -p "$p/docs/.obsidian"
echo '["daily-notes"]' >"$p/docs/.obsidian/core-plugins.json"
echo '{"folder": "Daily", "format": "YYYY-MM-DD"}' >"$p/docs/.obsidian/daily-notes.json"
check "a daily notes folder set elsewhere is kept" "\
docs/.obsidian/daily-notes.json:
  kept folder, already \"Daily\"" "$(run "$p" plan | grep -A1 daily-notes)"

finish
