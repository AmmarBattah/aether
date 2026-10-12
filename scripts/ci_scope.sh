#!/usr/bin/env bash
# Reads the paths a pull request changes, one per line, and prints `code=false`
# when every one is documentation, else `code=true`. .github/workflows/ci.yml
# asks before it installs Rust; its push trigger names the same paths.
#
#   git diff --name-only HEAD^1 HEAD | scripts/ci_scope.sh
set -euo pipefail

is_documentation() {
  case "$1" in
    # `cargo test` reads these two, and a workflow is not documentation.
    docs/engines.md | docs/tui.md | .github/workflows/*) return 1 ;;
    *.md | docs/* | LICENSE | .github/*) return 0 ;;
    *) return 1 ;;
  esac
}

paths=0
code=false
# Reads to the end even once the answer is known: the writer must not get SIGPIPE.
while IFS= read -r path; do
  [ -n "$path" ] || continue
  paths=$((paths + 1))
  is_documentation "$path" || code=true
done
# No path at all is not a documentation change: run everything.
[ "$paths" -gt 0 ] || code=true
printf 'code=%s\n' "$code"
