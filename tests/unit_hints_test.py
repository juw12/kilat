"""Checks the unit hints added in 0.2.2, using programs a model really wrote.

A free model, asked for small engineering programs, made three kinds of mistake that
Kilat 0.2.1 either let through with a wrong number or refused without saying why:

1. `speed * 2 * pi / 60` on a speed that already has the unit rpm (printed 471.7, not 49.4)
2. `x in J` (a plain number) shown with `{x:J.3}`
3. `"{force} N"`, which printed "96 N N"

Run: python tests/unit_hints_test.py     (needs the built program, see build.py)
"""

import subprocess
import sys
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
PROGRAM = ROOT / "target" / "release" / ("kilat.exe" if sys.platform == "win32" else "kilat")

SHAFT = "let speed = 1450 rpm\nlet power = 7.5 kW\n"
SPRING = "let k = 12 N/mm\nlet x = 8 mm\nlet force = k * x\n"

REFUSED = [
    # (name, source, text the message must hold, text the hint must hold)
    (
        "rpm times 2 pi over (60 * 1 s)",
        SHAFT + 'let omega = speed * 2 * pi / (60 * 1 s)\nprint("{power / omega:N*m.1}")\n',
        "`2 * pi / 60` turns a plain rpm number into rad/s, but `speed` already has a unit",
        "torque = power / speed",
    ),
    (
        "rpm times 2 pi over 60 rad/s",
        SHAFT + 'let omega = speed * 2 * pi / 60 rad/s\nprint("{power / omega / (1 N*m):.1}")\n',
        "`2 * pi / 60` turns a plain rpm number into rad/s",
        "use it as it is",
    ),
    (
        "60 P over 2 pi n, with n in rpm",
        SHAFT + 'let torque = 60 * power / (2 * pi * speed)\nprint("{torque:N*m.1}")\n',
        "`2 * pi / 60` turns a plain rpm number into rad/s, but `speed` already has a unit",
        "torque = power / speed",
    ),
    (
        "rpm times pi over 30",
        SHAFT + 'let omega = speed * pi / 30\nprint("{power / omega:N*m.1}")\n',
        "`pi / 30` turns a plain rpm number into rad/s",
        "torque = power / speed",
    ),
    (
        "a literal in rpm times 2 pi over 60",
        'let omega = 1450 rpm * 2 * pi / 60\nprint("{omega}")\n',
        "but this value already has a unit",
        "use it as it is",
    ),
    (
        "in J, then shown as J",
        SPRING + 'let energy = 0.5 * k * x * x in J\nprint("Energy = {energy:J.3}")\n',
        "unit mismatch: cannot show Float as J",
        "`{value:.3} J`",
    ),
    (
        "in J, then shown as J without digits",
        SPRING + 'let energy = 0.5 * k * x * x in J\nprint("Energy = {energy:J}")\n',
        "unit mismatch: cannot show Float as J",
        "removing `in J`",
    ),
    (
        "a unit word after a placeholder",
        SPRING + 'print("Force = {force} N")\n',
        "this placeholder already prints a unit, so the `N` after it would be a second unit",
        "`{force:N}`",
    ),
    (
        "another unit after a placeholder, with digits",
        SPRING + 'print("x = {x:.4} mm")\n',
        "so the `mm` after it would be a second unit",
        "`{x:mm.4}`",
    ),
    (
        "a unit word after a computed placeholder",
        'let time = 3000 s\nprint("{time / 1:.2} min")\n',
        "so the `min` after it would be a second unit",
        "`{value:min.2}`",
    ),
]

ALLOWED = [
    # (name, source, what it must print)
    ("torque from power and rpm", SHAFT + 'print("{power / speed:N*m.1}")\n', "49.4 N*m\n"),
    ("2 pi f, no 60", 'let f = 50 Hz\nprint("{2 * pi * f:.1}")\n', "314.2 1/s\n"),
    ("a plain rpm number times 2 pi over 60", 'let n = 1450.0\nprint("{n * 2 * pi / 60:.2}")\n', "151.84\n"),
    ("a speed over 60 s", 'let f = 50 Hz\nprint("{f * 2 * pi / 60 s:.3}")\n', "5.236 1/s^2\n"),
    ("rim speed, no frequency", 'let r = 0.3 m\nprint("{2 * pi * r / 60 s:.4}")\n', "0.0314 m/s\n"),
    ("a plain number, then the unit", SPRING + 'print("{0.5 * k * x * x in J:.3} J")\n', "0.384 J\n"),
    ("the unit inside the braces", SPRING + 'print("{force:N.0} and {0.5 * k * x * x:J.3}")\n', "96 N and 0.384 J\n"),
    ("a placeholder, then a plain word", SPRING + 'print("{force} needed, {x} moved")\n', "96.0 N needed, 0.008 m moved\n"),
    ("a count, then a unit word", 'let turns = 3\nprint("{turns} m of wire")\n', "3 m of wire\n"),
    ("a unit word joined to more letters", SPRING + 'print("{x} metres")\n', "0.008 m metres\n"),
]


def run_source(folder, source):
    path = Path(folder) / "case.kilat"
    path.write_text(source, encoding="utf-8", newline="\n")
    done = subprocess.run(
        [str(PROGRAM), str(path)],
        capture_output=True,
        text=True,
        encoding="utf-8",
        errors="replace",
        timeout=60,
    )
    return done.returncode, done.stdout, done.stderr


def main():
    if not PROGRAM.exists():
        print(f"missing {PROGRAM}; run: python build.py native")
        return 2
    failed = 0
    with tempfile.TemporaryDirectory() as folder:
        for name, source, message, hint in REFUSED:
            code, printed, errors = run_source(folder, source)
            problems = []
            if code == 0:
                problems.append(f"it ran and printed {printed.strip()!r}")
            if message not in errors:
                problems.append("the message is missing")
            if hint not in errors:
                problems.append("the hint is missing")
            if errors.count(message) > 1:
                problems.append("the same mistake is reported twice")
            if problems:
                failed += 1
                print(f"FAIL refused: {name}: {'; '.join(problems)}")
                print("     " + errors.strip().replace("\n", "\n     "))
            else:
                print(f"ok   refused: {name}")
        for name, source, wanted in ALLOWED:
            code, printed, errors = run_source(folder, source)
            if code != 0 or printed != wanted:
                failed += 1
                print(f"FAIL allowed: {name}: exit {code}, printed {printed!r}, wanted {wanted!r}")
                print("     " + errors.strip().replace("\n", "\n     "))
            else:
                print(f"ok   allowed: {name}")
    total = len(REFUSED) + len(ALLOWED)
    print(f"unit hints: {total - failed} of {total}")
    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main())
