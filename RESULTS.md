# Measured results

Written by `python judge.py` on 2026-11-13. Kilat kilat 0.2.2, CPython 3.13.2, Windows 10, Intel(R) Core(TM) Ultra 7 258V.

## Speed against CPython

Best of 5 runs each. CPU time is the whole process, measured with `QueryProcessCycleTime`, with both languages kept on the fastest cores.
"Net" takes each language's start-up time off first, which is the stricter figure for Kilat.
Both languages run the same plain algorithm; Python with NumPy would be faster on some of these.
Wall-clock time on the development PC is dominated by how long a process takes to start, so it is shown last.

| Program | Kilat CPU s | CPython CPU s | Times faster | Net of start-up | Kilat wall s | CPython wall s |
|---|---|---|---|---|---|---|
| `collatz` | 0.312 | 0.991 | 3.18 | 2.98 | 0.456 | 2.572 |
| `fib` | 0.110 | 0.387 | 3.51 | 2.95 | 0.247 | 0.574 |
| `loop_sum` | 0.177 | 0.691 | 3.91 | 3.58 | 0.252 | 1.206 |
| `mandelbrot` | 0.206 | 0.865 | 4.19 | 3.92 | 0.322 | 1.866 |
| `map_count` | 0.260 | 0.349 | 1.34 | 1.06 | 0.483 | 0.787 |
| `matmul` | 0.110 | 0.300 | 2.73 | 2.12 | 0.216 | 0.591 |
| `nbody` | 0.203 | 0.758 | 3.73 | 3.44 | 0.336 | 1.427 |
| `sieve` | 0.123 | 0.313 | 2.55 | 2.00 | 0.261 | 0.409 |
| `sort` | 0.093 | 0.638 | 6.88 | 6.46 | 0.149 | 0.679 |
| `strings` | 0.137 | 0.198 | 1.45 | 0.90 | 0.173 | 0.249 |

- Geometric mean: 3.02 times faster for the whole process, 2.54 net of start-up.
- Net of start-up Kilat is faster on 9 of 10 programs, slower on `strings`.
- The target set before measuring was 5 times: not met.
- The two languages print the same text on every program: yes.
- Start-up: Kilat 6 ms of CPU (18 ms wall), CPython 81 ms (113 ms wall).

## Sizes

| File | Bytes |
|---|---|
| kilat program | 1,282,048 |
| kilat library | 1,728,512 |
| docs/kilat.wasm | 728,823 |
| docs/kilat.js | 16,007 |
| main.js (Cairn plugin) | 365,625 |

The kilat program needs only these libraries: api-ms-win-core-synch-l1-2-0.dll, bcryptprimitives.dll, kernel32.dll, msvcrt.dll, ntdll.dll.

The kilat library needs only these libraries: api-ms-win-core-synch-l1-2-0.dll, bcryptprimitives.dll, kernel32.dll, msvcrt.dll, ntdll.dll, userenv.dll, ws2_32.dll.
All of them are part of Windows: yes.
`docs/kilat.wasm` imports 0 things from its host.

## Mistakes caught before the program runs

20 small programs in `bugs/`, each with one mistake, a fixed version and a Python twin.
The corpus was written by the author of Kilat, so read it as a list of what Kilat checks, not as a survey.

| Kind of mistake | Programs | Kilat refuses | mypy --strict reports | Python refuses before running |
|---|---|---|---|---|
| units | 5 | 5 | 0 | 0 |
| names | 4 | 4 | 4 | 0 |
| types | 6 | 6 | 6 | 0 |
| safety | 5 | 5 | 4 | 0 |
| **all** | 20 | 20 | 14 | 0 |

- Fixed versions that run: 20 of 20.
- Python twins when run: 10 stop with an error, 10 finish with a wrong or meaningless answer.
- Python can check units while a program runs with a library such as pint; that is not measured here.

## Checks

| Check | Passed |
|---|---|
| Conformance programs print their recorded output | 6 of 6 |
| The library prints what the program prints | 11 of 11 |
| Limits and file access on the command line | 6 of 6 |
| Behaviours quoted in the README | 6 of 6 |
| WebAssembly build prints what the program prints | 11 of 11 |
| Engine in a browser | 79 of 79 |
| Playground and Cairn plugin pages | 44 of 44 |

| Behaviour | Kilat | Python |
|---|---|---|
| `round_to(2.675, 2)` | 2.68 | 2.67 |
| `"{1.005:.2}"` | 1.01 | 1.00 |
| `0.0 / 0.0` | runtime error: division by zero | ZeroDivisionError |
| `sqrt(-1.0)` | runtime error: sqrt needs a number that is not negative | ValueError |
| `9223372036854775807 + 1` | runtime error: integer overflow | 9223372036854775808 |
| `a misspelt name` | did you mean `length`? | NameError when the line runs |
| `1e308 * 10.0` | inf | |
| `infinity - infinity` | nan | |

