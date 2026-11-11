# Rubric kilat-r2 (council format; same criteria, weights and anchors as rubric.md, frozen 2026-11-11)

Artifact: the Kilat language repository, shown as its README and RESULTS files.
Hard gates are checked by tools, not by the judges.

- S1 Speed against CPython (w=0.30): 4 = at least one benchmark slower than CPython net of start-up, 7 = every benchmark faster net of start-up and geometric mean >= 3x, 10 = every benchmark >= 2x and geometric mean >= 5x
- S2 Safety evidence (w=0.20): 4 = safety claims without tests, 7 = bug corpus 20/20 plus conformance suite plus sandbox limits tested, 10 = plus differential or fuzz testing and a bug set not written by the author
- S3 Usable today by an engineer who is not a programmer (w=0.25): 4 = must build from source and thin docs, 7 = documented install plus worked examples plus everyday helpers (statistics) plus clear error messages, 10 = prebuilt download verified end to end and playground stays responsive and errors explain the fix
- S4 Fits Cairn and Glow (w=0.15): 4 = plugin file exists but never loaded by the real app, 7 = plugin loaded by Cairn's real loader and catalogue entry proposed, 10 = merged and used inside Cairn and Glow with a verified demo
- S5 Honest and reproducible claims (w=0.10): 4 = numbers that cannot be reproduced, 7 = every README number produced by judge.py and losses stated, 10 = plus independent reproduction
