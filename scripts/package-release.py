"""Assemble downloadable CLI archives using only the public export."""
import hashlib
import pathlib
import sys
import tarfile
import tomllib
import zipfile

platform = sys.argv[1]
version = tomllib.loads(pathlib.Path("Cargo.toml").read_text())["workspace"]["package"]["version"]
suffix = ".exe" if platform.startswith("windows") else ""
binary = pathlib.Path(f"target/release/fellersim{suffix}")
files = [(binary, binary.name)]
files += [(pathlib.Path(name), name) for name in ["README.md", "agent-guide.md", "LICENSE", "NOTICE"]]
files += [(p, f"examples/{p.name}") for p in pathlib.Path("apps/fellersim/examples").iterdir() if p.is_file()]
files += [(p, f"default-apls/{p.name}") for p in pathlib.Path("default-apls").iterdir() if p.is_file()]
files += [(pathlib.Path("crates/fellersim-data/data/catalog.json"), "catalog.json")]
files += [(pathlib.Path("packaging/aur/README.md"), "packaging/aur/README.md")]
destination = pathlib.Path("release")
destination.mkdir(exist_ok=True)
name = f"fellersim-{version}-{platform}"
if suffix:
    archive = destination / f"{name}.zip"
    with zipfile.ZipFile(archive, "w", zipfile.ZIP_DEFLATED) as output:
        for source, relative in files:
            output.write(source, relative)
else:
    archive = destination / f"{name}.tar.gz"
    with tarfile.open(archive, "w:gz") as output:
        for source, relative in files:
            output.add(source, arcname=relative)
digest = hashlib.sha256(archive.read_bytes()).hexdigest()
archive.with_name(archive.name + ".sha256").write_text(f"{digest}  {archive.name}\n")
