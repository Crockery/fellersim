"""Preview or publish checked APL pages from a public Fellersim checkout."""
import argparse
import difflib
import json
import pathlib
import subprocess
import tempfile
import tomllib

from apl_docs import PUBLIC_URL, PUBLICATION_NOTICE, render

WIKI_REMOTE = 'git@github.com:Crockery/fellersim.wiki.git'
PUBLIC_REMOTES = {f'{PUBLIC_URL}.git', PUBLIC_URL, 'git@github.com:Crockery/fellersim.git',
                  'ssh://git@github.com/Crockery/fellersim.git'}


def git(root, *args):
    return subprocess.check_output(['git', '-C', str(root), *args], text=True).strip()


def verify_source(root):
    """Publish only the exact public main revision whose push workflow passed."""
    if git(root, 'status', '--porcelain', '--untracked-files=all'):
        raise ValueError('Public checkout has local changes; commit and verify them before publishing.')
    if git(root, 'branch', '--show-current') != 'main':
        raise ValueError('Publish from the public main branch.')
    if git(root, 'remote', 'get-url', 'origin') not in PUBLIC_REMOTES:
        raise ValueError('origin must be Crockery/fellersim, not the private monorepo.')
    revision = git(root, 'rev-parse', 'HEAD')
    remote = git(root, 'ls-remote', 'origin', 'refs/heads/main').split()
    if not remote or remote[0] != revision:
        raise ValueError('Checkout must match the current public main revision.')
    response = subprocess.check_output([
        'gh', 'api', f'repos/Crockery/fellersim/actions/workflows/release.yml/runs?head_sha={revision}&branch=main&event=push&per_page=100',
    ], text=True)
    runs = [r for r in json.loads(response)['workflow_runs']
            if r['head_sha'] == revision and r['head_branch'] == 'main' and r['event'] == 'push']
    latest = max(runs, key=lambda r: r['run_number'], default=None)
    if not latest or latest['status'] != 'completed' or latest['conclusion'] != 'success':
        raise ValueError('The public main push workflow must finish successfully before wiki publication.')
    return revision


def sync_pages(pages, revision, remote, publish):
    """Update rendered pages and remove obsolete generated APL pages."""
    with tempfile.TemporaryDirectory(prefix='fellersim-wiki-') as temporary:
        checkout = pathlib.Path(temporary)
        result = subprocess.run(['git', 'clone', '-c', 'core.autocrlf=false', '--', remote, temporary],
                                text=True, capture_output=True)
        if result.returncode:
            raise ValueError('Cannot read the wiki repository. Check Git access. If it has no pages, '
                             f'create its first Home page at {PUBLIC_URL}/wiki, then retry.\n{result.stderr}')
        if subprocess.run(['git', '-C', temporary, 'rev-parse', '--verify', 'HEAD'],
                          capture_output=True).returncode:
            raise ValueError(f'Wiki is uninitialized. Create its first Home page at {PUBLIC_URL}/wiki, then retry.')
        obsolete = []
        for target in sorted(checkout.glob('APL-*.md')):
            if target.name in pages or target.is_symlink() or not target.is_file():
                continue
            old = target.read_text(encoding='utf-8')
            if old.rstrip().endswith(PUBLICATION_NOTICE) and f']({PUBLIC_URL}/commit/' in old:
                obsolete.append(target.name)
                print(''.join(difflib.unified_diff(old.splitlines(True), [],
                                                 fromfile=f'wiki/{target.name}', tofile='/dev/null')), end='')
        changed = []
        for name, text in pages.items():
            if pathlib.PurePath(name).name != name or not name.endswith('.md'):
                raise ValueError(f'Invalid managed page name: {name}')
            target = checkout / name
            if target.is_symlink() or (target.exists() and not target.is_file()):
                raise ValueError(f'Managed page is not a regular file: {name}')
            old = target.read_text(encoding='utf-8') if target.exists() else ''
            if old == text:
                continue
            print(''.join(difflib.unified_diff(old.splitlines(True), text.splitlines(True),
                                             fromfile=f'wiki/{name}', tofile=f'source/{name}')), end='')
            changed.append(name)
            if publish:
                target.write_text(text, encoding='utf-8', newline='\n')
        if not changed and not obsolete:
            print('Wiki already matches the source documentation.')
            return False
        if not publish:
            print(f'Preview only: {len(changed)} pages would change; {len(obsolete)} obsolete generated pages '
                  'would be removed. Use --publish after review.')
            return True
        if obsolete:
            git(checkout, 'rm', '--', *obsolete)
        if changed:
            git(checkout, 'add', '--', *changed)
        git(checkout, 'commit', '-m', f'Document APLs from fellersim {revision[:12]}')
        # No force, retry, or rebase: concurrent edits must be reviewed.
        git(checkout, 'push', 'origin', 'HEAD')
        print(f'Published {len(changed)} pages and removed {len(obsolete)} obsolete generated pages '
              f'at {PUBLIC_URL}/wiki.')
        return True


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('checkout', type=pathlib.Path, help='Public source checkout containing docs/apl')
    parser.add_argument('--publish', action='store_true', help='Commit and push after source/CI checks')
    args = parser.parse_args()
    root = args.checkout.resolve()
    try:
        revision = verify_source(root) if args.publish else git(root, 'rev-parse', 'HEAD')
        version = tomllib.loads((root / 'Cargo.toml').read_text(encoding='utf-8'))['workspace']['package']['version']
        pages = render(root, revision, version)
        if args.publish and (git(root, 'rev-parse', 'HEAD') != revision
                             or git(root, 'status', '--porcelain', '--untracked-files=all')):
            raise ValueError('Source changed during preparation; review it and retry.')
        sync_pages(pages, revision, WIKI_REMOTE, args.publish)
    except (ValueError, OSError, subprocess.CalledProcessError) as error:
        parser.exit(1, f'{error}\n')


if __name__ == '__main__':
    main()
