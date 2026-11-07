import errno
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import runtime


class RuntimeTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.source = self.root / "build"
        self.source.write_bytes(b"opened executable")
        self.source.chmod(0o751)
        self.state = self.root / "state"

    def pin(self):
        return runtime.pin_executable(self.source, self.state)

    def test_hardlink_preserves_inode_and_mode(self):
        target = self.pin()
        self.assertEqual(target.stat().st_ino, self.source.stat().st_ino)
        self.assertEqual(self.source.stat().st_mode & 0o777, 0o751)
        self.assertEqual(target.parent.stat().st_mode & 0o777, 0o700)

    def test_cross_filesystem_copy(self):
        with patch.object(runtime.os, "link", side_effect=OSError(errno.EXDEV, "cross-device")):
            target = self.pin()
        self.assertNotEqual(target.stat().st_ino, self.source.stat().st_ino)
        self.assertEqual(target.read_bytes(), b"opened executable")
        self.assertEqual(target.stat().st_mode & 0o777, 0o500)

    def test_source_replacement_copies_held_inode(self):
        link = os.link
        def race(source, target):
            other = self.root / "new"
            other.write_bytes(b"replacement")
            other.replace(source)
            link(source, target)
        with patch.object(runtime.os, "link", side_effect=race):
            target = self.pin()
        self.assertEqual(target.read_bytes(), b"opened executable")
        self.assertEqual(self.source.read_bytes(), b"replacement")

    def test_gc_dead_reused_active_unknown_and_symlink(self):
        active = self.pin()
        ns = Path("/proc/self/ns/pid").stat().st_ino
        def entry(pid, start, suffix):
            path = self.state / f"pin-{pid}-{start}-{ns}-{suffix * 32}"
            path.mkdir(mode=0o700)
            (path / "rapidmaker-api").write_bytes(b"old")
            return path
        dead = entry(2147483647, 1, "a")
        reused = entry(os.getpid(), 0, "b")
        unknown = entry(2147483647, 1, "c")
        (unknown / "unknown").write_bytes(b"keep")
        symlink = self.state / f"pin-2147483647-1-{ns}-{'d' * 32}"
        symlink.symlink_to(unknown, target_is_directory=True)
        self.pin()
        self.assertFalse(dead.exists())
        self.assertFalse(reused.exists())
        self.assertTrue(active.exists())
        self.assertTrue(unknown.exists())
        self.assertTrue(symlink.is_symlink())

    def test_gc_preserves_proc_uncertainty(self):
        target = self.pin()
        ns = Path("/proc/self/ns/pid").stat().st_ino
        for error in [PermissionError(), ValueError(), IndexError()]:
            with patch.object(runtime, "_start", side_effect=error):
                runtime._collect(self.state, ns)
            self.assertTrue(target.exists())

    def test_refuses_public_or_symlink_root(self):
        self.state.mkdir(mode=0o755)
        with self.assertRaises(ValueError):
            self.pin()
        self.state.rmdir()
        self.state.symlink_to(self.root, target_is_directory=True)
        with self.assertRaises(ValueError):
            self.pin()

    def test_proc_parse_uncertainty(self):
        with patch.object(Path, "read_text", return_value="1 (comm) " + "0 " * 19 + "bad"):
            with self.assertRaises(ValueError):
                runtime._start(1)

    def test_exec_keeps_pid(self):
        code = "import os,sys; from pathlib import Path; from runtime import pin_executable; print(os.getpid(),flush=True); p=pin_executable(Path('/bin/sh'),Path(sys.argv[1])); os.execv(p,[str(p),'-c','echo $$'])"
        result = subprocess.check_output([sys.executable, "-c", code, str(self.state)], env={**os.environ, 'PYTHONPATH': str(Path(runtime.__file__).parent)}, text=True)
        first, second = result.splitlines()
        self.assertEqual(first, second)


if __name__ == "__main__":
    unittest.main()
