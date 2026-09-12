import io, re, sys, os
# usage: fix_imports.py <analyze.txt> <TypeRegex> <package-import-path>
os.chdir(os.path.join(os.path.dirname(os.path.abspath(__file__)), '..', '..', 'flutter'))
rep = io.open(sys.argv[1], encoding='utf-8', errors='replace').read()
types, imp = sys.argv[2], sys.argv[3]
pat = re.compile(r"isn't defined for the type '(?:" + types + r")' - (lib[^:]+):")
files = sorted(set(m.group(1).replace('\\', '/') for m in pat.finditer(rep)))
for f in files:
    s = io.open(f, encoding='utf-8', newline='').read()
    m = re.search(r"^part of '([^']+)';", s, re.M)
    if m:
        f = os.path.normpath(os.path.join(os.path.dirname(f), m.group(1))).replace(os.sep, '/')
        s = io.open(f, encoding='utf-8', newline='').read()
    nl = '\r\n' if '\r\n' in s else '\n'
    if ("'" + imp + "'") in s:
        print('already', f); continue
    lines = s.split(nl)
    last = max(i for i, l in enumerate(lines) if l.startswith('import '))
    while ';' not in lines[last]:
        last += 1
    lines.insert(last + 1, "import '" + imp + "';")
    io.open(f, 'w', encoding='utf-8', newline='').write(nl.join(lines))
    print('added', f)
