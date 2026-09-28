#!/usr/bin/env bash
# Run every test script against the debug build, with the bash that runs this one. Needs
# cargo, jq and git.
#
#   bash tests/run.sh          # or /bin/bash tests/run.sh, to check bash 3.2 on macOS

set -u
cd "$(dirname "$0")/.." || exit 1
cargo build --quiet || exit 1
status=0
for test in tests/*.test.sh; do
  "$BASH" "$test" || status=1
  echo
done
[ "$status" -eq 0 ] && echo "All tests passed." || echo "Some tests failed."
exit "$status"
