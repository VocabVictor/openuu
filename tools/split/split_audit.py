"""Audit mechanical split/move commits: every non-trivial line a commit removed must reappear
in one of the files the same commit touched.

usage: python tools/split/split_audit.py <repo> <since-date> <out.md> [--all] [--ref master]
Run it before landing a split; a mechanical move scores 0 lines and 0 arms per commit.
Commits: since <since-date> on <ref>, subject matching a refactor/move pattern (or every
non-merge commit with --all). For each commit the parent versions of all touched .rs/.dart/.py
files form the "before" multiset and the commit versions the "after" multiset; lines are
normalised (whitespace, visibility, path prefixes, receiver names) and trivial lines dropped.
"before - after" is what the commit did not carry over, and it is then split three ways, because
a mechanical move legitimately produces two of them:

* renamed -- the line's shape survives with different identifiers. Compared by replacing every
  non-keyword identifier with a placeholder, so a member renamed during a move is recognised
  rather than counted as a loss.
* deduplicated -- the exact line still exists afterwards, just fewer times, which is what
  collapsing two identical call sites into one helper does.
* lost -- neither the text nor the shape is there afterwards. This is the only one the gate
  counts, because it is the only one that can mean code went missing.

`Union::Variant(` patterns are compared separately so a dropped match arm is called out by name
even when its body lines happen to exist elsewhere.
"""
import collections
import re
import subprocess
import sys

repo, since, out = sys.argv[1], sys.argv[2], sys.argv[3]
scan_all = '--all' in sys.argv
ref = sys.argv[sys.argv.index('--ref') + 1] if '--ref' in sys.argv else 'master'
MOVE_RE = re.compile(r'^refactor|^style.*(?:split|move)|^chore.*(?:split|move)', re.I)
MOVE_HINT = re.compile(r'split|move|rename|out of|into|->|→|extract', re.I)
SRC_EXT = ('.rs', '.dart', '.py')

def git(*a):
    return subprocess.run(['git', '-C', repo, *a], capture_output=True, text=True,
                          encoding='utf-8', errors='replace').stdout

def show(rev, path):
    r = subprocess.run(['git', '-C', repo, 'show', f'{rev}:{path}'], capture_output=True)
    return r.stdout.decode('utf-8', 'replace').splitlines() if r.returncode == 0 else None

DROP = [
    re.compile(r'^$'), re.compile(r'^//'), re.compile(r'^/\*|^\*|^\*/'), re.compile(r'^#\['),
    re.compile(r'^(pub(\([^)]*\))?\s+)?(use|mod)\s'), re.compile(r'^import\s|^export\s|^part\s'),
    re.compile(r'^[{}()\[\];,]+$'), re.compile(r'^(\}\s*)?else\s*\{?$'), re.compile(r'^\}\s*(else\s*)?\{?[,;)]*$'),
    re.compile(r'^(break|continue|return|true|false|None|Ok\(\(\)\)|_ => \{\}|\.\.Default::default\(\))[,;]?$'),
    re.compile(r'^\)\s*(\.await)?[?;,]*$'), re.compile(r'^\.await[?;,]*$'),
]
def normalise(line):
    s = line.strip()
    s = re.sub(r'\bpub\((super|crate|in [\w:]+)\)\s*', '', s)
    s = re.sub(r'\bpub\s+', '', s)
    s = re.sub(r'\b(super::|crate::|self::)+', '', s)
    s = re.sub(r'\b(conn|rs|remote|client)\.', 'self.', s)
    s = re.sub(r'\s+', ' ', s)
    return s

def keep(s):
    if len(s) < 14:
        return False
    return not any(p.match(s) for p in DROP)

# Words that carry meaning rather than naming something, so blanking them would make two
# genuinely different lines look alike. `self`/`this` are deliberately NOT among them:
# moving code out of a class turns the receiver into an ordinary parameter, which is the
# commonest way a mechanical move changes a line's shape.
KEYWORDS = set(
    'as async await break const continue crate dyn else enum extern false fn for if impl in '
    'let loop match mod move mut pub ref return static struct super trait true type '
    'unsafe use where while abstract class extends factory final get implements import is new '
    'null on operator part rethrow set show switch throw try var void with yield def '
    'elif except lambda not or and pass raise'.split())
