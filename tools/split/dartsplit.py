"""Mechanical splitter for oversized Dart files.

usage:
  dartsplit.py scan <file>                 -> list top-level declarations
  dartsplit.py split <file> <plan.json>    -> write files per plan
  dartsplit.py prune <analyze-report.txt>  -> delete unused/unnecessary imports

plan.json: {"dir": "flutter/lib/common/widgets/dialog",
            "entry": "dialog.dart",
            "files": {"validation.dart": ["ValidationRule", ...], ...},
            "rename": {"_foo": "foo"}}      # optional
Declarations not listed go to the file named in "default".
"""
import io, json, os, re, sys

def strip_code(src):
    """Return src with comments/strings blanked (same length) for brace scanning."""
    out = []
    i, n = 0, len(src)
    while i < n:
        c = src[i]
        if src.startswith('//', i):
            j = src.find('\n', i)
            j = n if j < 0 else j
            out.append(' ' * (j - i)); i = j; continue
        if src.startswith('/*', i):
            j = src.find('*/', i + 2)
            j = n if j < 0 else j + 2
            seg = src[i:j]
            out.append(re.sub(r'[^\r\n]', ' ', seg)); i = j; continue
        if c in '"\'':
            raw = i > 0 and src[i-1] == 'r'
            if src.startswith(c * 3, i):
                q = c * 3
                j = i + 3
                while j < n and not src.startswith(q, j):
                    if src[j] == '\\' and not raw: j += 2
                    else: j += 1
                j = min(n, j + 3)
            else:
                j = i + 1
                while j < n:
                    ch = src[j]
                    if ch == '\\' and not raw: j += 2; continue
                    if ch == '$' and not raw and src.startswith('${', j):
                        k = j + 2; d = 1
                        while k < n and d:
                            if src[k] == '{': d += 1
                            elif src[k] == '}': d -= 1
                            elif src[k] in '"\'':
                                qq = src[k]; k += 1
                                while k < n and src[k] != qq:
                                    if src[k] == '\\': k += 1
                                    k += 1
                            k += 1
                        j = k; continue
                    if ch == c: j += 1; break
                    if ch == '\n': break
                    j += 1
            seg = src[i:j]
            out.append(re.sub(r'[^\r\n]', ' ', seg)); i = j; continue
        out.append(c); i += 1
    return ''.join(out)

DECL_RE = re.compile(
    r'^(?:@\w+(?:\([^)]*\))?\s*)*'
    r'(?:(?:abstract|final|base|sealed|mixin)\s+)*'
    r'(?:class|enum|extension|mixin|typedef)\s+(\w+)'
    r'|^(?:(?:const|final|late|external|static)\s+)*'
    r'(?:[\w<>,\s\?\.\[\]]+?\s+)?'
    r'(\w+)\s*(?:<[^{=;(]*>)?\s*[(=]'
    r'|^(?:(?:const|final|late)\s+)(?:[\w<>,\?\.\[\]]+\s+)?(\w+)\s*;'
    r'|^[\w<>,\?\.\[\]]+\s+(?:get|set)\s+(\w+)\b'
    r'|^[\w<>,\?\.\[\]]+\s+(\w+)\s*;'
)

def scan(path):
    src = io.open(path, encoding='utf-8', newline='').read()
    nl = '\r\n' if '\r\n' in src else '\n'
    src = src.replace('\r\n', '\n')
    lines = src.split('\n')
    code = strip_code(src).split('\n')
    assert len(lines) == len(code)
    header_end = 0
    depth = 0
    decls = []
    pending_start = None
    i = 0
    n = len(lines)
    while i < n:
        raw = lines[i]; cl = code[i]
        if depth == 0 and pending_start is None:
            s = raw.strip()
            if s == '' or s.startswith('//') or s.startswith('/*') or s.startswith('*') or s.startswith('*/'):
                i += 1; continue
            if re.match(r'(import|export|part|library)\b', s):
                while ';' not in code[i]:
                    i += 1
                header_end = i + 1
                i += 1; continue
            pending_start = i
        depth += cl.count('{') + cl.count('(') + cl.count('[') - cl.count('}') - cl.count(')') - cl.count(']')
        if pending_start is not None and depth == 0:
            stripped = cl.rstrip()
            if stripped.endswith('}') or stripped.endswith(';'):
                decls.append([pending_start, i])
                pending_start = None
        i += 1
    assert pending_start is None, ('unterminated decl at', pending_start + 1)
    out = []
    prev_end = header_end
    for (s, e) in decls:
        k = s
        while k - 1 >= prev_end:
            t = lines[k-1].strip()
            if t.startswith('///') or t.startswith('@') or t.startswith('//'):
                k -= 1
            else:
                break
        name = None
        m = DECL_RE.match(code[s])
        if m:
            name = next(g for g in m.groups() if g)
        else:
            first = code[s].split('=>')[0]
            ms = re.findall(r'(\w+)\s*(?:<[^(]*>)?\s*\(', first)
            name = ms[-1] if ms else f'anon@{s+1}'
        out.append({'name': name, 'start': k, 'end': e, 'lines': e - k + 1})
        prev_end = e + 1
    seen = {}
    for d in out:
        assert d['name'] not in seen, ('duplicate name', d['name'], seen[d['name']], d['start']+1)
        seen[d['name']] = d['start'] + 1
    return src, nl, lines, header_end, out

