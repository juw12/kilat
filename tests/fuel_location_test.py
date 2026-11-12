"""Checks that a fuel stop names the line that was running, not a library function.

Version 0.2.0 reported `<prelude>:309 in to_celsius` for an empty `while true { }` loop.
Run: python tests/fuel_location_test.py     (needs the built program, see build.py)
"""

import subprocess
import sys
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
PROGRAM = ROOT / "target" / "release" / ("kilat.exe" if sys.platform == "win32" else "kilat")

CASES = [
    # (name, source, line that must be named)
    ("empty loop at the start", "while true { }\n", 1),
    ("empty loop after a print", "print(1)\nwhile true { }\n", 2),
    ("counting loop", "var n = 0\nwhile true {\n    n = n + 1\n}\n", None),
    (
        "loop inside a function",
        "fn spin() {\n    while true { }\n}\nspin()\n",
        2,
    ),
]


def run_case(folder, name, source, wanted_line):
    path = Path(folder) / "case.kilat"
    path.write_text(source, encoding="utf-8", newline="\n")
    done = subprocess.run(
        [str(PROGRAM), str(path), "--fuel", "200000"],
        capture_output=True,
        text=True,
        encoding="utf-8",
        errors="replace",
        timeout=60,
    )
    text = done.stdout + done.stderr
    problems = []
    if done.returncode == 0:
        problems.append("the program was not stopped")
    if "fuel limit" not in text:
        problems.append("no fuel message")
    if "<prelude>" in text:
        problems.append("the message names the built-in library")
    if "case.kilat:" not in text:
        problems.append("the message does not name the program file")
    if wanted_line is not None and f"case.kilat:{wanted_line}" not in text:
        problems.append(f"the message does not name line {wanted_line}")
    return problems, text.strip()


def main():
    if not PROGRAM.exists():
        print(f"missing {PROGRAM}; run: python build.py native")
        return 2
    failed = 0
    with tempfile.TemporaryDirectory() as folder:
        for name, source, wanted_line in CASES:
            problems, text = run_case(folder, name, source, wanted_line)
            if problems:
                failed += 1
                print(f"FAIL {name}: {'; '.join(problems)}")
                print("     " + text.replace("\n", "\n     "))
            else:
                print(f"ok   {name}")
    print(f"fuel location: {len(CASES) - failed} of {len(CASES)}")
    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main())
