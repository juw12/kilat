"""Checks how Kilat shows values that have a unit, and the hints it gives.

The rule since 0.2.3: a value with a unit never turns into text unless the program names
the unit. `{d}`, `print(d)` and `str(d)` are refused with a hint that names a unit, and
`{d:mm}` runs. Before 0.2.3 a bare `{d}` printed the stored SI number (`0.0024 m` for
`2.4 mm`, `151.84 1/s` for `1450 rpm`), so a program could print a right number in a unit
nobody asked for.

The first cases come from programs a free model really wrote (found with 0.2.1):

1. `speed * 2 * pi / 60` on a speed that already has the unit rpm (printed 471.7, not 49.4)
2. `x in J` (a plain number) shown with `{x:J.3}`
3. `"{force} N"`, which printed "96 N N"

Four kinds of check, each against the built program:

REFUSED   the program must not run; the message and the hint must both be there, once
ALLOWED   the program must run and print exactly this
EVALS     `kilat eval <expression>`: the last value is shown, so the same rule holds
TESTS     `kilat test <file>`: what a failed `assert_eq` on values with a unit says

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
SIZES = "let sizes = [2.4 mm, 3.1 mm]\n"
PART = 'struct Part { name: Str, mass: kg, qty: Int }\nlet part = Part(name: "bolt", mass: 0.25 kg, qty: 4)\n'
STRESS = "let stress = 1200 N / 20 mm^2\n"
SHOW = 'fn show[T](item: T) {\n    print("item = {item}")\n}\n'

NO_UNIT = "this value has a unit, so say which unit to show it in"
ROUNDING = (
    "(the two differ only beyond the digits shown; "
    "for calculated numbers use `assert(approx(a, b, tolerance))`)"
)

REFUSED = [
    # (name, source, text the message must hold, text the hint must hold)
    #
    # --- a speed in rpm converted a second time
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
    # --- a plain number shown with a unit
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
    # --- a value with a unit shown without naming the unit
    (
        "a bare placeholder",
        'let d = 2.4 mm\nprint("{d}")\n',
        NO_UNIT,
        "write `{d:mm}`",
    ),
    (
        "a unit word after a placeholder",
        SPRING + 'print("Force = {force} N")\n',
        NO_UNIT,
        "write `{force:N}` and delete the `N` after the braces",
    ),
    (
        "another unit after a placeholder, with digits",
        SPRING + 'print("x = {x:.4} mm")\n',
        NO_UNIT,
        "write `{x:mm.4}` and delete the `mm` after the braces",
    ),
    (
        "a unit word after a computed placeholder",
        'let time = 3000 s\nprint("{time / 1:.2} min")\n',
        NO_UNIT,
        "write `{time / 1:min.2}` and delete the `min` after the braces",
    ),
    (
        "a value with a unit given to print",
        "let d = 2.4 mm\nprint(d)\n",
        NO_UNIT,
        'put it in a text with its unit: `"{d:mm}"`',
    ),
    (
        "a value with a unit given to str",
        "let d = 2.4 mm\nlet label = str(d)\nprint(label)\n",
        NO_UNIT,
        'write `"{d:mm}"` in place of `str(d)`',
    ),
    # --- the unit written twice
    (
        "the same unit inside and after the braces",
        SPRING + 'print("Force = {force:N} N")\n',
        "this placeholder already prints `N`, so the `N` after the braces would be a second unit",
        "delete the `N` after the braces",
    ),
    (
        "the same unit again, before a full stop",
        SPRING + 'print("{force:N} N.")\n',
        "this placeholder already prints `N`, so the `N` after the braces would be a second unit",
        "delete the `N` after the braces",
    ),
    # --- a value with a unit inside something else
    (
        "a list of values with a unit",
        SIZES + 'print("{sizes}")\n',
        "this list holds values with a unit, so say which unit to show them in",
        "for the plain numbers write `sizes.map(|x| x in mm)`",
    ),
    (
        "a struct that holds a value with a unit",
        PART + 'print("{part}")\n',
        "this `Part` holds a value with a unit (field `mass`), so say which unit to show it in",
        'show the fields one by one, e.g. `"{part.mass:kg}"`',
    ),
    (
        "a tuple that holds a value with a unit",
        'let pair = ("shaft", 12 mm)\nprint("{pair}")\n',
        "this tuple holds a value with a unit, so say which unit to show it in",
        'show the parts one by one, e.g. `"{pair.1:mm}"`',
    ),
    (
        "a map of values with a unit",
        'let limits = ["bore": 12 mm, "boss": 20 mm]\nprint("{limits}")\n',
        "this map holds values with a unit, so say which unit to show them in",
        'for (key, value) in limits.items() { print("{key}: {value:mm}") }',
    ),
    (
        "an optional value with a unit",
        SIZES + 'let maybe = find(sizes, |x| x > 3 mm)\nprint("{maybe}")\n',
        "this optional value has a unit, so say which unit to show it in",
        "take the number out first, e.g. `let value = maybe ?? 0 mm`",
    ),
    # --- a value with a unit that reaches a placeholder through a function
    (
        "a lambda that shows its argument",
        'let show = |x| print("{x}")\nshow(2 mm)\n',
        NO_UNIT,
        "write `{x:m}` (or another unit of the same kind: `km`, `cm`, `mm`)",
    ),
    (
        "a generic function that shows its argument",
        SHOW + 'show(5)\nshow("text")\nshow(2 mm)\n',
        "`show` turns this value into text, but the value has a unit",
        'pass a text such as `"{value:mm}"`, or a plain number such as `value in mm`',
    ),
    (
        "a generic function called by another",
        SHOW + "fn show_twice[T](item: T) {\n    show(item)\n    show(item)\n}\nshow_twice(7)\nshow_twice(3 kg)\n",
        "`show_twice` turns this value into text, but the value has a unit",
        'pass a text such as `"{value:kg}"`, or a plain number such as `value in kg`',
    ),
    (
        "a generic function passed as a value",
        SHOW + SIZES + "each(sizes, show)\n",
        "`show` turns this value into text, but the value has a unit",
        "name the unit before the call",
    ),
    # --- the hint names a unit that fits the program
    (
        "a speed in rpm, bare",
        'let n = 1450 rpm\nprint("speed {n}")\n',
        NO_UNIT,
        "write `{n:rpm}`",
    ),
    (
        "an angular speed, bare",
        'let omega = 151.84 rad/s\nprint("omega {omega}")\n',
        NO_UNIT,
        "write `{omega:rad/s}`",
    ),
    (
        "a frequency, bare",
        'let f = 50 Hz\nprint("f {f}")\n',
        NO_UNIT,
        "write `{f:Hz}`",
    ),
    (
        "a stress, bare",
        STRESS + 'print("stress {stress}")\n',
        NO_UNIT,
        "write `{stress:N/mm^2}`",
    ),
    (
        "a torque, bare",
        'let torque = 1200 N * 150 mm\nprint("torque {torque}")\n',
        NO_UNIT,
        "write `{torque:N*mm}`",
    ),
    (
        "the hint follows the unit shown before",
        STRESS + 'print("{stress:MPa.2}")\nprint("{stress}")\n',
        NO_UNIT,
        "write `{stress:MPa}`",
    ),
    (
        "the hint follows the unit shown for a field",
        PART + 'print("{part.mass:g.0}")\nprint("{part.mass}")\n',
        NO_UNIT,
        "write `{part.mass:g}`",
    ),
    (
        "a unit that cannot be written",
        'let d = 4 mm\nlet odd = sqrt(d)\nprint("{odd}")\n',
        "this value has a unit that cannot be written in a format (`m^(1/2)`)",
        "finish the calculation so the unit is a whole one",
    ),
]

ALLOWED = [
    # (name, source, what it must print)
    #
    # --- the right forms of the mistakes above
    ("torque from power and rpm", SHAFT + 'print("{power / speed:N*m.1}")\n', "49.4 N*m\n"),
    ("2 pi f, no 60", 'let f = 50 Hz\nprint("{2 * pi * f:rad/s.1}")\n', "314.2 rad/s\n"),
    ("a plain rpm number times 2 pi over 60", 'let n = 1450.0\nprint("{n * 2 * pi / 60:.2}")\n', "151.84\n"),
    ("a speed over 60 s", 'let f = 50 Hz\nprint("{f * 2 * pi / 60 s:1/s^2.3}")\n', "5.236 1/s^2\n"),
    ("rim speed, no frequency", 'let r = 0.3 m\nprint("{2 * pi * r / 60 s:m/s.4}")\n', "0.0314 m/s\n"),
    ("a plain number, then the unit", SPRING + 'print("{0.5 * k * x * x in J:.3} J")\n', "0.384 J\n"),
    ("the unit inside the braces", SPRING + 'print("{force:N.0} and {0.5 * k * x * x:J.3}")\n', "96 N and 0.384 J\n"),
    ("a plain number with the unit after it", SPRING + 'print("{x in mm} mm")\n', "8.0 mm\n"),
    # --- a named unit
    ("a named unit", 'let d = 2.4 mm\nprint("{d:mm}")\n', "2.4 mm\n"),
    ("a named unit with a width and digits", 'let d = 2.4 mm\nprint("{d:>10mm.2}|")\n', "      2.40 mm|\n"),
    ("per second, written 1/s", 'let rate = 81 Hz\nprint("{rate:1/s} or {rate:Hz}")\n', "81.0 1/s or 81.0 Hz\n"),
    (
        "a speed shown in rpm",
        'let n = 1450 rpm\nprint("speed {n:rpm} or {n:rpm.0}")\n',
        "speed 1450.0 rpm or 1450 rpm\n",
    ),
    (
        "a stress in MPa and in N/mm^2",
        STRESS + 'print("stress {stress:MPa} = {stress:N/mm^2}")\n',
        "stress 60.0 MPa = 60.0 N/mm^2\n",
    ),
    (
        "a torque in N*m and in N*mm",
        'let torque = 1200 N * 150 mm\nprint("{torque:N*m} = {torque:N*mm.0}")\n',
        "180.0 N*m = 180000 N*mm\n",
    ),
    # --- a plain word after the braces is not taken for a second unit
    (
        "a placeholder, then a plain word",
        SPRING + 'print("{force:N} needed, {x:mm} moved")\n',
        "96.0 N needed, 8.0 mm moved\n",
    ),
    ("a count, then a unit word", 'let turns = 3\nprint("{turns} m of wire")\n', "3 m of wire\n"),
    ("a unit word joined to more letters", SPRING + 'print("{x:mm} metres")\n', "8.0 mm metres\n"),
    ("min after a length is a plain word", SPRING + 'print("{x:mm} min clearance")\n', "8.0 mm min clearance\n"),
    ("in after a force is a plain word", SPRING + 'print("{force:N} in the spring")\n', "96.0 N in the spring\n"),
    ("a unit in brackets after the braces", SPRING + 'print("{force:N} (N)")\n', "96.0 N (N)\n"),
    ("per mm after a stiffness", SPRING + 'print("{k:N/mm} per mm")\n', "12.0 N/mm per mm\n"),
    # --- a value with a unit inside something else, shown the explicit way
    (
        "a list as plain numbers, then one by one",
        SIZES + 'print("{sizes.map(|x| x in mm)}")\nfor size in sizes {\n    print("{size:mm}")\n}\n',
        "[2.4, 3.1]\n2.4 mm\n3.1 mm\n",
    ),
    (
        "the fields of a struct one by one",
        PART + 'print("{part.name}: {part.mass:kg} x {part.qty}")\n',
        "bolt: 0.25 kg x 4\n",
    ),
    (
        "a map one by one",
        'let limits = ["bore": 12 mm, "boss": 20 mm]\n'
        'for (key, value) in limits.items() {\n    print("{key}: {value:mm}")\n}\n',
        "bore: 12.0 mm\nboss: 20.0 mm\n",
    ),
    (
        "an optional value taken out first",
        SIZES
        + "let maybe = find(sizes, |x| x > 3 mm)\n"
        + 'let value = maybe ?? 0 mm\nprint("{value:mm}")\n'
        + 'match maybe {\n    Some(v) => print("found {v:mm}")\n    None => print("nothing")\n}\n',
        "3.1 mm\nfound 3.1 mm\n",
    ),
    # --- functions
    ("a lambda that names the unit", 'let show = |x| print("{x:mm}")\nshow(2 mm)\n', "2.0 mm\n"),
    ("a lambda on plain numbers", 'let show = |x| print("{x}")\nshow(2)\nshow(3)\n', "2\n3\n"),
    ("a generic function on a number and a text", SHOW + 'show(5)\nshow("text")\n', "item = 5\nitem = text\n"),
    (
        "a generic function given a text with the unit",
        SHOW + 'let d = 2 mm\nshow("{d:mm}")\nshow(d in mm)\n',
        "item = 2.0 mm\nitem = 2.0\n",
    ),
]

EVALS = [
    # (name, expression, what it must print, or None when it must be refused,
    #  text the message must hold, text the hint must hold)
    (
        "a value with a unit",
        "2.4 mm",
        None,
        NO_UNIT,
        'end with `(2.4 mm) in mm` for the plain number, or show it as a text: `"{2.4 mm:mm}"`',
    ),
    (
        "a list of values with a unit",
        "[1 mm, 2 mm]",
        None,
        "this list holds values with a unit, so say which unit to show them in",
        'show each number on its own with its unit, e.g. `"{x:mm}"` inside a loop',
    ),
    ("a value with a unit, as a plain number", "2.4 mm in mm", "2.4\n", "", ""),
    ("a value with a unit, as a text", '"{2.4 mm:mm}"', "2.4 mm\n", "", ""),
    ("a stress shown in MPa", '"{1200 N / 20 mm^2:MPa}"', "60.0 MPa\n", "", ""),
    ("a plain sum", "1 + 2", "3\n", "", ""),
]

TESTS = [
    # (name, source, True when the test must pass, text the output must hold)
    (
        "sizes that differ are shown in the unit the test wrote",
        'test "gap" {\n    assert_eq(2.4 mm, 2.5 mm)\n}\n',
        False,
        "assertion failed: 2.4 mm is not equal to 2.5 mm\n",
    ),
    (
        "a stress that differs is shown in MPa",
        'test "stress" {\n    let stress = 1200 N / 20 mm^2\n    assert_eq(stress, 61 MPa)\n}\n',
        False,
        "assertion failed: 60.0 MPa is not equal to 61.0 MPa\n",
    ),
    (
        "lists that differ say the numbers are in SI base units",
        'test "lists" {\n    assert_eq([1 mm, 2 mm], [1 mm, 3 mm])\n}\n',
        False,
        "[0.001, 0.002] is not equal to [0.001, 0.003] (numbers with a unit are shown in SI base units)\n",
    ),
    (
        "plain numbers that differ",
        'test "plain" {\n    assert_eq(2, 3)\n}\n',
        False,
        "assertion failed: 2 is not equal to 3\n",
    ),
    (
        "values that agree, also across units",
        'test "agree" {\n    assert_eq(2.4 mm, 2.4 mm)\n    assert_eq(2.4 mm, 0.0024 m)\n'
        '    assert_eq(1, 1)\n    assert_eq("a", "a")\n}\n',
        True,
        "1 passed, 0 failed\n",
    ),
    (
        "a calculated value that differs only by rounding says so",
        'test "rounding" {\n    assert_eq(1200 N / 20 mm^2, 60 MPa)\n}\n',
        False,
        "assertion failed: 60.0 MPa is not equal to 60.0 MPa " + ROUNDING + "\n",
    ),
    (
        "plain floats that differ only by rounding say so",
        'test "floats" {\n    assert_eq(0.1 + 0.2, 0.3)\n}\n',
        False,
        "assertion failed: 0.3 is not equal to 0.3 " + ROUNDING + "\n",
    ),
    (
        "the same calculated value passes with approx",
        'test "approx" {\n    assert(approx(1200 N / 20 mm^2, 60 MPa, 1 Pa))\n}\n',
        True,
        "1 passed, 0 failed\n",
    ),
]


def run_program(arguments):
    done = subprocess.run(
        [str(PROGRAM)] + arguments,
        capture_output=True,
        text=True,
        encoding="utf-8",
        errors="replace",
        timeout=60,
    )
    return done.returncode, done.stdout, done.stderr


def run_source(folder, source, command=None):
    path = Path(folder) / "case.kilat"
    path.write_text(source, encoding="utf-8", newline="\n")
    arguments = [command, str(path)] if command else [str(path)]
    return run_program(arguments)


def refusal_problems(code, printed, errors, message, hint):
    problems = []
    if code == 0:
        problems.append(f"it ran and printed {printed.strip()!r}")
    if message not in errors:
        problems.append("the message is missing")
    if hint not in errors:
        problems.append("the hint is missing")
    if errors.count(message) > 1:
        problems.append("the same mistake is reported twice")
    return problems


def indented(text):
    return "     " + text.strip().replace("\n", "\n     ")


def main():
    if not PROGRAM.exists():
        print(f"missing {PROGRAM}; run: python build.py native")
        return 2
    failed = 0
    with tempfile.TemporaryDirectory() as folder:
        for name, source, message, hint in REFUSED:
            code, printed, errors = run_source(folder, source)
            problems = refusal_problems(code, printed, errors, message, hint)
            if problems:
                failed += 1
                print(f"FAIL refused: {name}: {'; '.join(problems)}")
                print(indented(errors))
            else:
                print(f"ok   refused: {name}")
        for name, source, wanted in ALLOWED:
            code, printed, errors = run_source(folder, source)
            if code != 0 or printed != wanted:
                failed += 1
                print(f"FAIL allowed: {name}: exit {code}, printed {printed!r}, wanted {wanted!r}")
                print(indented(errors))
            else:
                print(f"ok   allowed: {name}")
        for name, expression, wanted, message, hint in EVALS:
            code, printed, errors = run_program(["eval", expression])
            if wanted is None:
                problems = refusal_problems(code, printed, errors, message, hint)
            elif code != 0 or printed != wanted:
                problems = [f"exit {code}, printed {printed!r}, wanted {wanted!r}"]
            else:
                problems = []
            if problems:
                failed += 1
                print(f"FAIL eval:    {name}: {'; '.join(problems)}")
                print(indented(errors))
            else:
                print(f"ok   eval:    {name}")
        for name, source, passes, wanted in TESTS:
            code, printed, errors = run_source(folder, source, command="test")
            if (code == 0) != passes or wanted not in printed:
                failed += 1
                print(f"FAIL test:    {name}: exit {code}, wanted {wanted!r} in the output")
                print(indented(printed + errors))
            else:
                print(f"ok   test:    {name}")
    total = len(REFUSED) + len(ALLOWED) + len(EVALS) + len(TESTS)
    print(f"unit hints: {total - failed} of {total}")
    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main())
