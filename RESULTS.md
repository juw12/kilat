# Measured results

Written by `python judge.py` on 2026-11-12. Kilat kilat 0.2.0, CPython 3.13.2, Windows 10, Intel(R) Core(TM) Ultra 7 258V.

## Speed against CPython

Best of 5 runs each. CPU time is the whole process, measured with `QueryProcessCycleTime`, with both languages kept on the fastest cores.
"Net" takes each language's start-up time off first, which is the stricter figure for Kilat.
Both languages run the same plain algorithm; Python with NumPy would be faster on some of these.
Wall-clock time on the development PC is dominated by how long a process takes to start, so it is shown last.

| Program | Kilat CPU s | CPython CPU s | Times faster | Net of start-up | Kilat wall s | CPython wall s |
|---|---|---|---|---|---|---|
| `collatz` | 0.290 | 0.940 | 3.24 | 3.05 | 0.333 | 1.174 |
| `fib` | 0.108 | 0.356 | 3.30 | 2.78 | 0.118 | 0.384 |
| `loop_sum` | 0.161 | 0.627 | 3.90 | 3.57 | 0.203 | 0.815 |
| `mandelbrot` | 0.190 | 0.823 | 4.32 | 4.06 | 0.229 | 1.044 |
| `map_count` | 0.246 | 0.324 | 1.32 | 1.06 | 0.289 | 0.365 |
| `matmul` | 0.107 | 0.276 | 2.58 | 2.02 | 0.159 | 0.348 |
| `nbody` | 0.190 | 0.719 | 3.78 | 3.51 | 0.235 | 0.882 |
| `sieve` | 0.115 | 0.295 | 2.56 | 2.04 | 0.136 | 0.347 |
| `sort` | 0.081 | 0.670 | 8.24 | 7.87 | 0.093 | 0.798 |
| `strings` | 0.144 | 0.214 | 1.49 | 1.04 | 0.167 | 0.254 |

- Geometric mean: 3.06 times faster for the whole process, 2.62 net of start-up.
- Net of start-up Kilat is faster on 9 of 10 programs.
- The target set before measuring was 5 times: not met.
- The two languages print the same text on every program: yes.
- Start-up: Kilat 5 ms of CPU (8 ms wall), CPython 70 ms (82 ms wall).

## Sizes

| File | Bytes |
|---|---|
| kilat program | 1,271,808 |
| kilat library | 1,719,296 |
| docs/kilat.wasm | 722,701 |
| docs/kilat.js | 16,007 |
| main.js (Cairn plugin) | 361,965 |

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

- `playgroundLimitMs`: 114
- `playgroundLongestFreezeMs`: 119
- `pluginCharacters`: 361965
- `pluginFirstResultMs`: 71
- `pluginLimitMs`: 49
- `pluginLongTextLimitMs`: 3185
- `pluginLongestFreezeMs`: 132
- `pluginReadyMs`: 494
- `pluginRedrawMs`: 15
- `pluginTimeLimitMs`: 10071

## Fuzz and differential tests

Added by hand from the output of `python tests/fuzz_test.py --jobs 6` (seed 20261112, 200,000 fuel and
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
- Worker restarts: 0. The run took 591 s on a heavily loaded PC; the longest single case took 37 s
  there (the hang limit was 60 s).
- Before the run the harness caught a planted crash and a planted hang (`--selftest`).
- One fault was found while writing the test: a fuel stop can report the wrong place
  (`at <prelude>:309 in to_celsius` for an empty `while true { }` loop). It is not fixed.

## Release download check

Added by hand from a separate script that downloads the 0.2.0 release files.

| Check | Result |
|---|---|
| `kilat.exe` from the release page | 1,271,808 bytes, SHA-256 same as the local build |
| `kilat-windows-x64.zip` | 10 files; the `kilat.exe` inside has the same SHA-256 |
| `kilat --version` on the download | `kilat 0.2.0` |
| The five examples | each prints the same lines as the local build |
| `print(2 m + 3 kg)` | refused: unit mismatch, cannot add m and kg |

SHA-256 of `kilat.exe`: `6b07ba0dacfeb3fb7d83f57385594a810e02f1874b749e19994ab118ec312c9d`

`install.cmd` was only run with `/dry-run`. The files are unsigned.
