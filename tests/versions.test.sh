#!/usr/bin/env bash
# The plugin's version decides which release the launcher downloads, so it must be the
# binary's version, in every manifest.

set -eu
. "$(dirname "$0")/lib.sh"

echo "versions"

cargo=$(sed -n 's/^version = "\(.*\)"$/\1/p' "$repo/Cargo.toml" | head -n 1)
check "the plugin's version is the crate's" "$cargo" "$(jq -r .version "$repo/.claude-plugin/plugin.json")"
check "and the marketplace's" "$cargo" \
  "$(jq -r '.plugins[] | select(.name == "neoarchivist") | .version' "$repo/.claude-plugin/marketplace.json")"
check "and the launcher reads it" "$cargo" \
  "$(sed -n 's/^  "version": "\(.*\)",$/\1/p' "$repo/.claude-plugin/plugin.json" | head -n 1)"
check "and the binary reports it" "neoarchivist $cargo" "$("$bin" --version)"

finish
