# Maintaining fellersim-bin

The Fellership monorepo owns this template and its tooling. Its public source
export includes them in Fellersim; release automation generates `PKGBUILD` and
`.SRCINFO` for the separate [AUR repository](https://aur.archlinux.org/packages/fellersim-bin).
Do not edit generated recipes in AUR without bringing the change into the template.

AUR publication is temporarily paused pending account registration and SSH-key
setup. Arch package checks and GitHub releases remain active. To resume, uncomment
the `aur-publish` job, `workflow_dispatch` inputs, and `build` job filter together
in the release workflow source, update its publication-policy test, and export
the changes. The release and retry instructions below apply after re-enabling it.

## Account setup

Create an account at <https://aur.archlinux.org/register> and register a dedicated
Ed25519 public key in its account settings. Keep its private key in the public
Fellersim repository's GitHub Actions secret `AUR_SSH_PRIVATE_KEY`. The account
must maintain `fellersim-bin`; the initial push creates the package when its name
is available. Never commit the private key.

`known_hosts` pins AUR's Ed25519 host key. Its fingerprint is
`SHA256:RFzBCUItH9LZS0cKB5UE6ceAYhBD5C8GeOBip8Z11+4`, published at
<https://aur.archlinux.org/>. If AUR rotates it, verify the replacement against
Arch's published information before updating this file.

The GitHub repository and release assets must be public. The publication job
intentionally downloads without a token, just as an AUR user would.

## Releases and retries

Normal stable `vMAJOR.MINOR.PATCH` releases run Windows, Linux, and Arch package
checks before GitHub publication. The next job anonymously downloads the released
Linux archive, verifies its checksum, builds and tests the final Arch package,
and pushes only `PKGBUILD` and `.SRCINFO` to AUR. Every new version starts at
`pkgrel=1`. Drafts and prereleases are not eligible.

An AUR failure does not remove the GitHub release. Fix its cause, then run the
public **Fellersim** workflow using **Run workflow**, select `main`, supply the
existing tag in `aur_tag`, and set `pkgrel` (normally `1`). An empty tag runs the
normal build checks without publishing. Manual publication always uses the
packaging tools from `main`.

For a packaging-only correction, update and export the template/tooling first,
then dispatch the existing tag with a higher `pkgrel`. Identical retries do
nothing; changing a recipe without increasing its version/revision is rejected.
Older versions cannot replace newer AUR versions. Concurrent publications are
serialized and Git pushes never overwrite remote history.

The workflow retains the tested package and recipe as artifacts. Generated
release checksums are not committed back to the source repositories.

## Local verification

From a public source checkout with Python 3, Git, and Docker available:

```sh
python -m unittest discover -s scripts -p 'test_*.py'
python scripts/aur.py download --tag v0.2.0 --output release
python scripts/check-aur.py \
  --archive release/fellersim-0.2.0-linux-x86_64.tar.gz \
  --version 0.2.0 --template packaging/aur/PKGBUILD.in --output aur-package
```

Before a release exists, the same check accepts the local archive and checksum
produced by `scripts/package-release.py linux-x86_64`. The archive is cached under
its final filename; makepkg still verifies its pinned checksum. The package is
built unprivileged in Arch and installed in a separate Arch runtime container.
Namcap errors and warnings fail the check, except its exact notices about the
preserved executable's symbols and glibc loader dependency; neither warrants
rewriting the release binary. Checks cover dependencies, installed files, all six heroes, embedded and
visible default APL parity, deterministic results, upgrade, and removal.

The recipe requires no Rust toolchain. It installs the original executable
without stripping it, alongside default APLs, examples, and catalog in
`/usr/share/fellersim`, documentation in `/usr/share/doc/fellersim`, and license
notices in `/usr/share/licenses/fellersim-bin`. Copy examples/APLs to a writable
working directory before editing them.
