"""Recipe integrity and publication tests, independent of AUR credentials."""
import hashlib
import pathlib
import subprocess
import tempfile
import unittest
from unittest import mock

import aur

ROOTS = pathlib.Path(__file__).resolve().parents
TEMPLATE = next(p for root in [ROOTS[1], ROOTS[2]] for p in [root / 'apps/fellersim/packaging/aur/PKGBUILD.in', root / 'packaging/aur/PKGBUILD.in'] if p.exists())


class PackagingTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = pathlib.Path(self.temporary.name)
        self.archive = self.root / aur.archive_name('0.2.0')
        self.archive.write_bytes(b'test archive')
        digest = hashlib.sha256(self.archive.read_bytes()).hexdigest()
        self.archive.with_name(self.archive.name + '.sha256').write_text(f'{digest}  {self.archive.name}\n')
        self.recipe = self.root / 'recipe'
        aur.generate(self.archive, '0.2.0', 1, TEMPLATE, self.recipe)

    def test_generation(self):
        recipe = (self.recipe / 'PKGBUILD').read_text()
        self.assertNotIn('@VERSION@', recipe)
        for name in ['PKGBUILD', '.SRCINFO']:
            self.assertNotIn(b'\r', (self.recipe / name).read_bytes())
        self.assertIn('pkgver=0.2.0', recipe)
        self.assertIn("options=('!strip' '!debug')", recipe)
        self.assertEqual(aur.package_version((self.recipe / '.SRCINFO').read_text()), (0, 2, 0, 1))

    def test_namcap_preservation_exceptions_are_narrow(self):
        aur.validate_namcap("fellersim-bin W: ELF file ('usr/bin/fellersim') is unstripped.")
        aur.validate_namcap("fellersim-bin W: Unused shared library '/usr/lib64/ld-linux-x86-64.so.2' by file ('usr/bin/fellersim')")
        for message in ["fellersim-bin E: Dependency glibc missing", "fellersim-bin W: Dependency libgcc missing", "PKGBUILD W: Invalid architecture"]:
            with self.assertRaisesRegex(ValueError, 'namcap'):
                aur.validate_namcap(message)

    def test_bad_versions_and_revisions(self):
        for version in ['v0.2.0', '0.2.0-rc1', '0.2', '01.2.0', '$(id)', '../0.2.0']:
            with self.subTest(version=version), self.assertRaises(ValueError):
                aur.version_tuple(version)
        with self.assertRaises(ValueError):
            aur.generate(self.archive, '0.2.0', 0, TEMPLATE, self.recipe)

    def test_missing_and_corrupt_assets(self):
        self.archive.unlink()
        with self.assertRaises(FileNotFoundError):
            aur.verify(self.archive, '0.2.0')
        self.archive.write_bytes(b'changed')
        with self.assertRaisesRegex(ValueError, 'mismatch'):
            aur.verify(self.archive, '0.2.0')
        self.archive.with_name(self.archive.name + '.sha256').write_text('invalid')
        with self.assertRaisesRegex(ValueError, 'document'):
            aur.verify(self.archive, '0.2.0')

    def test_download_rejects_nonstable_releases(self):
        with mock.patch('urllib.request.urlopen') as request:
            request.return_value.__enter__.return_value.read.return_value = b'{"draft":true,"prerelease":false,"tag_name":"v0.2.0"}'
            with self.assertRaisesRegex(ValueError, 'stable'):
                aur.download('v0.2.0', self.root)
        with self.assertRaises(ValueError):
            aur.download('0.2.0', self.root)

    def test_publication_and_retries(self):
        remote = self.root / 'remote.git'
        subprocess.run(['git', 'init', '--bare', '--initial-branch=master', remote], check=True, capture_output=True)
        def head():
            return subprocess.check_output(['git', '--git-dir', remote, 'rev-parse', 'master'], text=True)
        aur.publish(self.recipe, str(remote))
        first = head()
        aur.publish(self.recipe, str(remote))
        self.assertEqual(first, head())
        (self.recipe / 'PKGBUILD').write_text((self.recipe / 'PKGBUILD').read_text() + '\n# changed\n')
        with self.assertRaisesRegex(ValueError, 'pkgrel'):
            aur.publish(self.recipe, str(remote))
        aur.generate(self.archive, '0.2.0', 2, TEMPLATE, self.recipe)
        aur.publish(self.recipe, str(remote))
        self.assertNotEqual(first, head())
        aur.generate(self.archive, '0.2.0', 1, TEMPLATE, self.recipe)
        with self.assertRaisesRegex(ValueError, 'downgrade'):
            aur.publish(self.recipe, str(remote))
        self.assertEqual(subprocess.check_output(['git', '--git-dir', remote, 'ls-tree', '--name-only', 'master'], text=True).splitlines(), ['.SRCINFO', 'PKGBUILD'])


if __name__ == '__main__':
    unittest.main()
