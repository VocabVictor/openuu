"""Find control characters that a shell heredoc left in tracked text files.

A Windows path carried through a heredoc loses its escape: the two characters
that spell a backslash followed by b become one byte, 0x08. Nothing shows it.
Markdown renders it as nothing, git diff prints it as nothing, and a reviewer
reading the rendered page sees a path with a letter missing and reads past it.
Only a byte-level look finds it, which is what this script is for.

    python tools/scan_control_chars.py            # the whole repository
    python tools/scan_control_chars.py docs        # one directory
    python tools/scan_control_chars.py new-check.ps1   # one file, tracked or not

A named file is read directly, so a build-machine script can be checked before
it is copied over. A directory that matches no tracked file is an error, not a
pass: a scan that silently reports "clean" for input it never opened is worse
than no scan.

Exit code 1 when something is found, so it can gate a commit or a script
install. Three control characters are legitimate in text and are allowed: tab,
line feed and carriage return.
"""

import os
import subprocess
import sys

TAB, LF, CR, DEL = 9, 10, 13, 127

# Byte-oriented formats only. A file whose bytes are not text (an image, a
# font, a compiled artefact) has every right to a 0x08 in it.
SUFFIXES = (
    ".md", ".rs", ".dart", ".ps1", ".sh", ".bat", ".cmd", ".py",
    ".yml", ".yaml", ".toml", ".json", ".txt", ".cc", ".cpp", ".h",
    ".gradle", ".kt", ".java", ".xml", ".cfg", ".ini",
)

NAMES = {"control character", "delete"}


def tracked_files(root, prefix):
    """Ask git, so the submodule, build outputs and ignored trees stay out."""
    cmd = ["git", "-C", root, "ls-files", "-z"]
    if prefix:
        cmd.append(prefix)
    out = subprocess.run(cmd, capture_output=True, check=True).stdout
    for name in out.split(b"\0"):
        if not name:
            continue
        path = name.decode("utf-8", "replace")
        if path.endswith(SUFFIXES):
            yield path


def paths_to_read(root, prefix):
    """A named file wins over the git listing; anything else must match."""
    if prefix and os.path.isfile(prefix):
        return [prefix]
    found = list(tracked_files(root, prefix))
    if prefix and not found:
        raise SystemExit(
            "nothing to scan: '%s' is neither a file nor a tracked path with a "
            "text suffix. Refusing to report a clean scan of nothing." % prefix)
    return found


def offending_bytes(data):
    """Every byte that has no business in a text file, with its offset."""
    for offset, byte in enumerate(data):
        if byte == DEL or (byte < 32 and byte not in (TAB, LF, CR)):
            yield offset, byte


def render(line, bad_column):
    """The line as a reader can see it: the byte spelled out where it sits.

    The text around it is decoded, or a line of Chinese would come back as
    mojibake and the reader could not tell which sentence to go and look at.
    """
    if line.endswith(b"\r"):
        line = line[:-1]
    shown = b""
    for column, byte in enumerate(line):
        one = line[column:column + 1]
        if byte == TAB and column != bad_column:
            shown += b"    "
        elif column == bad_column or byte < 32 or byte == DEL:
            shown += ("<%02X>" % byte).encode()
        else:
            shown += one
    return shown.decode("utf-8", "replace")


def locate(data, offset):
    """Line number (from 1) and the byte offset within that line."""
    start = data.rfind(b"\n", 0, offset) + 1
    end = data.find(b"\n", offset)
    if end < 0:
        end = len(data)
    line_number = data.count(b"\n", 0, start) + 1
    return line_number, data[start:end], offset - start


def scan(root, prefix):
    findings = []
    for path in paths_to_read(root, prefix):
        full = root + "/" + path if root != "." else path
        try:
            with open(full, "rb") as handle:
                data = handle.read()
        except OSError as error:
            print("could not read %s: %s" % (path, error), file=sys.stderr)
            continue
        if b"\0" in data:      # not text after all, whatever the suffix says
            continue
        for offset, byte in offending_bytes(data):
            line_number, line, column = locate(data, offset)
            findings.append((path, line_number, byte, render(line, column)))
    return findings


def main(argv):
    root = "."
    prefix = argv[1] if len(argv) > 1 else ""
    scanned = paths_to_read(root, prefix)
    findings = scan(root, prefix)
    if not findings:
        print("no control characters in %d file(s)" % len(scanned))
        return 0
    for path, line_number, byte, line in findings:
        print("%s:%d: byte 0x%02X" % (path, line_number, byte))
        print("    %s" % line)
    print()
    print("%d control character(s) found." % len(findings))
    print("0x08 is a backslash-b that a shell heredoc ate; write the file with")
    print("a tool that does not interpret escapes, or build the path from")
    print("chr(92), and run this again.")
    return 1


if __name__ == "__main__":
    sys.exit(main(sys.argv))
