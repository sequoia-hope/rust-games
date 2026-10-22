#!/usr/bin/env python3
"""The kit's docs as a browsable site, for `./serve.sh`.

    tools/docs.py <dir>   render every tracked .md under crates/engine into <dir>

The kit is a library, so it has no web build to serve the way a game does —
what it has is the documentation the games and the outside consumers read:
`README.md`, the `CHANGELOG.md` every breaking change goes in, `docs/PLAN.md`
and `docs/DECISIONS.md`, each crate's README, each patch's PATCHED.md, the
skills and the template. This renders all of it with the links between the
documents intact, so `proj up game-engine` opens onto the kit itself instead
of a status page that says nothing.

The file list comes from `git ls-files`, not a list kept here: a doc added
under `crates/engine/` appears on the next build, and nothing generated,
vendored or ignored sneaks in.
"""
import html
import re
import subprocess
import sys
from pathlib import Path

try:
    import markdown
except ModuleNotFoundError:
    sys.exit(
        "docs.py: needs the Python markdown package (pip install --user markdown)"
    )

ROOT = Path(__file__).resolve().parent.parent

# Which section of the sidebar a document lands in, by the folder it is in.
# Longest matching prefix wins, so docs/design beats docs. A folder not named
# here (a new crate, say) becomes its own section under its own name, which is
# why adding one needs no change in this file.
SECTIONS = [
    ("docs/design", "Design notes"),
    ("docs/upstream", "Upstream drafts"),
    ("docs", "Plan & decisions"),
    ("patches", "Patches"),
    ("skills", "Skills"),
    ("template", "New-game template"),
    ("js", "Browser tools"),
]
# The order the sections appear in. A `pk_*` folder is one of the kit's crates
# (the root CLAUDE.md's naming rule), so a crate added later lands in "Crates"
# on its own; any other unlisted folder becomes a section under its own name
# and is appended after these.
SECTION_ORDER = [
    "The kit",
    "Crates",
    "Plan & decisions",
    "Design notes",
    "Patches",
    "Upstream drafts",
    "Browser tools",
    "Skills",
    "New-game template",
]
# Within "The kit": the overview first, then what changed, then the rules.
ROOT_ORDER = ["README.md", "CHANGELOG.md", "HANDOFF.md", "CLAUDE.md"]


def docs():
    """Every tracked markdown file under crates/engine, as repo-relative paths."""
    out = subprocess.run(
        ["git", "ls-files", "-z", "*.md"],
        cwd=ROOT,
        check=True,
        capture_output=True,
        text=True,
    ).stdout
    return sorted(p for p in out.split("\0") if p)


def section(path):
    folder = str(Path(path).parent).replace("\\", "/")
    if folder == ".":
        return "The kit"
    for prefix, title in SECTIONS:
        if folder == prefix or folder.startswith(prefix + "/"):
            return title
    top = folder.split("/")[0]
    return "Crates" if top.startswith("pk_") else top


def title_of(path, text):
    """A document's own `# ` heading, falling back to its path."""
    for line in text.splitlines():
        if line.startswith("# "):
            return line[2:].strip()
    return path


def sort_key(path):
    """Overview before changelog before rules; elsewhere alphabetical, but a
    folder's README or PATCHED.md leads it (it is that folder's front page)."""
    name = Path(path).name
    if str(Path(path).parent) == ".":
        lead = ROOT_ORDER.index(name) if name in ROOT_ORDER else len(ROOT_ORDER)
    else:
        lead = 0 if name in ("README.md", "SKILL.md", "PATCHED.md") else 1
    return (lead, path)


# A relative link to a .md file, which becomes the .html we wrote for it.
# Absolute and external links are left alone, and so is the #fragment.
MD_LINK = re.compile(r'(href=")(?!https?:|/|#)([^"#]*?)\.md(#[^"]*)?(")')


def page(body, nav, title, depth):
    """One rendered document: the sidebar, then the document."""
    up = "../" * depth
    return f"""<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>{html.escape(title)} — Playground Kit</title>
<link rel="stylesheet" href="{up}docs.css">
</head>
<body>
<a class="skip" href="#doc">Skip to the document</a>
<nav aria-label="The kit's documents">
<a class="home" href="{up}index.html">Playground Kit</a>
{nav}
</nav>
<main id="doc">
{body}
</main>
</body>
</html>
"""


def nav_html(entries, current, depth):
    """The sidebar, the same on every page, with the current document marked."""
    up = "../" * depth
    out = []
    for title in SECTION_ORDER + sorted(
        {e["section"] for e in entries} - set(SECTION_ORDER)
    ):
        inside = [e for e in entries if e["section"] == title]
        if not inside:
            continue
        out.append(f"<h2>{html.escape(title)}</h2>\n<ul>")
        for e in inside:
            here = ' aria-current="page"' if e["path"] == current else ""
            out.append(
                f'<li><a href="{up}{html.escape(e["href"])}"{here}>'
                f'{html.escape(e["title"])}</a></li>'
            )
        out.append("</ul>")
    return "\n".join(out)


