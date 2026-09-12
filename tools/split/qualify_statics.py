"""qualify_statics.py <root.dart> <Class> <part files...>

Inside `extension X on Class` bodies of the given part files, prefix every
unqualified reference to a static member of Class with `Class.` (extensions
cannot see the extended type's statics unqualified)."""
import io, os, re, subprocess, sys

root, cls, files = sys.argv[1], sys.argv[2], sys.argv[3:]
here = os.path.dirname(os.path.abspath(__file__))
out = subprocess.check_output([sys.executable, os.path.join(here, 'dartsplit.py'), 'members', root, cls], text=True)
statics = []
for l in out.splitlines():
    p = l.split()
    if len(p) >= 4 and p[2].startswith('static'):
        statics.append(p[-1])
if not statics:
    print('no statics'); sys.exit(0)
pat = re.compile(r'(?<![\w.$])(' + '|'.join(map(re.escape, statics)) + r')(?![\w$])(?!\s*:)')
for f in files:
    s = io.open(f, encoding='utf-8', newline='').read()
    out, pos, n = [], 0, 0
    for m in re.finditer(r'extension\s+\w+\s+on\s+' + re.escape(cls) + r'\s*\{', s):
        start = m.end(); depth = 1; i = start
        while depth and i < len(s):
            if s[i] == '{': depth += 1
            elif s[i] == '}': depth -= 1
            i += 1
        body, k = pat.subn(cls + r'.\1', s[start:i])
        n += k
        out.append(s[pos:start]); out.append(body); pos = i
    out.append(s[pos:])
    if n:
        io.open(f, 'w', encoding='utf-8', newline='').write(''.join(out))
    print(f, n)
