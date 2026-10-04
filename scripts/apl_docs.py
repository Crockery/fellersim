"""Shared APL page selection, link checks, and wiki rendering (standard library only)."""
import pathlib
import re
from urllib.parse import unquote, urlsplit

PUBLIC_URL = 'https://github.com/Crockery/fellersim'
PAGES = (
    'Home.md',
    'Getting-started.md',
    'Language-reference.md',
    'Checking-combat-state.md',
    'Why-actions-run.md',
    'Examples-and-common-mistakes.md',
)
PUBLICATION_NOTICE = 'Published from the source documentation. Make lasting edits there.'
LINK = re.compile(r'\[([^\]\n]+)\]\(([^\s)]+)\)')


def headings(text):
    """GitHub heading anchors for the plain headings used by these pages."""
    counts = {}
    anchors = set()
    for heading in re.findall(r'^#{1,6} (.+)$', text, re.M):
        slug = re.sub(r'[^\w\- ]', '', heading.lower()).replace(' ', '-')
        count = counts.get(slug, 0)
        counts[slug] = count + 1
        anchors.add(f'{slug}-{count}' if count else slug)
    return anchors


def local_target(page, href, root):
    url = urlsplit(href)
    if url.scheme or url.netloc:
        return None
    target = (page.parent / unquote(url.path)).resolve() if url.path else page.resolve()
    if not target.is_relative_to(root.resolve()):
        raise ValueError(f'Link leaves documentation distribution: {page}: {href}')
    if not target.exists():
        raise ValueError(f'Broken link: {page}: {href}')
    if url.fragment:
        if not target.is_file() or unquote(url.fragment) not in headings(target.read_text(encoding='utf-8')):
            raise ValueError(f'Broken heading link: {page}: {href}')
    return target, url.fragment


def check_pages(root, docs=None):
    root = pathlib.Path(root).resolve()
    docs = pathlib.Path(docs) if docs else root / 'docs/apl'
    if {p.name for p in docs.iterdir()} != set(PAGES):
        raise ValueError('APL documentation must contain exactly the declared pages')
    for name in PAGES:
        page = docs / name
        if page.is_symlink() or not page.is_file():
            raise ValueError(f'Expected a regular documentation file: {page}')
        text = page.read_text(encoding='utf-8')
        if not text.startswith('# ') or text.count('\n```') % 2:
            raise ValueError(f'Missing title or unclosed code block: {page}')
        for match in LINK.finditer(text):
            local_target(page, match[2], root)


def render(root, revision, version):
    root = pathlib.Path(root).resolve()
    check_pages(root)
    docs = root / 'docs/apl'
    home = (docs / 'Home.md').read_text(encoding='utf-8')
    section = re.search(r'^## Where to start\n(.*?)(?=^## |\Z)', home, re.M | re.S)
    order = re.findall(r'^\d+\. \[[^\]\n]+\]\(([^\s)]+)\)', section[1], re.M) if section else []
    if len(order) != len(PAGES) - 1 or set(order) != set(PAGES) - {'Home.md'}:
        raise ValueError('Home "Where to start" must list every other APL page exactly once')
    # GitHub's Pages index sorts page names; number them using Home's reading order.
    slugs = {'Home.md': 'Home'}
    slugs.update({name: f'APL-{index:02d}-{pathlib.Path(name).stem}'
                  for index, name in enumerate(order, 1)})
    titles = {name: (docs / name).read_text(encoding='utf-8').splitlines()[0][2:] for name in PAGES}
    navigation = ' · '.join(f'[{titles[name]}]({PUBLIC_URL}/wiki/{slug})' for name, slug in slugs.items())
    footer = (f'\n---\n\nFellersim {version} · '
              f'[Public source revision {revision[:12]}]({PUBLIC_URL}/commit/{revision})\n\n'
              f'{PUBLICATION_NOTICE}\n')
    rendered = {}
    for name, slug in slugs.items():
        page = docs / name
        text = page.read_text(encoding='utf-8')
        # Example expectations are test inputs, not part of the reader's guide.
        text = re.sub(r'^<!-- (?:apl-test|query-examples) .* -->\n', '', text, flags=re.M)

        def rewrite(match):
            destination = local_target(page, match[2], root)
            if destination is None:
                return match[0]
            target, anchor = destination
            if target.parent == docs and target.name in PAGES:
                href = f'{PUBLIC_URL}/wiki/{slugs[target.name]}'
            else:
                kind = 'tree' if target.is_dir() else 'blob'
                href = f'{PUBLIC_URL}/{kind}/{revision}/{target.relative_to(root).as_posix()}'
            if anchor:
                href += f'#{anchor}'
            return f'[{match[1]}]({href})'

        text = LINK.sub(rewrite, text)
        title, body = text.split('\n', 1)
        rendered[f'{slug}.md'] = f'{title}\n\n{navigation}\n{body.rstrip()}\n{footer}'
    return rendered
