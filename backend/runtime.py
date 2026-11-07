"""Keep current_exe resolvable across Cargo's atomic executable replacement (Linux)."""
import os
from pathlib import Path
import re
import shutil
import stat
import uuid

_NAME = re.compile(r"pin-(\d+)-(\d+)-(\d+)-[0-9a-f]{32}\Z")


def _start(pid):
    # comm may contain spaces or parentheses; field 22 follows its final ')'.
    value = Path(f"/proc/{pid}/stat").read_text().rsplit(")", 1)[1].split()[19]
    if not value.isdecimal() or int(value) <= 0:
        raise ValueError("Invalid process start time")
    return value


def _private(path):
    info = path.lstat()
    if not stat.S_ISDIR(info.st_mode) or info.st_uid != os.getuid() or info.st_mode & 0o077:
        raise ValueError(f"Runtime directory must be real, owned and private: {path}")


def _collect(root, namespace):
    for entry in root.iterdir():
        match = _NAME.fullmatch(entry.name)
        if not match or match[3] != str(namespace):
            continue
        try:
            _private(entry)
            try:
                stale = _start(int(match[1])) != match[2]
            except FileNotFoundError:
                try:
                    Path(f"/proc/{match[1]}").stat()
                    stale = False
                except FileNotFoundError:
                    stale = True
            if not stale:
                continue
            # Never recursively delete unknown contents, symlinks or other owners.
            contents = list(entry.iterdir())
            if len(contents) != 1 or contents[0].name != "rapidmaker-api":
                continue
            info = contents[0].lstat()
            if not stat.S_ISREG(info.st_mode) or info.st_uid != os.getuid():
                continue
            contents[0].unlink()
            entry.rmdir()
        except (OSError, ValueError, IndexError):
            continue  # Uncertain ownership/process visibility always preserves the pin.


def pin_executable(source: Path, state_root: Path | None = None) -> Path:
    # Open first: a rebuild may replace the source pathname at any later point.
    with source.open("rb") as held:
        root = state_root if state_root is not None else Path(os.environ.get("XDG_STATE_HOME", Path.home() / ".local/state")) / "rapidmaker/runtime"
        root.mkdir(mode=0o700, parents=True, exist_ok=True)
        _private(root)
        namespace = Path("/proc/self/ns/pid").stat().st_ino
        _collect(root, namespace)
        launch = root / f"pin-{os.getpid()}-{_start(os.getpid())}-{namespace}-{uuid.uuid4().hex}"
        launch.mkdir(mode=0o700)
        target = launch / "rapidmaker-api"
        try:
            try:
                os.link(source, target)
                linked, original = target.lstat(), os.fstat(held.fileno())
                if (linked.st_dev, linked.st_ino) == (original.st_dev, original.st_ino):
                    return target  # Do not chmod the shared Cargo inode.
                target.unlink()
            except OSError:
                target.unlink(missing_ok=True)
            with target.open("xb") as output:
                shutil.copyfileobj(held, output)
                os.fchmod(output.fileno(), 0o500)
            return target
        except BaseException:
            target.unlink(missing_ok=True)
            launch.rmdir()
            raise
