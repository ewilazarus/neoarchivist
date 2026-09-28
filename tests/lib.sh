# Shared helpers for the test scripts. Source it; don't run it.

repo=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
# The binary under test: `cargo build` first.
bin="$repo/target/debug/neoarchivist"
passed=0
failed=0

pass() { passed=$((passed + 1)); printf '  ok    %s\n' "$1"; }

fail() {
  failed=$((failed + 1))
  printf '  FAIL  %s\n' "$1"
  [ $# -lt 2 ] || printf '%s\n' "$2" | sed 's/^/        /'
}

# check <name> <expected> <actual>
check() {
  if [ "$2" = "$3" ]; then
    pass "$1"
  else
    fail "$1" "$(diff <(printf '%s\n' "$2") <(printf '%s\n' "$3") || true)"
  fi
}

# Prints a summary and exits non-zero if anything failed.
finish() {
  printf '\n%d passed, %d failed\n' "$passed" "$failed"
  [ "$failed" -eq 0 ]
}