def refs(text, names):
    found = set()
    for nm in names:
        if re.search(r'(?<![\w$])' + re.escape(nm) + r'(?![\w$])', text):
            found.add(nm)
    return found

def cmd_scan(path):
    src, nl, lines, header_end, decls = scan(path)
    names = [d['name'] for d in decls]
    priv = [nm for nm in names if nm.startswith('_')]
    for d in decls:
        r = refs('\n'.join(lines[d['start']+1:d['end']+1]), [p for p in priv if p != d['name']])
        print(f"{d['start']+1:5d}-{d['end']+1:<5d} {d['lines']:4d}  {d['name']:<45s} {' '.join(sorted(r))}")
    print(f"header ends at line {header_end}; {len(decls)} decls; total {len(lines)} lines")

def cmd_split(path, planfile):
    plan = json.load(io.open(planfile, encoding='utf-8'))
    src, nl, lines, header_end, decls = scan(path)
    header = '\n'.join(lines[:header_end])
    # new files live one directory deeper than the original
    header = re.sub(r"(import\s+')(?!package:|dart:)", lambda m: m.group(1) + '../', header)
    names = {d['name']: d for d in decls}
    assign = {}
    for f, lst in plan['files'].items():
        for nm in lst:
            assert nm in names, f'unknown decl {nm}'
            assert nm not in assign, f'dup {nm}'
            assign[nm] = f
    default = plan.get('default')
    for nm in names:
        if nm not in assign:
            assert default, f'unassigned decl {nm}'
            assign[nm] = default
    rename = plan.get('rename', {})
    def apply_rename(text):
        for a, b in rename.items():
            text = re.sub(r'(?<![\w$])' + re.escape(a) + r'(?![\w$])', b, text)
        return text
    d = plan['dir']
    os.makedirs(d, exist_ok=True)
    files = {}
    for dd in decls:
        f = assign[dd['name']]
        files.setdefault(f, []).append(dd)
    # class splits: cut absolute line ranges out of a declaration and emit them
    # as an extension in another file of the same (part-based) library
    extras = {}   # file -> list of text blocks
    cut_lines = set()
    for cs in plan.get('classsplit', []):
        for c in cs['cuts']:
            a, b = c['lines']
            block = '\n'.join(lines[a-1:b])
            extras.setdefault(c['file'], []).append(
                f"extension {c['ext']} on {cs['class']} {{\n{block}\n}}")
            cut_lines.update(range(a-1, b))
    def decl_text(x):
        ls = [lines[i] for i in range(x['start'], x['end']+1) if i not in cut_lines]
        # collapse runs of blank lines left by the cuts
        out = []
        for l in ls:
            if l.strip() == '' and out and out[-1].strip() == '':
                continue
            out.append(l)
        return '\n'.join(out)
    use_parts = plan.get('parts', False)
    written = []
    entry = plan['entry']
    for f in extras:
        files.setdefault(f, [])
    for f, ds in files.items():
        body = '\n\n'.join([decl_text(x) for x in ds] + extras.get(f, []))
        if use_parts:
            if f == entry:
                parts = '\n'.join(f"part '{g}';" for g in sorted(files) if g != f)
                content = header + '\n\n' + parts + '\n\n' + body + '\n'
            else:
                content = f"part of '{entry}';\n\n" + body + '\n'
        else:
            imps = '\n'.join(f"import '{g}';" for g in sorted(files) if g != f)
            content = header + '\n' + imps + '\n\n' + body + '\n'
        content = apply_rename(content).replace('\n', nl)
        p = os.path.join(d, f)
        io.open(p, 'w', encoding='utf-8', newline='').write(content)
        written.append((p, content.count(nl) + 1))
    if use_parts:
        io.open(path, 'w', encoding='utf-8', newline='').write(
            f"export '{os.path.basename(d)}/{entry}';\n".replace('\n', nl))
        for p, c in written:
            print(f'{c:5d} {p}')
        print(f'barrel -> {path}')
        return
    exports = '\n'.join(f"export '{os.path.basename(d)}/{f}';" for f in sorted(files))
    io.open(path, 'w', encoding='utf-8', newline='').write((exports + '\n').replace('\n', nl))
    for p, c in written:
        print(f'{c:5d} {p}')
    print(f'barrel -> {path}')

