#!/usr/bin/env bash
# ac.sh "<commit message>" <pathspec...>
#
# Runs `flutter analyze` for flutter/, requires the issue count to equal the
# baseline (BASELINE env, default 245), then commits the given pathspecs.
# New files must be `git add`ed first: a pathspec commit only takes files
# that git already tracks. Run from anywhere inside the repository.
set -u
ROOT=$(git rev-parse --show-toplevel)
FLUTTER=${FLUTTER:-flutter}
BASELINE=${BASELINE:-245}
REPORT=${ANALYZE_REPORT:-$ROOT/.dart_tool_analyze.txt}
msg="$1"; shift
(cd "$ROOT/flutter" && "$FLUTTER" analyze --no-pub > "$REPORT" 2>&1)
cd "$ROOT"
grep "issues found" "$REPORT"
if ! grep -q "^$BASELINE issues found" "$REPORT"; then
  grep "^\s*\(error\|warning\) -" "$REPORT" | cut -c1-170 | head -15
  exit 1
fi
git commit -q -m "$msg" -- "$@" && git log -1 --format='%h %s' && git show --stat --format= HEAD | tail -1