IDENT = re.compile(r'[A-Za-z_][A-Za-z0-9_]*')

def skeleton(s):
    """The line with every name blanked, so only its shape is left."""
    sk = IDENT.sub(lambda m: m.group(0) if m.group(0) in KEYWORDS else '_', s)
    # A move often drops the qualifier a name needed in its old home, turning
    # `Owner._field` into `_field`. That is the same line, so the shape ignores the chain.
    return re.sub(r'(_\.)+_', '_', sk)

def classify(missing, after):
    """Split what a commit did not carry over into renamed, deduplicated and genuinely lost.

    A line is only called a rename when a line of the same shape actually exists afterwards to
    account for it, and each such line accounts for one removal, not for any number of them.
    """
    shapes = collections.Counter()
    for line, cnt in after.items():
        shapes[skeleton(line)] += cnt
    renamed, deduped, lost = collections.Counter(), collections.Counter(), collections.Counter()
    for line, cnt in missing.items():
        if after[line]:
            deduped[line] = cnt
            continue
        sk = skeleton(line)
        take = min(cnt, shapes[sk])
        if take:
            renamed[line] = take
            shapes[sk] -= take
        if cnt - take:
            lost[line] = cnt - take
    return lost, renamed, deduped

def arms(lines):
    c = collections.Counter()
    for l in lines:
        for m in re.finditer(r'Union::(\w+)\s*[\(\{=]', l):
            c[m.group(1)] += 1
        for m in re.finditer(r'^\s*ipc::Data::(\w+)', l):
            c['ipc::Data::' + m.group(1)] += 1
    return c

def commits():
    log = git('log', f'--since={since}', '--no-merges', '--format=%H%x09%ad%x09%s', '--date=short', ref)
    for row in log.splitlines():
        h, d, s = row.split('\t', 2)
        if scan_all or (MOVE_RE.search(s) and MOVE_HINT.search(s)):
            yield h, d, s

report = [f'# Split audit of `{repo.rstrip("/").split("/")[-1]}` since {since}', '',
          'Each refactor/move commit: lines its parent had in the touched files that no touched file',
          'has after the commit (normalised; comments, imports, attributes, braces and other trivial',
          'lines ignored). `Union::` / `ipc::Data::` arm names are compared separately.', '']
summary = []
for h, d, s in commits():
    status = git('show', '--name-status', '-M', '--format=', h).splitlines()
    touched = []
    for row in status:
        parts = row.split('\t')
        if parts[0].startswith('R'):
            touched.append((parts[1], parts[2]))
        elif parts[0] == 'D':
            touched.append((parts[1], None))
        elif parts[0] == 'A':
            touched.append((None, parts[1]))
        elif parts[0] == 'M':
            touched.append((parts[1], parts[1]))
    touched = [(a, b) for a, b in touched if (a or b).endswith(SRC_EXT)]
    if not touched:
        continue
    before, after = collections.Counter(), collections.Counter()
    before_raw, after_raw = [], []
    for a, b in touched:
        if a:
            ls = show(h + '^', a) or []
            before_raw += ls
            before.update(n for n in map(normalise, ls) if keep(n))
        if b:
            ls = show(h, b) or []
            after_raw += ls
            after.update(n for n in map(normalise, ls) if keep(n))
    missing = before - after
    lost, renamed, deduped = classify(missing, after)
    lost_arms = arms(before_raw) - arms(after_raw)
    n_missing = sum(lost.values())
    summary.append((h[:9], d, s, n_missing, sum(lost_arms.values()),
                    sum(renamed.values()), sum(deduped.values())))
    if n_missing == 0 and not lost_arms:
        continue
    report.append(f'## {h[:9]} {d} {s}')
    report.append('')
    report.append('Files: ' + ', '.join(f'`{a or "+"}`→`{b or "-"}`' if a != b else f'`{a}`' for a, b in touched))
    report.append('')
    if lost_arms:
        report.append('**Match arms lost:** ' + ', '.join(f'`{k}`×{v}' for k, v in sorted(lost_arms.items())))
        report.append('')
    if renamed or deduped:
        report.append(f'Accounted for: {sum(renamed.values())} renamed, '
                      f'{sum(deduped.values())} deduplicated.')
        report.append('')
    if n_missing:
        report.append(f'{n_missing} removed line(s) not found in any touched file afterwards:')
        report.append('')
        report.append('```')
        for line, cnt in list(lost.items())[:60]:
            report.append((f'{cnt}x ' if cnt > 1 else '') + line[:160])
        if len(lost) > 60:
            report.append(f'... {len(lost) - 60} more')
        report.append('```')
        report.append('')

