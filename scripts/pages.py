#!/usr/bin/env python3
"""The public site's list (site/games.toml), for the Pages workflow.

    scripts/pages.py matrix       the entries as JSON ({name, path}), for the build matrix
    scripts/pages.py index <dir>  write <dir>/index.html, the front page
"""
import html
import json
import sys
import tomllib
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
MARK = "<!-- GAMES: written by scripts/pages.py from site/games.toml -->"


def games():
    listed = tomllib.loads((ROOT / "site/games.toml").read_text())["game"]
    names = [g["name"] for g in listed]
    if len(set(names)) != len(names):
        sys.exit("pages.py: two entries share a name (site/games.toml)")
    for g in listed:
        if not (ROOT / g["path"] / "pages.sh").is_file():
            sys.exit(f"pages.py: {g['path']}/pages.sh is missing (site/games.toml)")
    return listed


def index(out):
    items = "\n".join(
        f'<li><a class="game" href="{html.escape(g["name"])}/">'
        f'<strong>{html.escape(g["title"])}</strong>'
        f'<span>{html.escape(g["blurb"])}</span></a></li>'
        for g in games()
    )
    page = (ROOT / "site/index.html").read_text()
    if MARK not in page:
        sys.exit("pages.py: site/index.html has lost its GAMES marker")
    Path(out, "index.html").write_text(page.replace(MARK, items))


def main(args):
    if args == ["matrix"]:
        print(json.dumps([{"name": g["name"], "path": g["path"]} for g in games()]))
    elif len(args) == 2 and args[0] == "index":
        index(args[1])
    else:
        sys.exit(__doc__)


if __name__ == "__main__":
    main(sys.argv[1:])
