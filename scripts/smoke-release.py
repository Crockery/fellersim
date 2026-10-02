"""Verify and run a downloaded release archive without source or runtime data files."""
import hashlib
import json
import pathlib
import shutil
import subprocess
import tempfile

release = pathlib.Path("release")
checksums = list(release.glob("*.sha256"))
assert len(checksums) == 1, "Expected one platform archive"
digest, name = checksums[0].read_text().split()
archive = release / name
assert hashlib.sha256(archive.read_bytes()).hexdigest() == digest
with tempfile.TemporaryDirectory() as directory:
    root = pathlib.Path(directory)
    shutil.unpack_archive(archive, root)
    (root / "catalog.json").unlink()  # The executable must contain all runtime data.
    binary = root / ("fellersim.exe" if name.endswith(".zip") else "fellersim")
    subprocess.run([binary, "--version"], check=True)
    for hero in ["ardeos", "elarion", "gunde", "mara", "rime", "tariq"]:
        result = subprocess.run([
            binary, "run", "--character", root / f"examples/{hero}.json",
            "--apl", root / f"default-apls/{hero}.apl", "--iterations", "100",
            "--seed", "0123456789abcdef", "--json",
        ], cwd=root, check=True, capture_output=True, text=True)
        value = json.loads(result.stdout)
        assert value["meanDps"] > 0 and value["iterations"] == 100
        assert value["seed"] == "0123456789abcdef"
    print("Downloaded archive checksum and all six standalone heroes passed.")
