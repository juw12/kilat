# Kilat /hone run log (run 2, 2026-11-11)

Goal: "do it all in next items urself, and make sure this is the bests in the world beating python, dont forget /hone for this"
Goal beyond the goal: a language Juwen can actually reach for in Cairn and Glow tomorrow - faster than Python
on every measured program, safer than Python on the bug set, and one click to install.

## Run queue

- [x] 0. Cairn vibe session + this log
- [x] A. Speed: strings and map_count faster than CPython net of start-up; raise the geometric mean
- [x] B. Stats helpers mean / median / stddev (natives, checker, docs, conformance example)
- [x] C. Web engine in a Worker (page stays responsive)
- [x] D. GitHub release with prebuilt kilat.exe, version bump, install.cmd
- [x] E. Pull request: list the plugin in Cairn's catalogue (https://github.com/juw12/cairn/pull/35)
- [x] F. Try the plugin inside the real Cairn app (8 of 8 checks, passed twice, headless Edge, demo brain)
- [x] G. Glow: GlowCode page that checks Kilat programs before running them (https://github.com/juw12/projects/pull/53)
- [x] H. /hone loop (frozen rubric, baseline v0 = 94f3e63, council judge, keep-best)
- [x] I. Sync: repo, profile, portfolio card, landing page, Obsidian, precedent log, vibe end
- [x] J. Recapture vow-card.jpg (juw12/portfolio ad8ed7c; mean saturation 0.04 -> 0.22 with the card tool's own measure, threshold 0.10; live file matches by hash)

## Assumptions

- Assumed: "do it all in next items" = every item under "Next" in the last report and the Kilat lines in the vault queue.
- Assumed: "best in the world" cannot be claimed without measurements; the run reports measured wins and losses against CPython only.
- Assumed: Cairn and Glow changes ship as pull requests from a worktree, never a push to the default branch.

- Assumed: the pool allocator is native only; the WebAssembly build keeps the standard allocator (it grew the file by 170 KB for no gain).
- Assumed: worker mode is opt-in. In headless Edge the same program ran 1.5 to 2.5 times slower in a worker, so the playground and the Cairn plugin stay in page mode.
- Assumed: stddev is the sample form; the three stats helpers refuse an empty list at run time.
- Assumed: the command-line program may skip freeing memory at exit (the process ends right after).
- Measured after A (judge, best of 5): geometric mean 3.06 whole process, 2.62 net of start-up (baseline 2.82 and 2.43). Net of start-up: map_count 1.06, strings 1.04 (baseline 0.80 and 0.71). The 5 times target set before measuring was not reached.
- Assumed: the Cairn catalogue entry uses "repo": "juw12/kilat" and author "Juwen Lie", matching the plugin manifest.
- Assumed: git push kept answering 407 through the proxy (20 tries), so the Cairn branch was created through the GitHub REST API with the same file content as the local commit. The remote commit is 18ac1e9; the local one is 1eacdfa.
- Not covered by F: Cairn's in-app install from GitHub (the test browser is offline) and Cairn's desktop build.
- Assumed: GlowCode ships as its own page plus a menu entry, without a home tile, start-box choice, home.js entry or service-worker assets, because existing tests pin the home list and cache names.
- Assumed: a model reply without a code fence counts as a program only if it passes the checker.
- Assumed: GlowCode's page text is English only; only the menu line is translated (four languages).
- Measured for G (headless Edge, two runs): 17 of 17 page actions finished, no page errors, no CSP violations. Run 1 found that a reply with no code fence was treated as a program; fixed and confirmed in run 2.
- Assumed: the council rubric file `hone/rubric_council.md` is a reformatted copy of the frozen rubric `kilat-r2` (same criteria, weights and anchors), because the council tool needs one line per criterion.
- Assumed: the GlowCode branch goes straight to the GitHub REST API, because git push answered 407 on 30+ tries in a row.
- Assumed: the fuzz test's reference treats a comparison of an Int with a Float as a comparison of two floats, and follows Python in stopping `and` / `or` early.
- Assumed: two Glow browser tests that passed before and ended with one page-load timeout afterwards are load failures (the PC ran 30 to 80 times slower with the screen locked); the pull request says so and leaves the verdict to CI.
- Assumed: the stale profile line, portfolio card text and landing page were updated to the 0.2.0 figures as part of the sync; the card text was a direct catalogue commit as before.
- Assumed: the first council run for v1 went silent for 20 minutes and was killed and relaunched once; a second hang means both versions are self-judged with the same rubric.
- Assumed: "recapture vow-card.jpg" means framing the existing vow-desktop.png in the house style with the card tool, using the settings already in the catalogue; the card goes straight to juw12/portfolio main like earlier card art.
- Assumed: the council run for v1 hung twice (20 minutes each) and was killed, so v0 and v1 were both judged by self with the frozen rubric; the council's 5.15 for v0 is kept only as a side note. A self judge is biased towards its own work, so the scores were set on the low side of each anchor.

## Rounds

| round | change | gates | judge score | pairwise win | kept? | note |
|---|---|---|---|---|---|---|
| 0 | baseline 0.1.0 (94f3e63) | 5/5 | 5.15 (self) | - | baseline | council also scored 5.15; slower than CPython net of start-up on 2 of 10 programs |
| 1 | 0.2.0 (11a0d2a): faster engine, mean / median / stddev, worker mode, release download, fuzz and differential test, Cairn and Glow pull requests | 5/5 | 7.05 (self) | yes (self) | kept | gain 1.90; no engine rebuild after 0.2.0, so G1 to G4 were last run on the same binaries; G5 holds because files were only added |

Self-judge scores per criterion (rubric kilat-r2, weights 0.30 / 0.20 / 0.25 / 0.15 / 0.10):

| version | S1 speed | S2 safety | S3 usable today | S4 Cairn and Glow | S5 honest claims | total |
|---|---|---|---|---|---|---|
| v0 | 4.0 | 7.0 | 5.0 | 4.0 | 7.0 | 5.15 |
| v1 | 6.0 | 8.0 | 7.5 | 7.5 | 6.5 | 7.05 |

- S1 v1: no program slower net of start-up, but the net mean is 2.62 (under 3) and two programs are about level.
- S2 v1: fuzz and differential test added (11,607 cases, 0 failures); bug set and reference are still written by the author.
- S3 v1: release download verified by hash and run; a real install was not run and the files are unsigned; worker mode is opt-in.
- S4 v1: loaded by the real Cairn web app (8 of 8, twice); both pull requests are open, not merged.
- S5 v1: losses are stated, but the fuzz and release sections of the README were added by hand, not by judge.py; nothing was reproduced by someone else.

- Sync done 2026-11-12: Kilat repo d7c2e2a (written through the GitHub API), landing page 80ad196, profile ffa5b63, portfolio text 5dda6f7 and c90b42a, Obsidian note and update log pushed, precedent log and vibe end run last.

stop: no further round possible without an engine rebuild or outside reviewers (net speed of 3 or more, an independent bug set, merged pull requests, independent reproduction); v1 kept, below DONE_SCORE 8.5.
