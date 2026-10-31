#!/usr/bin/env bash
# Story ids vs. the tests that cite them.
# Lists stories no test cites, test citations of stories that do not exist,
# and, per story, the test files citing it.
#
# Usage: story_coverage.sh <stories file> <source dir> <test file glob> <story id regex>
#   e.g. story_coverage.sh docs/user-stories.md bot/src 'tests.rs' 'US-[A-Z][0-9]+[a-z]?'
# A story is defined where its id is written in bold and followed by a full
# stop (`**US-D12.`); paths are relative to the repository root.
set -euo pipefail
if [ $# -ne 4 ]; then
  sed -n '6,7p' "$0" >&2
  exit 2
fi
root="$(git -C "$(dirname "$0")" rev-parse --show-toplevel)"
stories="$root/$1"
src="$root/$2"
tests="$3"
id="$4"

# grep exits 1 when it finds nothing, which is a result here, not an error.
defined=$( { grep -oE "\*\*$id\." "$stories" || true; } | grep -oE "$id" | sort -u || true)
cited=$( { grep -rhoE "$id" "$src" --include="$tests" || true; } | sort -u)

echo "## Stories no test cites"
comm -23 <(echo "$defined") <(echo "$cited") || true
echo
echo "## Tests citing stories that do not exist"
for s in $(comm -13 <(echo "$defined") <(echo "$cited")); do
  grep -rlE "$s\b" "$src" --include="$tests" | sed "s|^$root/|  $s  |"
done
echo
echo "## Citations per story"
for s in $defined; do
  files=$( { grep -rlE "$s([^0-9a-z]|$)" "$src" --include="$tests" || true; } | sed "s|^$root/||" | tr '\n' ' ')
  echo "$s: ${files:-—}"
done