def build(out):
    out = Path(out)
    out.mkdir(parents=True, exist_ok=True)
    md = markdown.Markdown(
        extensions=["fenced_code", "codehilite", "tables", "toc", "sane_lists"],
        extension_configs={"codehilite": {"guess_lang": False}},
    )

    entries = []
    for path in docs():
        text = (ROOT / path).read_text()
        # Every document keeps its own path with .html for .md, so a relative
        # link from one to another still resolves. The root README is also
        # copied to index.html below, for the address with no page in it.
        entries.append(
            {
                "path": path,
                "href": path[: -len(".md")] + ".html",
                "title": "Overview" if path == "README.md" else title_of(path, text),
                "section": section(path),
                "text": text,
            }
        )
    entries.sort(key=lambda e: sort_key(e["path"]))

    for e in entries:
        md.reset()
        body = MD_LINK.sub(r"\1\2.html\3\4", md.convert(e["text"]))
        depth = len(Path(e["href"]).parts) - 1
        dest = out / e["href"]
        dest.parent.mkdir(parents=True, exist_ok=True)
        dest.write_text(page(body, nav_html(entries, e["path"], depth), e["title"], depth))

    # The front page. README.html is at the root too, so the same file serves
    # as index.html unchanged — no second render, no rewritten links.
    (out / "index.html").write_text((out / "README.html").read_text())
    (out / "docs.css").write_text(STYLE)
    print(f"docs.py: {len(entries)} documents into {out}")


# The hub dashboard's palette, so the kit's page and the project list read as
# one system when you tab between them.
STYLE = """
:root {
  --bg: #f6f7f9; --card: #ffffff; --line: #e2e5ea; --fg: #14171c;
  --muted: #5f6875; --accent: #2f6feb; --chip: #eef1f5;
}
@media (prefers-color-scheme: dark) {
  :root:not([data-theme="light"]) {
    --bg: #0e1116; --card: #171b22; --line: #262c36; --fg: #e6e9ee;
    --muted: #8b94a3; --accent: #5b9bff; --chip: #222833;
  }
}
* { box-sizing: border-box; }
body {
  margin: 0; background: var(--bg); color: var(--fg);
  font: 16px/1.65 ui-sans-serif, system-ui, -apple-system, "Segoe UI", Roboto, sans-serif;
  display: grid; grid-template-columns: 17rem minmax(0, 1fr); align-items: start;
}
.skip { position: absolute; left: -9999px; }
.skip:focus { left: 1rem; top: .5rem; background: var(--card); padding: .5rem .8rem; z-index: 2; }
nav {
  position: sticky; top: 0; max-height: 100vh; overflow-y: auto;
  padding: 1.5rem 1rem 3rem; border-right: 1px solid var(--line);
  font-size: .9rem;
}
nav .home { display: block; font-weight: 700; font-size: 1.05rem; margin-bottom: 1.25rem; color: var(--fg); text-decoration: none; }
nav h2 {
  font-size: .72rem; text-transform: uppercase; letter-spacing: .08em;
  color: var(--muted); margin: 1.4rem 0 .4rem; font-weight: 650;
}
nav ul { list-style: none; margin: 0; padding: 0; display: grid; gap: .1rem; }
nav a { display: block; padding: .25rem .5rem; border-radius: 6px; color: var(--muted); text-decoration: none; }
nav a:hover { background: var(--chip); color: var(--fg); }
nav a[aria-current="page"] { background: var(--chip); color: var(--accent); font-weight: 600; }
main { max-width: 52rem; padding: 2.5rem 2rem 5rem; min-width: 0; }
h1, h2, h3, h4 { line-height: 1.25; margin: 2rem 0 .75rem; }
h1 { margin-top: 0; font-size: 1.9rem; letter-spacing: -.02em; }
h2 { font-size: 1.35rem; padding-bottom: .3rem; border-bottom: 1px solid var(--line); }
h3 { font-size: 1.1rem; }
a { color: var(--accent); }
code, pre { font-family: ui-monospace, SFMono-Regular, Menlo, monospace; font-size: .875em; }
code { background: var(--chip); padding: .15em .4em; border-radius: 4px; }
pre {
  background: var(--card); border: 1px solid var(--line); border-radius: 10px;
  padding: .9rem 1rem; overflow-x: auto;
}
pre code { background: none; padding: 0; }
table { border-collapse: collapse; width: 100%; margin: 1.25rem 0; display: block; overflow-x: auto; }
th, td { border: 1px solid var(--line); padding: .45rem .7rem; text-align: left; vertical-align: top; }
th { background: var(--chip); font-size: .9rem; }
blockquote { margin: 1.25rem 0; padding: .1rem 1rem; border-left: 3px solid var(--line); color: var(--muted); }
hr { border: 0; border-top: 1px solid var(--line); margin: 2rem 0; }
img { max-width: 100%; }
@media (max-width: 52rem) {
  body { grid-template-columns: 1fr; }
  nav { position: static; max-height: none; border-right: 0; border-bottom: 1px solid var(--line); }
  main { padding: 1.75rem 1.1rem 4rem; }
}
"""


def main(args):
    if len(args) != 1:
        sys.exit(__doc__)
    build(args[0])


if __name__ == "__main__":
    main(sys.argv[1:])
