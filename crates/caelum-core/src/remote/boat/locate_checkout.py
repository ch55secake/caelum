"""Run inside Boat's command workdir. Only the physical checkout path goes to stdout."""
import os
from pathlib import Path
import subprocess
import sys
from tempfile import TemporaryDirectory
from urllib.parse import urlsplit


def git(directory, *args):
    return subprocess.check_output(["git", "-C", directory, *args], stderr=subprocess.DEVNULL,
                                   encoding="utf-8").removesuffix("\n")


def github_origin(directory):
    if not (directory / ".git").exists():
        return None
    try:
        if git(directory, "rev-parse", "--show-toplevel") != str(directory):
            return None
        url = git(directory, "remote", "get-url", "origin")
        if url.startswith("git@github.com:"):
            slug = url.split(":", 1)[1]
        else:
            remote = urlsplit(url)
            if remote.scheme not in ("https", "ssh") or remote.hostname != "github.com":
                return None
            slug = remote.path.lstrip("/")
        return slug.rstrip("/").removesuffix(".git").lower() if slug.isascii() else None
    except (subprocess.CalledProcessError, ValueError):
        return None


def locate(repo, pinned, new_base):
    if pinned:
        path = Path(pinned)
        if not path.is_dir():
            sys.exit(40)
        if path.resolve() != path or github_origin(path) != repo.lower():
            sys.exit(41)
        return path

    name = repo.split("/")[1]
    home = Path.home().resolve()
    workdir = Path.cwd()
    if workdir == workdir.parent:
        workdir = home
    if workdir == workdir.parent:
        sys.exit(40)
    destination = workdir / name
    candidates = set()
    try:
        containing = Path(git(workdir, "rev-parse", "--show-toplevel"))
        candidates.add(containing)
    except subprocess.CalledProcessError:
        containing = None
    for base in {workdir, home}:
        if base == base.parent:
            continue
        candidates.add(base)
        inside_repo = (base / ".git").exists() or (containing and base.is_relative_to(containing))
        # The named slot must not point at some other repository, even if a matching
        # clone is found elsewhere. A link to the right checkout resolves to its real path.
        named = base / name
        if os.path.lexists(named) and (not inside_repo or (named / ".git").exists()):
            actual = named.resolve()
            if github_origin(actual) != repo.lower():
                sys.exit(41)
            candidates.add(actual)
        if inside_repo:
            continue
        for child in base.iterdir():
            if child.name.startswith(".") or child.is_symlink() or not child.is_dir():
                continue
            candidates.add(child)
            nested = child / name
            if not (child / ".git").exists() and not nested.is_symlink():
                candidates.add(nested)
    matches = [p for p in candidates if github_origin(p) == repo.lower()]
    if len(matches) > 1:
        sys.exit(42)
    if matches:
        return matches[0]

    if containing or os.path.lexists(destination):
        sys.exit(41)
    if not new_base:
        sys.exit(40)
    # Publish only a complete clone, without replacing even an empty preexisting directory.
    with TemporaryDirectory(prefix=".caelum-clone-", dir=workdir) as temporary:
        staged = Path(temporary) / name
        clone = subprocess.run(["git", "clone", "--branch", new_base, "--",
                                f"https://github.com/{repo}.git", staged],
                               env={**os.environ, "GIT_TERMINAL_PROMPT": "0"},
                               stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        if clone.returncode:
            sys.exit(43)
        if github_origin(staged) != repo.lower():
            sys.exit(41)
        try:
            destination.mkdir()
        except FileExistsError:
            if destination.is_symlink() or github_origin(destination) != repo.lower():
                sys.exit(41)
        else:
            staged.rename(destination)
    return destination


print(locate(*sys.argv[1:]))
