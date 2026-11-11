# Measured results

Written by `python judge.py` on 2026-11-11. Kilat kilat 0.1.0, CPython 3.13.2, Windows 10, Intel(R) Core(TM) Ultra 7 258V.

## Speed against CPython

Best of 5 runs each. CPU time is the whole process, measured with `QueryProcessCycleTime`, with both languages kept on the fastest cores.
"Net" takes each language's start-up time off first, which is the stricter figure for Kilat.
Both languages run the same plain algorithm; Python with NumPy would be faster on some of these.
Wall-clock time on the development PC is dominated by how long a process takes to start, so it is shown last.

| Program | Kilat CPU s | CPython CPU s | Times faster | Net of start-up | Kilat wall s | CPython wall s |
|---|---|---|---|---|---|---|
| `collatz` | 0.345 | 0.775 | 2.25 | 2.09 | 0.735 | 1.572 |
| `fib` | 0.087 | 0.307 | 3.53 | 2.98 | 0.220 | 0.442 |
| `loop_sum` | 0.121 | 0.540 | 4.45 | 4.11 | 0.233 | 1.248 |
| `mandelbrot` | 0.143 | 0.657 | 4.60 | 4.32 | 0.268 | 1.874 |
| `map_count` | 0.278 | 0.284 | 1.02 | 0.80 | 0.678 | 0.680 |
| `matmul` | 0.085 | 0.247 | 2.92 | 2.30 | 0.300 | 0.637 |
| `nbody` | 0.145 | 0.593 | 4.10 | 3.80 | 0.539 | 1.913 |
| `sieve` | 0.146 | 0.288 | 1.98 | 1.58 | 0.467 | 0.839 |
| `sort` | 0.091 | 0.761 | 8.37 | 8.21 | 0.274 | 1.500 |
| `strings` | 0.228 | 0.225 | 0.99 | 0.71 | 0.453 | 0.533 |

- Geometric mean: 2.82 times faster for the whole process, 2.43 net of start-up.
- Net of start-up Kilat is faster on 8 of 10 programs, slower on `map_count`, `strings`.
- The target set before measuring was 5 times: not met.
- The two languages print the same text on every program: yes.
- Start-up: Kilat 7 ms of CPU (68 ms wall), CPython 68 ms (104 ms wall).

## Sizes

| File | Bytes |
|---|---|
| kilat program | 1,144,832 |
| kilat library | 1,633,792 |
| docs/kilat.wasm | 715,723 |
| docs/kilat.js | 9,490 |
| main.js (Cairn plugin) | 352,472 |

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
| Conformance programs print their recorded output | 5 of 5 |
| The library prints what the program prints | 10 of 10 |
| Limits and file access on the command line | 6 of 6 |
| Behaviours quoted in the README | 6 of 6 |
| WebAssembly build prints what the program prints | 10 of 10 |
| Engine in a browser | 37 of 37 |
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

- `playgroundLimitMs`: 104
- `playgroundLongestFreezeMs`: 154
- `pluginCharacters`: 352472
- `pluginFirstResultMs`: 165
- `pluginLimitMs`: 42
- `pluginLongTextLimitMs`: 2933
- `pluginLongestFreezeMs`: 233
- `pluginReadyMs`: 2124
- `pluginRedrawMs`: 17
- `pluginTimeLimitMs`: 10071