def cmd_prune(report, prefix):
    pat = re.compile(r"(?:Unused import|The import of '[^']*' is unnecessary)[^\n]*? - (lib[^:]+):(\d+):\d+ - (unused_import|unnecessary_import)")
    text = io.open(report, encoding='utf-8', errors='replace').read()
    hits = {}
    for m in pat.finditer(text):
        f = m.group(1).replace('\\', '/')
        if not f.startswith(prefix):
            continue
        hits.setdefault(f, set()).add(int(m.group(2)))
    for f, ls in hits.items():
        p = os.path.join('flutter', f)
        src = io.open(p, encoding='utf-8', newline='').read()
        nl = '\r\n' if '\r\n' in src else '\n'
        lines = src.split(nl)
        for l in sorted(ls, reverse=True):
            assert lines[l-1].lstrip().startswith('import'), (p, l, lines[l-1])
            end = l - 1
            while ';' not in lines[end]:
                end += 1
            del lines[l-1:end+1]
        io.open(p, 'w', encoding='utf-8', newline='').write(nl.join(lines))
        print(f'pruned {len(ls)} imports from {p}')

def cmd_members(path, cls):
    """List members of a class declaration with absolute 1-based line ranges."""
    src, nl, lines, header_end, decls = scan(path)
    d = next(x for x in decls if x['name'] == cls)
    code = strip_code('\n'.join(lines)).split('\n')
    # find the opening brace line of the class
    i = d['start']
    while '{' not in code[i]:
        i += 1
    body_start = i + 1
    depth = 0
    pending = None
    members = []
    j = body_start
    while j <= d['end']:
        cl = code[j]
        if depth == 0 and pending is None:
            s = lines[j].strip()
            if s == '' or s.startswith('//') or s.startswith('/*') or s.startswith('*'):
                j += 1; continue
            if s == '}' and j == d['end']:
                break
            pending = j
        depth += cl.count('{') + cl.count('(') + cl.count('[') - cl.count('}') - cl.count(')') - cl.count(']')
        if pending is not None and depth == 0:
            st = cl.rstrip()
            if st.endswith('}') or st.endswith(';'):
                members.append((pending, j))
                pending = None
        j += 1
    prev = body_start
    for (s, e) in members:
        k = s
        while k - 1 >= prev and (lines[k-1].strip().startswith('//') or lines[k-1].strip().startswith('@')):
            k -= 1
        first = lines[s].strip()
        kind = 'field' if (re.match(r'(static\s+)?(final|const|late|var|[\w<>,\?\.\[\]]+)\s+[\w]+\s*(=|;)', first) and '(' not in first.split('=')[0]) else 'method'
        if first.startswith('static'): kind = 'static-' + kind
        ov = 'override' if any(lines[t].strip().startswith('@override') for t in range(k, s+1)) else ''
        m = re.search(r'(\w+)\s*(?:<[^(]*>)?\s*\(|(?:get|set)\s+(\w+)|(\w+)\s*[=;]', first)
        nm = next((g for g in (m.groups() if m else ()) if g), first[:30])
        print(f"{k+1:5d}-{e+1:<5d} {e-k+1:4d}  {kind:<13s} {ov:<8s} {nm}")
        prev = e + 1