Times seen in the browser checks, in milliseconds unless the name says otherwise:

- `playgroundLimitMs`: 135
- `playgroundLongestFreezeMs`: 97
- `pluginCharacters`: 365625
- `pluginFirstResultMs`: 104
- `pluginLimitMs`: 57
- `pluginLongTextLimitMs`: 4117
- `pluginLongestFreezeMs`: 200
- `pluginReadyMs`: 4514
- `pluginRedrawMs`: 16
- `pluginTimeLimitMs`: 10070

## About this speed run (0.2.2)

Added by hand.

- The PC was in use for other work during the run. Wall times were 1.3 to 2.6 times the CPU times,
  and CPython's start-up measured 81 ms of CPU against 35 ms in the quiet run for 0.2.1.
- The "without start-up" column subtracts that start-up estimate, so it moves when the estimate moves.
  `strings` shows 0.90 here and showed 1.12 in the 0.2.1 run.
- 0.2.1, on a quiet PC: 3.11 times for the whole process, 2.79 net of start-up, 10 of 10 faster.
- 0.2.2 changed only the checker (three messages), not the part that runs programs.
- `judge.py` was run twice for 0.2.2. The first run was started by mistake with a Python that has no
  mypy, so its bug table was incomplete; it measured 2.96 and 2.67, with `strings` at 1.01 net (level). The
  numbers above are from the second run, with CPython 3.13.2 and mypy. Neither run was repeated to
  get a better number.

## Fuzz and differential tests

Added by hand from the output of `python tests/fuzz_test.py` (seed 20261112, 200,000 fuel and
16 MB per run). `judge.py` does not run this test, so rerunning `judge.py` will drop this section.
The test and its Python reference were written by the author of Kilat.

| Kind of case | Cases | What must hold |
|---|---|---|
| Deep or runaway programs | 46 | ends as a result or an ordinary error |
| Corpus files, damaged copies, random tokens | 6,061 | no crash, no hang, `check` agrees with `run` |
| Random Int, Float and Bool expressions | 4,000 | same answer as the Python reference |
| `mean`, `median`, `stddev` | 600 | same answer as Python's `statistics` |
| Sums of lengths, masses and times | 600 | same answer as the reference |
| Sums of different dimensions | 300 | refused before running |
| Total | 11,607 | 0 failures |

- How the runs ended: 5,670 finished, 5,552 refused before running, 314 run-time errors, 67 fuel stops,
  4 memory stops.
- 299 of the 4,000 expressions overflow or divide by zero; each was refused at run time as expected.
- Worker restarts: 0. The run for 0.2.2 took 373 s with 4 worker processes on a PC that was in use
  for other work; the longest single case took 2.5 s (the hang limit is 60 s). The run for 0.2.1,
  on a quiet PC, took 10 s; the run for 0.2.0, on a heavily loaded PC, took 591 s.
- Before the run the harness caught a planted crash and a planted hang (`--selftest`).
- One fault was found while writing the test: in 0.2.0 a fuel stop could report the wrong place
  (`at <prelude>:309 in to_celsius` for an empty `while true { }` loop). It is fixed in 0.2.1, and
  `python tests/fuel_location_test.py` checks four cases (4 of 4).
- `python tests/unit_hints_test.py` checks the three checker messages added in 0.2.2 (20 of 20).

## Release download check (0.2.1)

Added by hand from a separate script that downloads the 0.2.1 release files.

| Check | Result |
|---|---|
| `kilat.exe` from the release page | 1,271,808 bytes, SHA-256 same as the local build |
| `kilat-windows-x64.zip` | 10 files; the `kilat.exe` inside has the same SHA-256 |
| `kilat --version` on the download | `kilat 0.2.1` |
| The five examples | each prints the same lines as the local build |
| `print(2 m + 3 kg)` | refused: unit mismatch, cannot add m and kg |

SHA-256 of `kilat.exe`: `73b1173f17c6a447e776453405c11f2e5587086c17a05efb94434d3e552eb4d3`

`install.cmd` was only run with `/dry-run`. The files are unsigned.

## Reruns on GitHub's machines (0.2.1)

Added by hand from the results of `.github/workflows/ci.yml` for the 0.2.1 commit. Each machine builds
Kilat and runs `judge.py --quick` (2 runs per program instead of 5) and the fuzz test. The script and
the programs are the author's; no other person repeated the measurements.

| Machine | CPython | Speed, whole process | Speed, net of start-up | Slower than CPython | Bugs refused | Fuzz cases | Fuzz failures |
|---|---|---|---|---|---|---|---|
| Windows x64 | 3.13.17 | 2.82 | 2.76 | 0 of 10 | 20 of 20 | 11,607 | 0 |
| Linux x86_64 | 3.13.18 | 2.85 | 2.80 | 0 of 10 | 20 of 20 | 11,607 | 0 |

Language tests 6 of 6, library tests 11 of 11 and sandbox tests 6 of 6 on both machines.
