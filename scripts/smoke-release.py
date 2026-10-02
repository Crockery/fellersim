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
        response = json.loads(result.stdout)
        assert response["ok"] and response["protocolVersion"] == 1
        value = response["data"]
        assert value["meanDps"] > 0 and value["iterations"] == 100
        assert value["seed"] == "0123456789abcdef"
    shutil.rmtree(root / "default-apls")  # Defaults must also be embedded.
    for hero in ["firemage", "rime", "ink", "bowguy", "mara", "gunde"]:
        document = subprocess.check_output([binary, "character", "init", "--hero", hero], text=True)
        request = {"version": 1, "character": {"kind": "inline", "document": json.loads(document)}, "apl": {"kind": "default"}, "options": {"iterations": 100}}
        result = subprocess.run([binary, "run", "--input", "-", "--json", "--quiet"], input=json.dumps(request), cwd=root, capture_output=True, text=True, check=True)
        assert json.loads(result.stdout)["data"]["meanDps"] > 0
    comparison = subprocess.run([binary, "compare", "--input", root / "examples/variants.json", "--baseline", "original", "--json", "--quiet"], cwd=root, capture_output=True, text=True, check=True)
    assert json.loads(comparison.stdout)["data"]["cases"][1]["comparison"]["absoluteChange"] == 0
    print("Downloaded archive checksum and all six standalone heroes passed.")
