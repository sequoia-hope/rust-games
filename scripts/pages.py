#!/usr/bin/env python3
"""The public site's list (site/games.toml), for the Pages workflow.

    scripts/pages.py matrix       the entries as JSON ({name, path}), for the build matrix
    scripts/pages.py index <dir>  write <dir>/index.html, the front page
    scripts/pages.py build <dir>  the whole site into <dir>: every entry, then the front page
"""
import html
import json
import subprocess
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
        if ("path" in g) == ("href" in g):
            sys.exit(f"pages.py: {g['name']} needs a path or an href, not both (site/games.toml)")
        if "path" in g and not (ROOT / g["path"] / "pages.sh").is_file():
            sys.exit(f"pages.py: {g['path']}/pages.sh is missing (site/games.toml)")
    return listed


def built():
    """The entries with a folder to build; the rest only link into them."""
    return [g for g in games() if "path" in g]


def index(out):
    items = "\n".join(
        f'<li><a class="game" href="{html.escape(g.get("href", g["name"] + "/"))}">'
        f'<strong>{html.escape(g["title"])}</strong>'
        f'<span>{html.escape(g["blurb"])}</span></a></li>'
        for g in games()
    )
    page = (ROOT / "site/index.html").read_text()
    if MARK not in page:
        sys.exit("pages.py: site/index.html has lost its GAMES marker")
    Path(out, "index.html").write_text(page.replace(MARK, items))


def build(out):
    """The whole site into <out>, the way the workflow assembles it.

    The workflow gives each entry a runner of its own and merges the pieces;
    here they run one after another into the same folder. Same `pages.sh`
    calls and the same layout either way, so what `./serve.sh` shows locally
    is what Pages deploys — which is the point of building it rather than
    serving a page of links to the games' own servers.
    """
    # Absolute, as the workflow's $RUNNER_TEMP path is: a `pages.sh` changes
    # directory to its own folder, so a relative output path would be written
    # relative to the game instead of to here.
    out = Path(out).resolve()
    out.mkdir(parents=True, exist_ok=True)
    for g in built():
        subprocess.run(
            [str(ROOT / g["path"] / "pages.sh"), str(out / g["name"])], check=True
        )
    index(out)


def main(args):
    if args == ["matrix"]:
        print(json.dumps([{"name": g["name"], "path": g["path"]} for g in built()]))
    elif len(args) == 2 and args[0] == "index":
        index(args[1])
    elif len(args) == 2 and args[0] == "build":
        build(args[1])
    else:
        sys.exit(__doc__)


if __name__ == "__main__":
    main(sys.argv[1:])
