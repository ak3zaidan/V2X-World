#!/usr/bin/env python3
"""Inject one fault, run a command, restore the source, and leave the build able to see it.

    tools/mutate/mutate.py <file> <old> <new> -- <command...>

`<old>` must occur exactly once in `<file>`; it is replaced by `<new>`, `<command>` runs
(normally `heavy.sh cargo test -p <crate> --test <t> <filter>`), and the file is put back.
Prints one verdict line: RED (the command failed, so the check can fail), GREEN (the
fault was not caught) or RESTORE FAILED, and exits 0 only for RED.

Why this exists (WP-DET, 2026-10-17). Cargo decides that a workspace crate is fresh by
comparing its source files' mtimes with the build's outputs. The obvious restore,
`shutil.copy(p, p + '.bak')` before the fault and `shutil.move(p + '.bak', p)` after, puts
back a file whose mtime is the moment of the copy: *older* than the build of the faulted
source. Cargo then keeps the faulted build, and every later run of the "restored" tree
tests the fault. That is how the D2 fast-forward equivalence test came to fail "in 2 of 5
runs with no code change": the runs after the "fast-forward stops one step late" fault
were still that fault's binary (the top-up left 100 ms late), until an unrelated edit to
the same file bumped its mtime. This tool restores the bytes and then sets the file's
mtime to now, so the next build recompiles the restored source.

Paths are taken relative to the current directory (run it from the worktree root); the
script never writes anywhere but `<file>`.
"""

import os
import subprocess
import sys
import time


def main() -> int:
    if "--" not in sys.argv or sys.argv.index("--") != 4:
        print(__doc__.split("\n\n")[1])
        return 2
    path, old, new = sys.argv[1:4]
    cmd = sys.argv[5:]
    src = open(path, "rb").read()
    n = src.count(old.encode())
    if n != 1:
        print(f"ANCHOR {n}x in {path}: nothing injected")
        return 2
    try:
        with open(path, "wb") as f:
            f.write(src.replace(old.encode(), new.encode()))
        r = subprocess.run(cmd, capture_output=True, text=True)
        out = (r.stdout or "") + (r.stderr or "")
        compiled = "error[" not in out and "error: could not compile" not in out
    finally:
        with open(path, "wb") as f:
            f.write(src)
        # Newer than anything the faulted build produced.
        now = time.time() + 1
        os.utime(path, (now, now))
    if open(path, "rb").read() != src:
        print("RESTORE FAILED")
        return 3
    if not compiled:
        print("NOCOMPILE (the fault does not build; nothing was tested)")
        return 4
    if r.returncode != 0:
        print("RED (the check failed under the fault); restored")
        lines = out.splitlines()
        for k, line in enumerate(lines):
            if "panicked at" in line:
                print("  " + " | ".join(x.strip() for x in lines[k : k + 3])[:400])
        return 0
    print("GREEN (the fault was not caught); restored")
    return 1


if __name__ == "__main__":
    sys.exit(main())
