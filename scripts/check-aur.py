"""Build a binary package in Arch, then install/test it in a separate runtime container."""
import argparse
import hashlib
import json
import pathlib
import shutil
import subprocess
import tarfile
import tempfile
import uuid

import aur
from apl_docs import PAGES

IMAGE = 'archlinux:base'
HEROES = [('ardeos', 'firemage'), ('elarion', 'bowguy'), ('gunde', 'gunde'), ('mara', 'mara'), ('rime', 'rime'), ('tariq', 'ink')]


def run(*args, **kwargs):
    return subprocess.run(list(map(str, args)), check=True, **kwargs)


def check(archive, version, revision, template, output):
    digest = aur.verify(archive, version)
    # Reject malformed release contents before any extraction by makepkg.
    with tarfile.open(archive) as package:
        members = package.getmembers()
        names = {m.name for m in members}
        required = {'fellersim', 'catalog.json', 'README.md', 'agent-guide.md', 'LICENSE', 'NOTICE', 'examples/request.json', 'examples/variants.json', 'packaging/aur/README.md'}
        required.update(f'default-apls/{hero}.apl' for hero, _ in HEROES)
        required.update(f'examples/{hero}.json' for hero, _ in HEROES)
        required.update(f'docs/apl/{name}' for name in PAGES)
        if not required <= names or len(names) != len(members):
            raise ValueError('Release is missing required files or has duplicate archive paths')
        if any(not m.isfile() or pathlib.PurePosixPath(m.name).is_absolute() or '..' in pathlib.PurePosixPath(m.name).parts for m in members):
            raise ValueError('Release archive contains an unsafe path or nonregular file')
        binary_digest = hashlib.sha256(package.extractfile('fellersim').read()).hexdigest()
        apls = {hero: package.extractfile(f'default-apls/{hero}.apl').read().decode() for hero, _ in HEROES}
    aur.generate(archive, version, revision, template, output)
    with tempfile.TemporaryDirectory(prefix='fellersim-arch-') as directory:
        work = pathlib.Path(directory)
        for child in ['recipe', 'upgrade', 'cache', 'packages']:
            (work / child).mkdir()
        for child, rel in [('recipe', revision), ('upgrade', revision + 1)]:
            aur.generate(archive, version, rel, template, work / child)
        shutil.copyfile(archive, work / 'cache' / archive.name)
        # All build files are disposable; no source checkout or credentials enter the container.
        build = r'''
set -eu
owner=$(stat -c '%u:%g' /work)
trap 'chown -R "$owner" /work' EXIT
pacman -Syu --noconfirm --needed base-devel namcap diffutils
useradd -m builder
chown -R builder:builder /work
for recipe in recipe upgrade; do
  cd /work/$recipe
  runuser -u builder -- sh -c 'makepkg --printsrcinfo > actual.SRCINFO'
  diff -u .SRCINFO actual.SRCINFO
  runuser -u builder -- env SRCDEST=/work/cache PKGDEST=/work/packages makepkg --cleanbuild --force --noconfirm
  namcap PKGBUILD > /work/$recipe/recipe-lint.txt
  cat /work/$recipe/recipe-lint.txt
  if grep -E ' (E|W): ' /work/$recipe/recipe-lint.txt; then exit 1; fi
 done
for package in /work/packages/*.pkg.tar.zst; do
  namcap "$package" > "/work/$(basename "$package").lint"
  cat "/work/$(basename "$package").lint"
 done
readelf -d /work/recipe/src/fellersim > /work/dynamic.txt
'''
        run('docker', 'run', '--rm', '-v', f'{work}:/work', IMAGE, 'bash', '-c', build)
        for lint in work.glob('*.lint'):
            aur.validate_namcap(lint.read_text())
        needed = [line.split('[', 1)[1].split(']', 1)[0] for line in (work / 'dynamic.txt').read_text().splitlines() if '(NEEDED)' in line]
        if set(needed) - {'libgcc_s.so.1', 'libm.so.6', 'libc.so.6', 'ld-linux-x86-64.so.2'}:
            raise ValueError(f'Undeclared runtime libraries: {needed}')
        container = f'fellersim-aur-{uuid.uuid4().hex}'
        def execute(*args, **kwargs):
            return run('docker', 'exec', container, *args, **kwargs)
        def cli(*args):
            return json.loads(execute('fellersim', *args, '--json', '--quiet', capture_output=True, text=True).stdout)
        try:
            run('docker', 'run', '-d', '--name', container, '-v', f'{work / "packages"}:/packages:ro', '-w', '/tmp', IMAGE, 'sleep', 'infinity', stdout=subprocess.DEVNULL)
            execute('pacman', '-Syu', '--noconfirm')
            package_name = f'fellersim-bin-{version}-{revision}-x86_64.pkg.tar.zst'
            execute('pacman', '-U', '--noconfirm', f'/packages/{package_name}')
            # Runtime image has neither the builder nor Rust, Python, source, or cached release files.
            execute('sh', '-c', '! command -v cargo && ! command -v cc')
            assert execute('sha256sum', '/usr/bin/fellersim', capture_output=True, text=True).stdout.split()[0] == binary_digest
            assert cli('version')['ok']
            assert cli('describe')['ok']
            assert execute('fellersim', '--version', capture_output=True, text=True).stdout.find(version) >= 0
            for hero, identifier in HEROES:
                character = f'/usr/share/fellersim/examples/{hero}.json'
                apl = f'/usr/share/fellersim/default-apls/{hero}.apl'
                assert cli('validate', '--character', character, '--default-apl')['ok']
                visible = execute('cat', apl, capture_output=True, text=True).stdout
                embedded = execute('fellersim', 'apl', 'default', '--hero', identifier, capture_output=True, text=True).stdout
                assert visible == apls[hero] and embedded.rstrip('\n') == visible.rstrip('\n')
                options = ['--character', character, '--iterations', '100', '--seed', '0123456789abcdef']
                default = cli('run', *options, '--default-apl')
                explicit = cli('run', *options, '--apl', apl)
                assert default['ok'] and default['data']['meanDps'] > 0
                assert default['data'] == explicit['data']
            assert cli('run', '--input', '/usr/share/fellersim/examples/request.json')['ok']
            comparison = cli('compare', '--input', '/usr/share/fellersim/examples/variants.json', '--baseline', 'original')
            assert comparison['data']['cases'][1]['comparison']['absoluteChange'] == 0
            upgrade = f'/packages/fellersim-bin-{version}-{revision + 1}-x86_64.pkg.tar.zst'
            execute('pacman', '-U', '--noconfirm', upgrade)
            assert execute('pacman', '-Q', 'fellersim-bin', capture_output=True, text=True).stdout.strip() == f'fellersim-bin {version}-{revision + 1}'
            assert cli('version')['ok']
            listing = execute('pacman', '-Qlq', 'fellersim-bin', capture_output=True, text=True).stdout.splitlines()
            assert all(p.startswith(('/usr/bin/', '/usr/share/')) or p == '/usr/' for p in listing)
            for required in ['/usr/share/fellersim/catalog.json', '/usr/share/doc/fellersim/agent-guide.md', '/usr/share/licenses/fellersim-bin/LICENSE', '/usr/share/licenses/fellersim-bin/NOTICE']:
                assert required in listing
            for page in PAGES:
                assert f'/usr/share/doc/fellersim/docs/apl/{page}' in listing
            execute('test', '-f', '/usr/share/doc/fellersim/default-apls/ardeos.apl')
            execute('pacman', '-R', '--noconfirm', 'fellersim-bin')
            execute('sh', '-c', 'test ! -e /usr/bin/fellersim && test ! -e /usr/share/fellersim && test ! -e /usr/share/doc/fellersim && test ! -e /usr/share/licenses/fellersim-bin')
        finally:
            run('docker', 'rm', '-f', container, stdout=subprocess.DEVNULL)
        # Preserve exactly the tested recipe and package as CI artifacts.
        shutil.copyfile(work / 'packages' / package_name, output / package_name)
        (output / 'archive.sha256').write_text(f'{digest}  {archive.name}\n')
    print('Arch package installation, all heroes, upgrade, and removal passed.')


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--archive', type=pathlib.Path, required=True)
    parser.add_argument('--version', required=True)
    parser.add_argument('--pkgrel', type=int, default=1)
    parser.add_argument('--template', type=pathlib.Path, required=True)
    parser.add_argument('--output', type=pathlib.Path, required=True)
    args = parser.parse_args()
    check(args.archive.resolve(), args.version, args.pkgrel, args.template.resolve(), args.output.resolve())
