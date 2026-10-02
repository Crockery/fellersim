"""Generate and publish the AUR recipe; never rebuild or modify release binaries."""
import argparse
import hashlib
import json
import pathlib
import re
import subprocess
import tempfile
import urllib.request

REPOSITORY = "https://github.com/Crockery/fellersim"
REMOTE = "ssh://aur@aur.archlinux.org/fellersim-bin.git"


def validate_namcap(output):
    # The package deliberately preserves upstream bytes, including symbols and
    # the glibc loader dependency. Every other namcap warning/error is fatal.
    allowed = {
        "fellersim-bin W: ELF file ('usr/bin/fellersim') is unstripped.",
        "fellersim-bin W: Unused shared library '/usr/lib/ld-linux-x86-64.so.2' by file ('usr/bin/fellersim')",
        "fellersim-bin W: Unused shared library '/usr/lib64/ld-linux-x86-64.so.2' by file ('usr/bin/fellersim')",
    }
    issues = [line for line in output.splitlines() if re.search(r" [EW]: ", line) and line not in allowed]
    if issues:
        raise ValueError("Unexpected namcap diagnostics: " + "\n".join(issues))


def version_tuple(version):
    if not re.fullmatch(r"(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)", version):
        raise ValueError("Expected a stable numeric version, e.g. 0.2.0")
    return tuple(map(int, version.split(".")))


def archive_name(version):
    version_tuple(version)
    return f"fellersim-{version}-linux-x86_64.tar.gz"


def verify(archive, version):
    name = archive_name(version)
    if archive.name != name:
        raise ValueError(f"Expected archive {name}")
    checksum = archive.with_name(name + ".sha256").read_text().split()
    if len(checksum) != 2 or checksum[1] != name or not re.fullmatch(r"[0-9a-f]{64}", checksum[0]):
        raise ValueError("Invalid release checksum document")
    digest = hashlib.sha256(archive.read_bytes()).hexdigest()
    if digest != checksum[0]:
        raise ValueError("Release archive checksum mismatch")
    return digest


def generate(archive, version, revision, template, output):
    version_tuple(version)
    if revision < 1:
        raise ValueError("Package revision must be positive")
    digest = verify(archive, version)
    recipe = template.read_text()
    for key, value in {"VERSION": version, "PKGREL": str(revision), "SHA256": digest}.items():
        recipe = recipe.replace(f"@{key}@", value)
    output.mkdir(parents=True, exist_ok=True)
    (output / "PKGBUILD").write_text(recipe, newline="\n")
    # Checked against makepkg --printsrcinfo in the clean Arch build.
    metadata = [
        "pkgbase = fellersim-bin", "\tpkgdesc = Offline Fellowship character simulator",
        f"\tpkgver = {version}", f"\tpkgrel = {revision}", f"\turl = {REPOSITORY}",
        "\tarch = x86_64", "\tlicense = MIT", "\tdepends = glibc", "\tdepends = libgcc",
        f"\tprovides = fellersim={version}", "\tconflicts = fellersim",
        "\toptions = !strip", "\toptions = !debug",
        f"\tsource = {REPOSITORY}/releases/download/v{version}/{archive_name(version)}",
        f"\tsha256sums = {digest}", "", "pkgname = fellersim-bin", "",
    ]
    (output / ".SRCINFO").write_text("\n".join(metadata), newline="\n")


def download(tag, output):
    if not tag.startswith("v"):
        raise ValueError("Expected a v-prefixed release tag")
    version = tag[1:]
    name = archive_name(version)
    # Deliberately anonymous: AUR users must be able to fetch the same assets.
    with urllib.request.urlopen(f"https://api.github.com/repos/Crockery/fellersim/releases/tags/{tag}", timeout=60) as response:
        release = json.load(response)
    if release["draft"] or release["prerelease"] or release["tag_name"] != tag:
        raise ValueError("Only published stable releases can be submitted to AUR")
    output.mkdir(parents=True, exist_ok=True)
    for filename in [name, name + ".sha256"]:
        with urllib.request.urlopen(f"{REPOSITORY}/releases/download/{tag}/{filename}", timeout=60) as response:
            (output / filename).write_bytes(response.read())
    verify(output / name, version)


def package_version(metadata):
    fields = dict(re.findall(r"^\s*(pkgver|pkgrel) = (\S+)$", metadata, re.M))
    version = version_tuple(fields["pkgver"])
    if not re.fullmatch(r"[1-9][0-9]*", fields["pkgrel"]):
        raise ValueError("Invalid package revision")
    return (*version, int(fields["pkgrel"]))


def publish(recipe, remote):
    files = {name: (recipe / name).read_bytes() for name in ["PKGBUILD", ".SRCINFO"]}
    incoming = package_version(files[".SRCINFO"].decode())
    with tempfile.TemporaryDirectory(prefix="fellersim-aur-") as directory:
        checkout = pathlib.Path(directory)
        subprocess.run(["git", "clone", "-c", "core.autocrlf=false", remote, directory], check=True)
        def git(*args):
            return subprocess.check_output(["git", "-C", directory, *args], text=True).strip()
        old = checkout / ".SRCINFO"
        if old.exists():
            current = package_version(old.read_text())
            if incoming < current:
                raise ValueError("Refusing AUR version downgrade")
            if incoming == current:
                if all((checkout / name).read_bytes() == content for name, content in files.items()):
                    print("AUR recipe already current")
                    return
                raise ValueError("Recipe changed: increase pkgrel before publishing")
        tracked = git("ls-files").splitlines()
        if any(name not in files for name in tracked):
            raise ValueError("Unexpected tracked files in AUR repository; review before publishing")
        for name, content in files.items():
            (checkout / name).write_bytes(content)
        git("add", "--", "PKGBUILD", ".SRCINFO")
        git("-c", "user.name=Fellersim release automation", "-c", "user.email=41898282+github-actions[bot]@users.noreply.github.com", "commit", "-m", f"Update fellersim-bin to {'.'.join(map(str, incoming[:3]))}-{incoming[3]}")
        git("push", "origin", "HEAD:master")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="command", required=True)
    gen = commands.add_parser("generate")
    gen.add_argument("--archive", type=pathlib.Path, required=True)
    gen.add_argument("--version", required=True)
    gen.add_argument("--pkgrel", type=int, default=1)
    gen.add_argument("--template", type=pathlib.Path, required=True)
    gen.add_argument("--output", type=pathlib.Path, required=True)
    fetch = commands.add_parser("download")
    fetch.add_argument("--tag", required=True)
    fetch.add_argument("--output", type=pathlib.Path, required=True)
    push = commands.add_parser("publish")
    push.add_argument("--recipe", type=pathlib.Path, required=True)
    push.add_argument("--remote", default=REMOTE)
    args = parser.parse_args()
    if args.command == "generate":
        generate(args.archive, args.version, args.pkgrel, args.template, args.output)
    elif args.command == "download":
        download(args.tag, args.output)
    else:
        publish(args.recipe, args.remote)


if __name__ == "__main__":
    main()
