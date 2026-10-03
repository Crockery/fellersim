"""Documentation links, release contents, and wiki publication without network writes."""
import contextlib
import importlib.util
import io
import json
import os
import pathlib
import shutil
import subprocess
import sys
import tarfile
import tempfile
import unittest
from unittest import mock
import zipfile

import apl_docs

SCRIPTS = pathlib.Path(__file__).resolve().parent
SPEC = importlib.util.spec_from_file_location('publish_wiki', SCRIPTS / 'publish-wiki.py')
wiki = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(wiki)
ROOT = next(root for root in SCRIPTS.parents if (root / 'Cargo.toml').is_file())
APP = ROOT / 'apps/fellersim'
DOCS = APP / 'docs/apl' if (APP / 'docs/apl').is_dir() else ROOT / 'docs/apl'
PUBLIC = ROOT if (ROOT / 'docs/apl').is_dir() else APP


def git(root, *args):
    return subprocess.check_output(['git', '-C', str(root), *args], text=True, stderr=subprocess.PIPE).strip()


class DocumentationTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = pathlib.Path(self.temporary.name)
        self.source = self.root / 'source'
        self.source.mkdir()
        shutil.copytree(DOCS, self.source / 'docs/apl')
        shutil.copytree(PUBLIC / 'default-apls', self.source / 'default-apls')
        shutil.copytree(APP / 'examples', self.source / 'apps/fellersim/examples')
        for name in ['README.md', 'agent-guide.md', 'LICENSE', 'NOTICE']:
            shutil.copyfile(PUBLIC / name, self.source / name)
        shutil.copytree(PUBLIC / 'packaging', self.source / 'packaging')
        (self.source / 'Cargo.toml').write_text('[workspace.package]\nversion = "0.2.0"\n')
        identity = mock.patch.dict(os.environ, {
            'GIT_AUTHOR_NAME': 'Documentation test', 'GIT_AUTHOR_EMAIL': 'docs@example.invalid',
            'GIT_COMMITTER_NAME': 'Documentation test', 'GIT_COMMITTER_EMAIL': 'docs@example.invalid',
        })
        identity.start()
        self.addCleanup(identity.stop)
        git(self.source, 'init', '--initial-branch=main')
        git(self.source, 'add', '.')
        git(self.source, 'commit', '-m', 'Public documentation fixture')
        self.revision = git(self.source, 'rev-parse', 'HEAD')
        self.pages = apl_docs.render(self.source, self.revision, '0.2.0')

    def remote(self):
        remote = self.root / 'wiki.git'
        subprocess.run(['git', 'init', '--bare', '--initial-branch=master', remote], check=True, capture_output=True)
        initial = self.root / 'wiki-initial'
        subprocess.run(['git', 'clone', str(remote), str(initial)], check=True, capture_output=True)
        (initial / 'Home.md').write_text('# Welcome\n')
        (initial / 'Unrelated.md').write_text('Keep this page.\n')
        git(initial, 'add', '.')
        git(initial, 'commit', '-m', 'Initialize wiki')
        git(initial, 'push', 'origin', 'HEAD')
        return remote

    def publish(self, remote, publish=True, pages=None):
        with contextlib.redirect_stdout(io.StringIO()):
            return wiki.sync_pages(pages or self.pages, self.revision, str(remote), publish)

    def test_source_links_and_rendered_navigation(self):
        apl_docs.check_pages(PUBLIC, DOCS)
        self.assertEqual(set(self.pages), {f'{slug}.md' for slug in apl_docs.PAGES.values()})
        for content in self.pages.values():
            self.assertIn(f'/commit/{self.revision}', content)
            self.assertIn('Fellersim 0.2.0', content)
            self.assertIn('/wiki/APL-Language-reference', content)
            self.assertNotIn('<!-- apl-test', content)
            self.assertNotIn('fellowscript', content)
            for match in apl_docs.LINK.finditer(content):
                self.assertTrue(match[2].startswith('https://'), match[2])
        self.assertIn(f'/tree/{self.revision}/default-apls', self.pages['Home.md'])
        self.assertIn(f'/blob/{self.revision}/agent-guide.md', self.pages['Home.md'])
        self.assertIn('/wiki/APL-Checking-combat-state#finding-valid-names', self.pages['APL-Language-reference.md'])
        for name in ['README.md', 'agent-guide.md']:
            self.assertIn('(docs/apl/Home.md)', (PUBLIC / name).read_text())

    def test_broken_page_and_heading_links_fail(self):
        page = self.source / 'docs/apl/Home.md'
        original = page.read_text()
        for link in ['Missing.md', 'Language-reference.md#missing', '../../../../outside.md']:
            page.write_text(original + f'\n[Broken]({link})\n')
            with self.subTest(link=link), self.assertRaises(ValueError):
                apl_docs.check_pages(self.source)

    def test_release_archives_include_readable_docs(self):
        binary_dir = self.source / 'target/release'
        binary_dir.mkdir(parents=True)
        for name in ['fellersim', 'fellersim.exe']:
            (binary_dir / name).write_bytes(b'archive fixture, not executed')
        catalog = self.source / 'crates/fellersim-data/data/catalog.json'
        catalog.parent.mkdir(parents=True)
        catalog.write_text('{}')
        for platform, suffix in [('linux-x86_64', '.tar.gz'), ('windows-x86_64', '.zip')]:
            subprocess.run([sys.executable, SCRIPTS / 'package-release.py', platform], cwd=self.source, check=True)
            archive = self.source / 'release' / f'fellersim-0.2.0-{platform}{suffix}'
            with zipfile.ZipFile(archive) if suffix == '.zip' else tarfile.open(archive) as package:
                names = package.namelist() if suffix == '.zip' else package.getnames()
                for name in apl_docs.PAGES:
                    path = f'docs/apl/{name}'
                    self.assertIn(path, names)
                    content = package.read(path) if suffix == '.zip' else package.extractfile(path).read()
                    self.assertEqual(content, (DOCS / name).read_bytes())

    def test_arch_install_keeps_local_links(self):
        # Execute only package(), with an isolated prefix and inert binary fixture.
        # The full Arch smoke check additionally verifies actual installation.
        if os.name != 'posix' or shutil.which('bash') is None:
            self.skipTest('Arch packaging requires a Linux shell environment')
        (self.source / 'fellersim').write_text('fixture')
        (self.source / 'catalog.json').write_text('{}')
        shutil.copytree(self.source / 'apps/fellersim/examples', self.source / 'examples')
        package_root = self.root / 'package'
        recipe = self.source / 'packaging/aur/PKGBUILD.in'
        subprocess.run(['bash', '-c', 'source "$1"; pkgdir="$2"; package', 'package-test', str(recipe), str(package_root)],
                       cwd=self.source, check=True, capture_output=True)
        installed = package_root / 'usr/share/doc/fellersim'
        for name in apl_docs.PAGES:
            self.assertEqual((installed / 'docs/apl' / name).read_bytes(), (DOCS / name).read_bytes())
        self.assertEqual(os.readlink(installed / 'default-apls'), '/usr/share/fellersim/default-apls')
        self.assertTrue((package_root / 'usr/share/fellersim/default-apls/ardeos.apl').is_file())

    def test_preview_publish_update_and_no_op(self):
        remote = self.remote()
        before = git(remote, 'rev-parse', 'HEAD')
        self.assertTrue(self.publish(remote, False))
        self.assertEqual(before, git(remote, 'rev-parse', 'HEAD'))
        self.assertTrue(self.publish(remote))
        published = git(remote, 'rev-parse', 'HEAD')
        self.assertNotEqual(before, published)
        self.assertFalse(self.publish(remote))
        self.assertEqual(published, git(remote, 'rev-parse', 'HEAD'))
        changed = dict(self.pages)
        changed['Home.md'] += '\nUpdated explanation.\n'
        self.assertTrue(self.publish(remote, pages=changed))
        self.assertNotEqual(published, git(remote, 'rev-parse', 'HEAD'))
        self.assertEqual(git(remote, 'show', 'HEAD:Unrelated.md'), 'Keep this page.')

    def test_uninitialized_wiki_is_actionable(self):
        remote = self.root / 'empty.git'
        subprocess.run(['git', 'init', '--bare', remote], check=True, capture_output=True)
        with self.assertRaisesRegex(ValueError, 'first Home page'):
            self.publish(remote)

    def test_concurrent_update_rejects_push_without_retry(self):
        remote = self.remote()
        original_git = wiki.git
        pushed = 0

        def concurrent_git(root, *args):
            nonlocal pushed
            if args[0] == 'push':
                pushed += 1
                other = self.root / 'concurrent'
                subprocess.run(['git', 'clone', str(remote), str(other)], check=True, capture_output=True)
                (other / 'Unrelated.md').write_text('New human edit.\n')
                git(other, 'add', '.')
                git(other, 'commit', '-m', 'Concurrent edit')
                git(other, 'push', 'origin', 'HEAD')
            return original_git(root, *args)

        with mock.patch.object(wiki, 'git', side_effect=concurrent_git):
            with self.assertRaises(subprocess.CalledProcessError):
                self.publish(remote)
        self.assertEqual(pushed, 1)
        self.assertEqual(git(remote, 'show', 'HEAD:Unrelated.md'), 'New human edit.')
        self.assertEqual(git(remote, 'show', 'HEAD:Home.md'), '# Welcome')

    def test_publication_requires_clean_current_verified_public_main(self):
        commands = {
            ('status', '--porcelain', '--untracked-files=all'): '',
            ('branch', '--show-current'): 'main',
            ('remote', 'get-url', 'origin'): 'git@github.com:Crockery/fellersim.git',
            ('rev-parse', 'HEAD'): self.revision,
            ('ls-remote', 'origin', 'refs/heads/main'): f'{self.revision}\trefs/heads/main',
        }
        run = dict(head_sha=self.revision, head_branch='main', event='push', run_number=1,
                   status='completed', conclusion='success')
        with mock.patch.object(wiki, 'git', side_effect=lambda root, *args: commands[args]):
            with mock.patch.object(wiki.subprocess, 'check_output', return_value=json.dumps({'workflow_runs': [run]})):
                self.assertEqual(wiki.verify_source(self.source), self.revision)
                for key, bad in [
                    (('status', '--porcelain', '--untracked-files=all'), ' M docs/apl/Home.md'),
                    (('branch', '--show-current'), 'feature'),
                    (('remote', 'get-url', 'origin'), 'git@github.com:Crockery/fellowscript.git'),
                    (('ls-remote', 'origin', 'refs/heads/main'), 'different\trefs/heads/main'),
                ]:
                    with mock.patch.dict(commands, {key: bad}), self.assertRaises(ValueError):
                        wiki.verify_source(self.source)
            for runs in [[], [dict(run, conclusion='failure')], [dict(run, status='in_progress')],
                         [run, dict(run, run_number=2, conclusion='failure')]]:
                with mock.patch.object(wiki.subprocess, 'check_output', return_value=json.dumps({'workflow_runs': runs})):
                    with self.assertRaisesRegex(ValueError, 'workflow'):
                        wiki.verify_source(self.source)


if __name__ == '__main__':
    unittest.main()