def cmd_move(argv):
    """move <entry> <target> [--lib] [--cut CLASS:EXT:START-END]... NAME...

    Incrementally move top-level declarations (and/or line ranges of a class,
    emitted as `extension EXT on CLASS`) out of the library root <entry> into
    <target> in the same directory. Default: <target> becomes a `part of`
    <entry>; with --lib it becomes an independent library that <entry>
    imports and exports."""
    entry, target = argv[0], argv[1]
    rest = argv[2:]
    use_lib = False
    append = False
    cuts = []
    names = []
    i = 0
    while i < len(rest):
        a = rest[i]
        if a == '--lib':
            use_lib = True
        elif a == '--append':
            append = True
        elif a == '--cut':
            cls, ext, rng = rest[i+1].split(':')
            a1, b1 = rng.split('-')
            cuts.append((cls, ext, int(a1), int(b1)))
            i += 1
        else:
            names.append(a)
        i += 1
    src, nl, lines, header_end, decls = scan(entry)
    byname = {d['name']: d for d in decls}
    for nm in names:
        assert nm in byname, f'unknown decl {nm}'
    remove = set()
    blocks = []
    for nm in names:
        d = byname[nm]
        remove.update(range(d['start'], d['end'] + 1))
        blocks.append('\n'.join(lines[d['start']:d['end']+1]))
    for cls, ext, a1, b1 in cuts:
        block = '\n'.join(lines[a1-1:b1])
        blocks.append(f"extension {ext} on {cls} {{\n{block}\n}}")
        remove.update(range(a1-1, b1))
    # rebuild entry without the removed lines, collapsing blank runs
    kept = []
    after_removed = False
    for idx, l in enumerate(lines):
        if idx in remove:
            after_removed = True
            continue
        if after_removed and l.strip() == '' and kept and kept[-1].strip() == '':
            continue
        after_removed = False
        kept.append(l)
    # insert directive after the last import/export/part line
    last_dir = -1
    for idx, l in enumerate(kept):
        if re.match(r'(import|export|part)\b', l):
            last_dir = idx
            while ';' not in kept[last_dir]:
                last_dir += 1
    # paths relative to the entry's directory (parts may live in a subdir)
    tname = os.path.relpath(target, os.path.dirname(entry) or '.').replace(os.sep, '/')
    ename = os.path.relpath(entry, os.path.dirname(target) or '.').replace(os.sep, '/')
    if use_lib:
        directives = [f"import '{tname}';", f"export '{tname}';"]
    else:
        directives = [f"part '{tname}';"]
    if not append:
        for k, dline in enumerate(directives):
            kept.insert(last_dir + 1 + k, dline)
    io.open(entry, 'w', encoding='utf-8', newline='').write((('\n'.join(kept)).rstrip('\n') + '\n').replace('\n', nl))
    body = '\n\n'.join(blocks)
    if use_lib:
        header = '\n'.join(lines[:header_end])
        header = '\n'.join(l for l in header.split('\n') if not re.match(r'(part|export)\b', l))
        content = header + '\n' + f"import '{ename}';" + '\n\n' + body + '\n'
    else:
        content = f"part of '{ename}';\n\n" + body + '\n'
    if append:
        prev = io.open(target, encoding='utf-8', newline='').read().replace('\r\n', '\n')
        content = prev.rstrip('\n') + '\n\n' + body + '\n'
    io.open(target, 'w', encoding='utf-8', newline='').write(content.replace('\n', nl))
    print(f'{content.count(chr(10))+1:5d} {target}')
    print(f"{len(kept):5d} {entry}")

def cmd_init(orig):
    """init <orig.dart>: git mv orig.dart -> orig/orig.dart (library root),
    fix its relative imports for the extra directory level, and leave a
    barrel `export 'orig/orig.dart';` at the old path."""
    import subprocess
    d = orig[:-5]
    base = os.path.basename(orig)
    os.makedirs(d, exist_ok=True)
    dest = os.path.join(d, base)
    subprocess.check_call(['git', 'mv', orig, dest])
    s = io.open(dest, encoding='utf-8', newline='').read()
    nl = '\r\n' if '\r\n' in s else '\n'
    s = re.sub(r"((?:import|export|part)\s+')(?!package:|dart:)", lambda m: m.group(1) + '../', s)
    io.open(dest, 'w', encoding='utf-8', newline='').write(s)
    io.open(orig, 'w', encoding='utf-8', newline='').write(f"export '{os.path.basename(d)}/{base}';{nl}")
    print(f'moved {orig} -> {dest}; barrel written')

if __name__ == '__main__':
    if sys.argv[1] == 'init':
        cmd_init(sys.argv[2])
    elif sys.argv[1] == 'move':
        cmd_move(sys.argv[2:])
    elif sys.argv[1] == 'members':
        cmd_members(sys.argv[2], sys.argv[3])
    elif sys.argv[1] == 'prune':
        cmd_prune(sys.argv[2], sys.argv[3])
    elif sys.argv[1] == 'scan':
        cmd_scan(sys.argv[2])
    else:
        cmd_split(sys.argv[2], sys.argv[3])
