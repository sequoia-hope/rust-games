# Skills

Claude Code skills for the crafts a Playground Kit game is made of. Each
directory holds one `SKILL.md`: a procedure for an agent, with the files to
read first, the commands in order, the gates that prove it worked, the
common mistakes, and the owner's conventions (never an ad-hoc server, ports
from `proj port`, a DECISIONS entry, push to `main`).

Until the carve is done, most skills point at Midnight Playground's files
(`~/Software/midnight-playground`) and say where each piece lands in the
kit; update the paths as the phases land (`client-shell` is a draft until
phases 1 and 2 do).

## Install

For every project on this machine, symlink each directory into the user's
skills directory:

```bash
mkdir -p ~/.claude/skills
for d in ~/Software/game-engine/skills/*/; do ln -sfn "$d" ~/.claude/skills/"$(basename "$d")"; done
```

For one project only, symlink them into that project's `.claude/skills/`
instead (the same loop with the target changed). Claude Code picks a skill
from its `description`, or by `/<name>`.

## The skills

- `voice-cast`: a spoken-word character: the sheet, three voice candidates, `design` and `render`, Whisper check, the audition page, the director's rules.
- `world-making`: a physical world from real geography: bbox and origin, `build.py`, `plan.json`, `report` and `corridor`, `ground.py`, planting by land cover, events at places, the viewer as the shared table.
- `music-genre`: a genre grammar on the composer: vocabulary first, the lab as oracle, goldens, a station's weights, the Radio page's Keep and Reject.
- `engine-sound`: an exhaust preset: firing order, headers, muffler sections, a stock and aftermarket pair, the lab, `Response::GAME`, the per-sample cost.
- `web-perf`: `xtask web/size/prof`, frame times on desktop, phone and throttled CPU, profiles by Bevy system, GL calls and GPU time, pixel diffs, the debug overlay, the budgets.
- `lockstep-sim`: a deterministic `Simulation`: the purity and exact-math rules, named streams, events not side effects, the hash and trace, recording and replay, fuzz.
- `level-viewer`: god mode: cameras, links that carry a pose, groups, the readout, annotating a draft with the owner.
- `decisions-log`: DECISIONS and DEVIATIONS, the commit shape, push to `main`, parallel work on disjoint files, the port rule.
- `client-shell` (draft): a new game on the kit: the template, `Params`, `Store`, the `Action` enum, `TouchLayout`, `ScreenHost`, `__game`, serving through `proj`.
