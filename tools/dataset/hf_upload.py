#!/usr/bin/env python3
"""hf_upload.py --root <release> --repo <org/name> [--private] [--yes]: publish a release.

Default is a dry run: it validates the release, lists what would be uploaded and the total
size, and uploads nothing. Only --yes uploads (huggingface_hub upload_folder, or
upload_large_folder with --large). The write token is read from HF_TOKEN in the repo's
.env (or --env FILE, else the HF_TOKEN environment variable) and is never printed, logged
or written anywhere; the upload is refused if any release file contains it.

--sync makes the Hub tree match the release exactly: in the same commit, every Hub file the
release does not hold is deleted (the Hub's own .gitattributes is kept).
"""

from __future__ import annotations

import argparse
import os
import re
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent


def default_env() -> Path:
    """The repo's .env; from a worktree under .claude/worktrees/, the main checkout's."""
    own = HERE.parents[1] / ".env"
    if own.is_file():
        return own
    for p in HERE.parents:
        if p.name == "worktrees" and p.parent.name == ".claude":
            return p.parent.parent / ".env"
    return own


DEFAULT_ENV = default_env()
IGNORE = {".DS_Store"}
IGNORE_DIRS = {".cache", ".git", "__pycache__"}
REFUSE_NAMES = re.compile(r"(^\.env$|\.pem$|\.key$|token)", re.I)
REPO_RE = re.compile(r"^[A-Za-z0-9][A-Za-z0-9._-]*/[A-Za-z0-9][A-Za-z0-9._-]*$")


def read_token(env_file: Path | None) -> tuple[str | None, str]:
    """(token, where it came from). The token itself is never shown."""
    if env_file and env_file.is_file():
        for line in env_file.read_text().splitlines():
            m = re.match(r"^\s*(?:export\s+)?HF_TOKEN\s*=\s*(.*)$", line)
            if m:
                tok = m.group(1).strip().strip('"').strip("'")
                if tok:
                    return tok, f"HF_TOKEN in {env_file} (not shown)"
    tok = os.environ.get("HF_TOKEN")
    if tok:
        return tok, "HF_TOKEN environment variable (not shown)"
    return None, "no HF_TOKEN found"


def release_files(root: Path) -> list[Path]:
    out = []
    for p in sorted(root.rglob("*")):
        rel = p.relative_to(root)
        if p.is_file() and p.name not in IGNORE and not (set(rel.parts) & IGNORE_DIRS):
            out.append(p)
    return out


def contains(path: Path, needle: bytes) -> bool:
    keep = len(needle) - 1
    tail = b""
    with open(path, "rb") as f:
        for chunk in iter(lambda: f.read(1 << 20), b""):
            if needle in tail + chunk:
                return True
            tail = (tail + chunk)[-keep:] if keep else b""
    return False


def preflight(root: Path, files: list[Path], token: str | None, validate: bool) -> list[str]:
    """Reasons to refuse the upload (empty when it may go ahead)."""
    problems = []
    for p in files:
        rel = p.relative_to(root).as_posix()
        if REFUSE_NAMES.search(p.name):
            problems.append(f"refusing a secret-looking file: {rel}")
        elif token and contains(p, token.encode()):
            problems.append(f"{rel} contains the HF token")
    for need in ("README.md", "manifest.json", "runs.parquet"):
        if not (root / need).is_file():
            problems.append(f"{need} missing: run assemble_release.py first")
    if validate and not problems:
        sys.path.insert(0, str(HERE))
        import validate_release
        for r in validate_release.run_checks(root):
            print(f"  {r.line()}")
            if r.ok is False:
                problems.append(f"validate_release: {r.name} failed")
    return problems


def upload(root: Path, repo: str, token: str, private: bool, large: bool, message: str,
           sync: bool = False):
    from huggingface_hub import HfApi
    api = HfApi(token=token)
    api.create_repo(repo_id=repo, repo_type="dataset", private=private, exist_ok=True)
    ignore = [".DS_Store", "**/.DS_Store", ".cache/**", ".git/**", "**/__pycache__/**"]
    if large:
        api.upload_large_folder(repo_id=repo, folder_path=str(root), repo_type="dataset",
                                private=private, ignore_patterns=ignore)
    else:
        api.upload_folder(repo_id=repo, folder_path=str(root), repo_type="dataset",
                          commit_message=message, ignore_patterns=ignore,
                          delete_patterns=["*"] if sync else None)


def human(n: float) -> str:
    for unit in ("B", "KB", "MB", "GB"):
        if n < 1000 or unit == "GB":
            return f"{n:.1f} {unit}" if unit != "B" else f"{int(n)} B"
        n /= 1000
    return f"{n:.1f} GB"


def main(argv=None) -> int:
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    ap.add_argument("--root", required=True, type=Path, help="assembled release root")
    ap.add_argument("--repo", required=True, help="Hugging Face dataset repo, org/name")
    ap.add_argument("--private", action="store_true", help="create the repo private")
    ap.add_argument("--yes", action="store_true", help="really upload (default: dry run)")
    ap.add_argument("--large", action="store_true", help="use upload_large_folder")
    ap.add_argument("--env", type=Path, default=DEFAULT_ENV, help="file holding HF_TOKEN")
    ap.add_argument("--no-validate", action="store_true", help="skip validate_release.py")
    ap.add_argument("--message", default="Upload V2X-SCMS-MBD release")
    ap.add_argument("--sync", action="store_true",
                    help="delete the Hub files the release does not hold, in the same commit")
    a = ap.parse_args(argv)
    if a.sync and a.large:
        print("--sync needs upload_folder; it cannot be used with --large", file=sys.stderr)
        return 2
    if not REPO_RE.match(a.repo):
        print(f"--repo must be org/name, got {a.repo!r}", file=sys.stderr)
        return 2
    root = a.root
    if not root.is_dir():
        print(f"not a directory: {root}", file=sys.stderr)
        return 2
    token, where = read_token(a.env)
    files = release_files(root)
    total = sum(p.stat().st_size for p in files)
    by_top: dict[str, list[int]] = {}
    for p in files:
        top = p.relative_to(root).parts[0]
        top = top + "/" if (root / top).is_dir() else top
        n = by_top.setdefault(top, [0, 0])
        n[0] += 1
        n[1] += p.stat().st_size
    mode = "UPLOAD" if a.yes else "DRY RUN"
    print(f"{mode}: {len(files)} files, {human(total)} -> https://huggingface.co/datasets/{a.repo}"
          f" ({'private' if a.private else 'public'}; {'upload_large_folder' if a.large else 'upload_folder'}"
          f"{'; Hub files not in the release are deleted' if a.sync else ''})")
    for top, (n, b) in sorted(by_top.items()):
        print(f"  {top:<16} {n:>6} files  {human(b):>10}")
    print(f"token: {where}")
    problems = preflight(root, files, token, not a.no_validate)
    if problems:
        for pr in problems:
            print(f"refused: {pr}")
        return 1
    if not a.yes:
        print("dry run: nothing uploaded; pass --yes to upload")
        return 0
    if not token:
        print("refused: no HF_TOKEN", file=sys.stderr)
        return 1
    try:
        upload(root, a.repo, token, a.private, a.large, a.message, a.sync)
    except Exception as e:  # never echo request details: they may carry the token
        print(f"upload failed: {type(e).__name__}", file=sys.stderr)
        return 1
    print(f"uploaded {len(files)} files to https://huggingface.co/datasets/{a.repo}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
