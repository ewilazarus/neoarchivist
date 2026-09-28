---
name: preset
description: Save the Obsidian settings of this project's docs vault as a named preset, or apply a saved preset so a new vault looks and behaves like one you already have. Settings only, never notes or vault structure. Use when the user runs /neoarchivist:preset, or asks to save, capture, reuse, copy or apply their Obsidian settings, set-up or look to another vault.
disable-model-invocation: true
context: fork
agent: neoarchivist:agent
background: false
allowed-tools: Bash(${CLAUDE_PLUGIN_ROOT}/bin/neoarchivist *)
---

# Obsidian setting presets

You're running this as neoarchivist:agent, for the user's `/neoarchivist:preset` command. Your
final message is for the user to read, in Markdown, not JSON. Where these steps say to ask
the user, end your reply with the numbered questions, and say they can answer in the
conversation for Claude to pass on; carry on when their answers arrive.

A preset is a snapshot of a vault's Obsidian settings, kept outside any project. The
plugin ships no opinions of its own. Whatever a preset holds, the user chose.

neoarchivist does the saving and merging, so both behave the same way every time:

```bash
${CLAUDE_PLUGIN_ROOT}/bin/neoarchivist preset list
${CLAUDE_PLUGIN_ROOT}/bin/neoarchivist preset save  <name>
${CLAUDE_PLUGIN_ROOT}/bin/neoarchivist preset plan  <name>
${CLAUDE_PLUGIN_ROOT}/bin/neoarchivist preset apply <name> [--prefer preset]
```

Run it from the project root: it works on the project's vault. Presets live in
`$XDG_CONFIG_HOME/neoarchivist/presets/<name>/`, or `~/.config/neoarchivist/presets/`.

## What a preset holds

- **Settings files:** `app.json`, `appearance.json`, `core-plugins.json`,
  `daily-notes.json`, `hotkeys.json`, `types.json` and `graph.json`. For `graph.json` it
  leaves out the zoom level and the last search, which only record where the view was left.
- **Community plugins:** their IDs and each plugin's `data.json` settings, never their code.
  `apply` reports them as not installed, and they're installed fresh.
- **CSS snippets** from `snippets/`.
- **The layout:** the sidebars and ribbon from `workspace.json`, meaning which panels
  they hold, in what order, and how wide. It's saved as `layout.json`, with search text
  cleared and any note references removed.

Never notes, folders or `Conventions.md`, and never which notes are open.

## Save

1. Ask for a name if the user didn't give one. Suggest a plain word such as `mine`.
2. Run `preset save <name>`. It refuses to replace an existing preset. If the user
   wants to replace one, show them its path and let them delete it.
3. Report the files it saved and where the preset lives.

## Apply

1. **No name given:** run `preset list`. With one preset, offer it. With several, ask
   which one.
2. **Plan first.** Run `preset plan <name>` and show the user its output. Nothing
   changes yet. `add` lines fill in settings the vault doesn't have. `conflict` lines are
   settings the vault already sets differently. A vault that Obsidian has opened always has
   a layout of its own, so `conflict layout` is expected there.
3. **Settle conflicts.** If there are any, ask whether to keep the vault's values (the
   default) or take the preset's (`--prefer preset`). The choice applies to every conflict
   in the run. Taking the preset's layout replaces only the sidebars and ribbon. The
   notes open in the vault stay open.
4. **Apply** with `preset apply <name>`, plus `--prefer preset` if chosen. It's safe to
   re-run.
5. **Install missing plugins.** For each `not installed:` line, offer to install it through
   the Obsidian CLI. This downloads third-party code, so get the user's OK for each one.
   Check first that the CLI targets this project's vault, and install only if the check
   passes:

   ```bash
   ${CLAUDE_PLUGIN_ROOT}/bin/neoarchivist obsidian check
   obsidian plugin:install id=<id>
   obsidian plugin:enable id=<id> filter=community
   ```

   If the CLI isn't available or restricted mode is on, give the user the plugin IDs to
   install from Obsidian's Community plugins settings. Their saved settings are already
   in place.
6. **Graph colours.** If the plan says colour groups match folders this vault doesn't have,
   offer `/neoarchivist:graph` to recolour the graph for this vault's layout.
7. **Reload Obsidian.** Obsidian reads these files only at startup, and writes its
   in-memory settings back over them when a setting changes. It rewrites
   `workspace.json` whenever a pane moves, so reload promptly after applying a layout.
   Offer it, and with the user's OK run
   `${CLAUDE_PLUGIN_ROOT}/bin/neoarchivist obsidian reload`. It reloads only after
   checking that the CLI targets this vault, and says what to do if it didn't.

## Nothing to record

Settings aren't vault content. Most of these files are gitignored as personal settings,
and git records the rest. Don't write a memo unless the user asks for one.
