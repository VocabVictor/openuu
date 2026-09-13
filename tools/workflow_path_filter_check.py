"""Would this change set trigger this workflow?

Mirrors GitHub's paths-ignore rule: a push is skipped only when EVERY changed
file matches a pattern. Exists because the way a path filter fails is silent —
a filter written too wide skips the code too, and a skipped run and a filter
working correctly look identical in the Actions tab, which is to say they look
like nothing at all.

So both directions are checked, not just the one being asked for: a
documentation-only set must skip, and each kind of code file must still run.

    python tools/workflow_path_filter_check.py .github/workflows/*.yml

There is one source of evidence the Actions tab does not have, and it belongs
to whoever pushed: they know what was in the push. On 2026-09-14 the session
that pushes master ran

    git diff --name-only origin/master..master | grep -vE '^docs/|\\.md$'

got no output, pushed four documentation commits ending at 9dc2824b9, and no
workflow fired. That is a real negative sample -- a doc-only push that was
correctly skipped -- and nothing in the run history could have supplied it,
because a skipped push leaves no row and its commits are folded into the next
push that does run. **An observer who is also the producer holds information
the record does not.** Reconstructing pushes from consecutive run heads, as
one session did, cannot see it: the doc-only push simply is not there.
"""
import re, sys, pathlib

def to_regex(pat):
    out, i = '', 0
    while i < len(pat):
        if pat.startswith('**/', i):
            out += '(?:.*/)?'; i += 3
        elif pat.startswith('**', i):
            out += '.*'; i += 2
        elif pat[i] == '*':
            out += '[^/]*'; i += 1
        elif pat[i] == '?':
            out += '[^/]'; i += 1
        else:
            out += re.escape(pat[i]); i += 1
    return re.compile('^' + out + '$')

def ignores(path):
    s = pathlib.Path(path).read_text(encoding='utf-8').replace('\r\n', '\n')
    i = s.index('\n  push:')
    j = s.find('\n\n', i)
    block = s[i:j if j > 0 else len(s)]
    if 'paths-ignore:' not in block:
        return None
    pats = re.findall(r'^\s+- "(.+)"$', block[block.index('paths-ignore:'):], re.M)
    return pats

def triggers(wf, files):
    pats = ignores(wf)
    if pats is None:
        return True, 'no filter'
    res = [to_regex(p) for p in pats]
    unmatched = [f for f in files if not any(r.match(f) for r in res)]
    return (len(unmatched) > 0), (unmatched[:3] or 'every file ignored')

CASES = {
 'docs only (negative sample)': [
    'docs/lessons-2026-09-13.md', 'docs/dark-acceptance.md', 'AGENTS.md', 'CLAUDE.md', 'README.md'],
 'rust code (positive sample)': ['src/client/video_queue.rs'],
 'flutter code (positive sample)': ['flutter/lib/desktop/widgets/ui_palette.dart'],
 'a test (positive sample)': ['flutter/test/online_presence_test.dart'],
 'Cargo.lock (positive sample)': ['Cargo.lock'],
 'docs + one code file (mixed)': ['docs/x.md', 'src/client/media.rs'],
}
for wf in sys.argv[1:]:
    print('==', wf)
    for name, files in CASES.items():
        t, why = triggers(wf, files)
        print('   %-34s %s   %s' % (name, 'RUNS' if t else 'skipped', why))
