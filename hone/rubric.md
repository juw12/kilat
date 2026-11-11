# Rubric kilat-r2 (frozen 2026-11-11 before any change in this run)

Artifact: the Kilat language repo (juw12/kilat). Baseline v0 = commit 94f3e63.

## Hard gates (tool-checked, all must pass)

G1. `python judge.py` reports every check passing (conformance, sandbox, README claims, WASM agreement, browser checks).
G2. Every benchmark prints the same text in Kilat and CPython.
G3. Bug corpus: Kilat refuses 20/20 before running.
G4. Native program <= 1.5 MB, Cairn plugin main.js <= 400,000 characters.
G5. No test, benchmark or bug-corpus file deleted or weakened versus v0 (file counts and assertions only grow).

## Soft criteria (0-10, anchors at 4 / 7 / 10)

S1. Speed against CPython, weight 0.30
  4 = at least one benchmark slower than CPython net of start-up
  7 = every benchmark faster net of start-up and geometric mean >= 3x
  10 = every benchmark >= 2x and geometric mean >= 5x
S2. Safety evidence, weight 0.20
  4 = safety claims without tests
  7 = bug corpus 20/20, conformance suite, sandbox limits tested
  10 = plus differential or fuzz testing and a bug set not written by the author
S3. Usable today by an engineer who is not a programmer, weight 0.25
  4 = must build from source; thin docs
  7 = documented install, worked examples, everyday helpers (statistics), clear error messages
  10 = prebuilt download verified end to end, playground stays responsive, errors explain the fix
S4. Fits Cairn and Glow, weight 0.15
  4 = plugin file exists, never loaded by the real app
  7 = plugin loaded by Cairn's real loader, catalogue entry proposed
  10 = merged and used inside Cairn and Glow with a verified demo
S5. Honest, reproducible claims, weight 0.10
  4 = numbers that cannot be reproduced
  7 = every README number produced by judge.py, losses stated
  10 = plus independent reproduction

Loop constants: MAX_ROUNDS 5, MIN_GAIN 0.3, PLATEAU 2, GATE_TRIES 3, DONE_SCORE 8.5.
Keep a candidate only if score >= best + 0.3 and it wins the blind pairwise.
