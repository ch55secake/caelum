"""Exercise the VM installer without a network connection or a Linux GUI."""
import hashlib
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import unittest


INSTALLER = Path(__file__).resolve().parents[2] / "crates/caelum-core/src/remote/native/bootstrap.sh"


class NativePeerInstall(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.home = Path(temporary.name)
        self.bin = self.home / "bin"
        self.bin.mkdir()

        def executable(name, script):
            path = self.bin / name
            path.write_text("#!/bin/sh\nset -eu\n" + script)
            path.chmod(0o700)
            return path

        executable("uname", "echo x86_64")
        executable("curl", """while [ "$#" -gt 3 ]; do shift; done
test "$2" = -o
cp "$HOME/${1##*/}" "$3"
""")
        if not shutil.which("sha256sum"):
            executable("sha256sum", 'exec shasum -a 256 -c "$3" >/dev/null')
        self.asset = self.home / "Caelum-v1.2.3-x86_64.AppImage"
        self.asset.write_text("""#!/bin/sh
set -eu
test "$1 $2" = '--appimage-extract usr/bin/caelum'
mkdir -p squashfs-root/usr/bin
printf '#!/bin/sh\\nprintf "%%s\\\\n" "$@"\\n' >squashfs-root/usr/bin/caelum
chmod 700 squashfs-root/usr/bin/caelum
""")
        self.line = f"{hashlib.sha256(self.asset.read_bytes()).hexdigest()}  {self.asset.name}\n"

    def install(self, manifest):
        (self.home / "SHA256SUMS").write_text(manifest)
        return subprocess.run(
            ["sh", str(INSTALLER), "1.2.3", "example/project"],
            env={"HOME": str(self.home), "PATH": f"{self.bin}:{os.environ['PATH']}"},
            capture_output=True, text=True, timeout=10,
        )

    def test_exact_checksum_before_extraction_and_cached_reattach(self):
        for manifest in ["", self.line * 2, "0" * 64 + self.line[64:]]:
            self.assertEqual(self.install(manifest).returncode, 66)
            self.assertFalse((self.home / ".local/lib/caelum-peer/v1.2.3/caelum").exists())
        self.assertEqual(self.install(self.line).stdout, "workspace-peer\nattach\nexample/project\n")
        self.asset.unlink()
        self.assertEqual(self.install("").stdout, "workspace-peer\nattach\nexample/project\n")
