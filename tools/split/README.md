# tools/split — mechanical splitter for oversized Dart files

Scripts used to bring `flutter/lib` under the 300-line rule (see
`AGENTS.md`, "File Size Rule"). They move code; they never rewrite logic.
Python 3 and Git Bash (or any POSIX shell) are required. Run the scripts
from the repository root.

## Workflow (incremental, one small commit per step)

```
S=tools/split
# 0. move foo.dart into foo/foo.dart and leave a barrel at the old path
python $S/dartsplit.py init flutter/lib/x/foo.dart
bash $S/ac.sh "refactor(x): move foo.dart into foo/ behind a barrel" flutter/lib/x/foo.dart flutter/lib/x/foo/foo.dart

# 1..n. move declarations (by name) or line ranges of a class (as an
#       extension) into new part files, 1-3 files per commit
python $S/dartsplit.py scan    flutter/lib/x/foo/foo.dart          # top-level declarations
python $S/dartsplit.py members flutter/lib/x/foo/foo.dart _FooState # members of one class
python $S/dartsplit.py move flutter/lib/x/foo/foo.dart flutter/lib/x/foo/bar.dart Bar _BarState
python $S/dartsplit.py move flutter/lib/x/foo/foo.dart flutter/lib/x/foo/foo_build.dart \
    --cut _FooState:_FooBuild:120-260          # lines 120-260 -> extension _FooBuild on _FooState
git add flutter/lib/x/foo/bar.dart flutter/lib/x/foo/foo_build.dart
bash $S/ac.sh "refactor(x): move Bar and the build helpers out of foo.dart" flutter/lib/x/foo/
```

`move` rewrites the entry file in place (removes the moved code, adds the
`part` directive) and writes the target as `part of` the entry. Use
`--append` to add more blocks to an existing part, `--lib` to emit an
independent library (imports copied from the entry) instead of a part.

Line numbers passed to `--cut` are 1-based and refer to the entry file as it
is *right now*: re-run `members` after every `move` (each new part inserts a
`part` line near the top, and forwarders add a line to the class).

## Helpers for extension cuts

* `forwarders.py <dir> setState _setState Class:file.dart ...` — extensions
  cannot call protected members. Rewrites `setState` / `notifyListeners`
  inside `extension ... on Class` bodies to `_setState` / `_notify` and
  inserts `void _setState(VoidCallback fn) => setState(fn);` (or
  `void _notify() => notifyListeners();`) after the class header. Extensions
  that declare a local `setState` parameter (StatefulBuilder) are skipped and
  reported; handle those by hand.
* `qualify_statics.py <root.dart> <Class> <parts...>` — prefixes unqualified
  references to the class's static members with `Class.` inside extension
  bodies.
* `fix_imports.py <analyze-report> <TypeRegex> <package import>` — after
  `flutter analyze` reports "isn't defined for the type 'X'" in callers that
  never imported the library, adds the import to those files (or to the
  library root when the caller is itself a `part`).
* `ac.sh "<message>" <pathspec...>` — analyze, compare with the baseline
  issue count (`BASELINE`, default 245), commit with an explicit pathspec.
  Set `FLUTTER=/path/to/flutter.bat` when it is not on `PATH`.

## Known limits (see the tracked exceptions in AGENTS.md)

* A class body cannot span files; `@override` methods, fields and static
  members cannot move into an extension. A class made of overrides or a
  single 300+ line function stays whole.
* Inside an extension the extended type's members are not in lexical scope,
  so a member shadowed by a top-level name (`ffi`, `pi`) must be written
  `this.ffi`; the scripts do not detect this, the analyzer does.
* Extension members are invisible to callers that receive the object through
  a library they do not import, and to `dynamic` receivers; do not use
  extensions for `web/bridge.dart`, whose callers only see it through a
  conditional import.

## Batch mode (used for the first files, before the incremental rule)

`dartsplit.py split <file> <plan.json>` writes every part in one go from a
plan such as `{"dir": ..., "entry": ..., "parts": true, "files": {"a.dart":
["Decl", ...]}, "classsplit": [{"class": "C", "cuts": [{"lines": [a, b],
"ext": "CExt", "file": "c_ext.dart"}]}]}`; `prune <report> <prefix>` deletes
imports the analyzer flags as unused in the generated files. Prefer the
incremental workflow above.