def chain_audit():
    """For every `X.rs -> X/mod.rs` (or `X.dart -> X/X.dart`) rename since <since>, compare the
    parent's X with the union of X/** at <ref> (the end of the whole chain)."""
    rows = []
    log = git('log', f'--since={since}', '--no-merges', '--format=%H', ref).split()
    for h in log:
        rows_ns = [r.split('	') for r in git('show', '--name-status', '-M', '--format=', h).splitlines()]
        pairs = [(p[1], p[2]) for p in rows_ns if p[0].startswith('R') and len(p) > 2]
        added = [p[1] for p in rows_ns if p[0] == 'A']
        for p in rows_ns:
            if p[0] == 'D':
                stem_d = re.sub(r'\.(rs|dart|py)$', '', p[1])
                hit = [a for a in added if a.startswith(stem_d + '/')]
                if hit:
                    pairs.append((p[1], hit[0]))
        for old, new in pairs:
            stem = re.sub(r'\.(rs|dart|py)$', '', old)
            if not new.startswith(stem + '/'):
                continue
            before_raw = show(h + '^', old) or []
            before = collections.Counter(n for n in map(normalise, before_raw) if keep(n))
            files = git('ls-tree', '-r', '--name-only', ref, stem + '/').split()
            after_raw = []
            for f in files:
                after_raw += show(ref, f) or []
            after = collections.Counter(n for n in map(normalise, after_raw) if keep(n))
            missing = before - after
            gone, renamed, deduped = classify(missing, after)
            lost = arms(before_raw) - arms(after_raw)
            rows.append((h[:9], old, len(files), sum(gone.values()), lost, gone,
                         sum(renamed.values()), sum(deduped.values())))
    return rows

chain_rows = chain_audit()
report.append(f'# Chain-level: every renamed monolith vs its directory at `{ref}`')
report.append('')
report.append('| Rename commit | Original file | Files now | Lines not found | Arms not found |')
report.append('| --- | --- | --- | --- | --- |')
for h, old, n, m, lost, missing, nr, nd in chain_rows:
    report.append(f'| {h} | `{old}` | {n} | {m} | {sum(lost.values())} |')
report.append('')
for h, old, n, m, lost, missing, nr, nd in chain_rows:
    if not m and not lost:
        continue
    report.append(f'## chain {h} `{old}`')
    report.append('')
    if lost:
        report.append('**Match arms not found:** ' + ', '.join(f'`{k}`×{v}' for k, v in sorted(lost.items())))
        report.append('')
    if nr or nd:
        report.append(f'Accounted for: {nr} renamed, {nd} deduplicated.')
        report.append('')
    report.append('```')
    for line, cnt in list(missing.items())[:40]:
        report.append((f'{cnt}x ' if cnt > 1 else '') + line[:160])
    if len(missing) > 40:
        report.append(f'... {len(missing) - 40} more')
    report.append('```')
    report.append('')

report.insert(5, '| Commit | Date | Subject | Lines lost | Arms lost | Renamed | Dedup |')
report.insert(6, '| --- | --- | --- | --- | --- | --- | --- |')
for i, (h, d, s, n, a, nr, nd) in enumerate(summary):
    report.insert(7 + i, f'| {h} | {d} | {s[:70]} | {n} | {a} | {nr} | {nd} |')
report.insert(7 + len(summary), '')
open(out, 'w', encoding='utf-8').write('\n'.join(report) + '\n')
print(f'{len(summary)} commits audited; {sum(1 for x in summary if x[3] or x[4])} with findings -> {out}')
