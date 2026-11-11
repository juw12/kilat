# Kilat /hone run log (run 2, 2026-11-11)

Goal: "do it all in next items urself, and make sure this is the bests in the world beating python, dont forget /hone for this"
Goal beyond the goal: a language Juwen can actually reach for in Cairn and Glow tomorrow - faster than Python
on every measured program, safer than Python on the bug set, and one click to install.

## Run queue

- [x] 0. Cairn vibe session + this log
- [x] A. Speed: strings and map_count faster than CPython net of start-up; raise the geometric mean
- [x] B. Stats helpers mean / median / stddev (natives, checker, docs, conformance example)
- [x] C. Web engine in a Worker (page stays responsive)
- [ ] D. GitHub release with prebuilt kilat.exe, version bump, install.cmd
- [ ] E. Pull request: list the plugin in Cairn's catalogue
- [ ] F. Try the plugin inside the real Cairn app
- [ ] G. Glow: playground that runs model-written Kilat in the sandbox (pull request)
- [ ] H. /hone loop (frozen rubric, baseline v0 = 94f3e63, council judge, keep-best)
- [ ] I. Sync: repo, profile, portfolio card, landing page, Obsidian, precedent log, vibe end
- [ ] J. Recapture vow-card.jpg

## Assumptions

- Assumed: "do it all in next items" = every item under "Next" in the last report and the Kilat lines in the vault queue.
- Assumed: "best in the world" cannot be claimed without measurements; the run reports measured wins and losses against CPython only.
- Assumed: Cairn and Glow changes ship as pull requests from a worktree, never a push to the default branch.

- Assumed: the pool allocator is native only; the WebAssembly build keeps the standard allocator (it grew the file by 170 KB for no gain).
- Assumed: worker mode is opt-in. In headless Edge the same program ran 1.5 to 2.5 times slower in a worker, so the playground and the Cairn plugin stay in page mode.
- Assumed: stddev is the sample form; the three stats helpers refuse an empty list at run time.
- Assumed: the command-line program may skip freeing memory at exit (the process ends right after).
- Measured after A (judge, best of 5): geometric mean 3.06 whole process, 2.62 net of start-up (baseline 2.82 and 2.43). Net of start-up: map_count 1.06, strings 1.04 (baseline 0.80 and 0.71). The 5 times target set before measuring was not reached.

## Rounds

| round | change | gates | judge score | pairwise win | kept? | note |
|---|---|---|---|---|---|---|
