"""Replace protected-member calls inside extension part files with private
forwarders declared in the class body (same library, so visible).

usage: forwarders.py <dir> <member> <forwarder> <class1:classfile1> [...]
  member    e.g. setState / notifyListeners
  forwarder e.g. _setState / _notify
Every part file in <dir> (except the class files) gets member( -> forwarder(
inside `extension X on Class {` blocks; the forwarder is inserted after the
class header of each listed class only when some extension of it uses it.
Also strips any '// ignore_for_file: invalid_use_of_...' lines.
"""
import io, os, re, sys

d, member, fwd = sys.argv[1], sys.argv[2], sys.argv[3]
classes = dict(a.split(':') for a in sys.argv[4:])
used = set()
for fn in sorted(os.listdir(d)):
    if not fn.endswith('.dart'):
        continue
    p = os.path.join(d, fn)
    s = io.open(p, encoding='utf-8', newline='').read()
    nl = '\r\n' if '\r\n' in s else '\n'
    orig = s
    s = re.sub(r'// ignore_for_file: invalid_use_of_[^\r\n]*' + nl, '', s)
    # rewrite inside extension bodies only
    out = []
    pos = 0
    for m in re.finditer(r'extension\s+\w+\s+on\s+(\w+)\s*\{', s):
        cls = m.group(1)
        start = m.end()
        depth = 1
        i = start
        while depth and i < len(s):
            if s[i] == '{': depth += 1
            elif s[i] == '}': depth -= 1
            i += 1
        body = s[start:i]
        if member == 'setState' and re.search(r'[(,]\s*setState\s*[,)]|StateSetter\s+setState', body):
            # a StatefulBuilder-style local parameter named setState shadows
            # the State method here; leave this extension for manual review
            print('SKIP (local setState parameter) in', fn, 'extension on', cls)
            out.append(s[pos:i]); pos = i
            continue
        new_body, n = re.subn(r'(?<![\w.$])' + re.escape(member) + r'(?![\w$])', fwd, body)
        if n:
            used.add(cls)
        out.append(s[pos:start]); out.append(new_body); pos = i
    out.append(s[pos:])
    s = ''.join(out)
    if s != orig:
        io.open(p, 'w', encoding='utf-8', newline='').write(s)
        print('rewrote', fn)
for cls in sorted(used):
    p = os.path.join(d, classes[cls])
    s = io.open(p, encoding='utf-8', newline='').read()
    nl = '\r\n' if '\r\n' in s else '\n'
    m = re.search(r'^class ' + re.escape(cls) + r'\b[^{]*\{[ \t]*' + nl, s, re.M)
    assert m, cls
    if fwd + '(' in s[m.end():]:
        print('forwarder already present in', cls); continue
    if member == 'setState':
        line = f'  void {fwd}(VoidCallback fn) => setState(fn);{nl}'
    else:
        line = f'  void {fwd}() => {member}();{nl}'
    s = s[:m.end()] + line + s[m.end():]
    io.open(p, 'w', encoding='utf-8', newline='').write(s)
    print('forwarder added to', cls, 'in', classes[cls])
