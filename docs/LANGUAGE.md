# Kilat language guide (version 0.1)

Kilat is a small, statically checked language for calculations and scripts.
This guide describes what version 0.1 does. Every example below was run when the
guide was written, and the text under "Output" is what the program printed.
A longer runnable tour is in [`tests/tour.kilat`](../tests/tour.kilat).

## A first program

```kilat
let force = 1200 N
let area = 58 mm^2
let stress = force / area
print("stress = {stress:MPa.1}")
```

Output:

```text
stress = 20.7 MPa
```

Save it as `hello.kilat` and run `kilat run hello.kilat`, or paste it into the
[playground](https://juw12.github.io/kilat/).

## Values and variables

```kilat
let name = "bracket"   // `let` cannot be assigned again
var count = 3          // `var` can
count += 1
let ratio = 7 / 2      // `/` always gives a Float
let whole = 7 div 2    // floored division
print(name, count, ratio, whole, 2 ^ 10, -7 % 3)
```

Output:

```text
bracket 4 3.5 3 1024 2
```

- Types: `Int` (64-bit), `Float`, `Bool`, `Str`, `List[T]`, `Map[K, V]`, tuples `(A, B)`,
  functions `fn(A) -> R`, optional values `T?`, and quantities such as `mm` or `N/mm^2`.
- Types are worked out for you. Write `let x: Float = 1.0` when you want to state one.
- `div` and `%` are floored. `^` is power. `and`, `or`, `not` are the logic words.
- An `Int` never turns into a quantity on its own: write `n * 1 mm`.
- Integer overflow and division by zero stop the program with an error.
- Statements end at the end of the line or at `;`. Comments are `//` and `/* */`.

## Text

- `"..."` puts values into text with `{expr}` and `{expr:spec}`. `{{` and `}}` are literal braces.
- `'...'` is plain text with no `{}` handling.
- A format spec is `[<|>][width][unit][.digits]`, for example `{x:.2}`, `{p:MPa.1}`, `{n:>8}`,
  `{a:mm^2.2}` or `{t:N*m.2}`.
- Rounding is half away from zero, as on a calculator.
- `len(s)` counts characters.

## Units

```kilat
unit USD
unit EUR = 1.1 USD

let torque = 40 N * 150 mm
let speed = 90 km/h
let price = 20 EUR
print("torque = {torque:N*m.2}")
print("speed = {speed:m/s.1}, or {speed in mph} mph")
print("price = {price:USD.2}")
```

Output:

```text
torque = 6.00 N*m
speed = 25.0 m/s, or 55.9234073014 mph
price = 22.00 USD
```

A unit mistake is found before the program runs:

```kilat
let length = 2 m
let mass = 3 kg
print(length + mass)
```

Kilat answers:

```text
error: unit mismatch: cannot add m and kg
  --> example.kilat:3:14
  |
3 | print(length + mass)
  |              ^
```

- A quantity is a number followed by a unit on the same line: `250 N`, `5mm`, `9.81 m/s^2`.
  Inside a unit, `*`, `/` and `^` are written without spaces.
- Units are checked when the program is checked. While it runs, a quantity is one number in SI units.
- `x in mm` gives a plain `Float`. `{x:mm.2}` prints in a chosen unit.
- `unit NAME` declares a new base unit. `unit NAME = 1.1 OTHER` declares a derived one.
- A quantity cannot be compared with a plain `0`; write `0 N`.
- Built-in units:
  - length `m km cm mm um nm inch ft yd mile`; mass `kg g mg t lb oz`; time `s ms us ns min h day`
  - force `N kN MN mN kgf lbf`; pressure `Pa kPa MPa GPa bar mbar psi atm`
  - energy `J kJ MJ Wh kWh cal kcal Nm`; power `W kW MW mW hp`
  - electrical `A mA uA C mAh Ah V mV kV ohm kohm Mohm F uF nF pF H mH uH S`
  - frequency `Hz kHz MHz GHz rpm`; angle `rad deg rev`; volume `L mL`; speed `mph kph knot`
  - ratios `pct ppm`; also `K` and `mol`. `celsius(25.0)` gives kelvin and `to_celsius` goes back.
- `rpm` is an angular speed, so `60 rpm in Hz` is 6.283. Torque prints as `J` unless you ask for `{t:N*m}`.

## Functions, structs and enums

```kilat
struct Part { name: Str, mass: kg, qty: Int }
enum Shape { Circle(Float), Rect(Float, Float), Empty }

fn area(s: Shape) -> Float {
    match s {
        Circle(r) => pi * r ^ 2
        Rect(w, h) => w * h
        Empty => 0.0
    }
}

let parts = [
    Part(name: "bracket", mass: 0.25 kg, qty: 4),
    Part("bolt M6", 12 g, 16),
]
var total = 0 kg
for p in parts {
    total += p.mass * p.qty
}
print("total mass = {total:kg.3}")
print(area(Rect(2.0, 3.0)), area(Empty))
```

Output:

```text
total mass = 1.192 kg
6.0 0.0
```

- The last expression of a block is its value, so `return` is only needed to leave early.
- `if`/`else` and `match` are expressions. A match arm is `pattern => statement`, with an optional `if` guard.
- `a.f(b)` is another way to write `f(a, b)`.
- Generic functions: `fn first_of[T](items: List[T]) -> T?`. The bounds are `Num`, `Ord` and `Key`.
- Lists, maps and structs are shared when passed around. A lambda copies the values it uses.

## Errors are values

```kilat
fn safe_div(a: Float, b: Float) -> Result[Float] {
    if b == 0.0 { return Err("division by zero") }
    Ok(a / b)
}

fn half_of(a: Float, b: Float) -> Result[Float] {
    let q = safe_div(a, b)?   // `?` hands an Err back to the caller
    Ok(q / 2)
}

print(half_of(9.0, 3.0))
print(half_of(1.0, 0.0) ?? -1.0)   // `??` supplies a fallback
match half_of(1.0, 0.0) {
    Ok(value) => print("value", value)
    Err(message) => print("failed:", message)
}
```

Output:

```text
Ok(1.5)
-1.0
failed: division by zero
```

A `Result` that is ignored is an error found before the program runs. Write `let _ = f()` to
discard one on purpose. `T?` is an optional value: `Some(x)` or `None`.

## Lists, maps and lambdas

```kilat
let sizes = [12, 5, 8, 20]
let big = sizes.filter(|x| x > 6).map(|x| x * 2)
print(big, sum(sizes), sizes.sort(), sizes[-1])

var counts: Map[Str, Int] = [:]
for word in "the quick fox and the dog".split(" ") {
    counts[word] = counts.get_or(word, 0) + 1
}
print(counts["the"], counts.keys())

for i in 0..3 {
    print(i)
}
```

Output:

```text
[24, 16, 40] 45 [5, 8, 12, 20] 20
2 ["the", "quick", "fox", "and", "dog"]
0
1
2
```

- Lists `[a, b]`, maps `["key": value]` (empty map `[:]`), tuples `(a, b)` read with `.0` and `.1`.
- Map keys are `Int` or `Str`. Maps keep the order in which keys were added.
- Negative list positions count from the end. A position outside the list stops the program.
- Ranges: `a..b` leaves out `b`, `a..=b` includes it.

## Tests

```kilat
fn double(x: Int) -> Int { x * 2 }

test "double doubles" {
    assert_eq(double(4), 8)
}
```

Output:

```text
ok    double doubles

1 passed, 0 failed
```

`kilat test file.kilat` runs the `test` blocks of that file. Tests in imported files are not run.
`import "other.kilat"` brings in another file.

## Talking to the outside

```kilat
match read_file("notes.txt") {
    Ok(text) => print(len(text), "characters")
    Err(message) => print("could not read:", message)
}
```

Output:

```text
could not read: reading files is not allowed (run with --allow-read)
```

A Kilat program can only reach the outside through the host that runs it.

- In the `kilat` program, reading needs `--allow-read` and writing needs `--allow-write`.
  `http_get` and `ask_ai` are not available there.
- In a web page or in Python, the embedding code supplies one function, `host(name, payload)`,
  and decides what each request may do.
- The wrappers are `read_file write_file http_get ask_ai input clock unix_time args`.
- Every run has a fuel limit and a memory limit. Fuel counts work (loop turns, calls and the
  size of long text and list operations), not seconds.

## Built-in functions

- Maths: `abs min max clamp floor ceil round trunc round_to sqrt sin cos tan asin acos atan atan2
  exp ln log10 log2 float int is_nan approx rand rand_int rand_seed rand_normal`; constants `pi tau g0`.
- Text: `len upper lower trim split lines chars join contains starts_with ends_with replace find
  slice repeat to_int to_float pad_left pad_right char_code from_char_code is_empty str`.
- Lists: `len push pop insert remove_at first last contains index_of slice reverse sort sort_by
  concat range fill clear copy is_empty extend sum map filter reduce each any all count find
  flat_map zip enumerate group_by`.
- Maps: `len get get_or has remove keys values items clear is_empty copy`.
- Optional and Result: `unwrap unwrap_or is_some is_none is_ok is_err ok`.
- JSON: `json_parse json_str json_pretty`, with accessors such as `get`, `at`, `as_int`, `as_num`, `as_str`.
- Other: `print assert assert_eq panic host`.

## The command line

```text
kilat run file.kilat [args]     run a program (also: kilat file.kilat)
kilat check file.kilat          check without running
kilat test file.kilat           run the test blocks
kilat eval "print(2 ^ 10)"      run one line (also: -e)
kilat version | kilat help
flags: --fuel n   --mem MB   --allow-read   --allow-write
exit codes: 0 fine, 1 error, 2 wrong usage
```

## Known limits of version 0.1

- No packages, classes, traits, async or threads. No debugger and no interactive prompt.
- Bounds on type parameters only allow passing the value to built-in functions.
- A lambda has one type; it is not generic. Built-in functions cannot be passed as values; wrap them in a lambda.
- A name cannot be declared twice with `let` in the same block.
- A function without a stated return type that reads a later top-level `var` needs the return type written.
- Quantities have no fractional powers.
- Float overflow gives `inf`, and `inf - inf` gives `nan`. Floats print 12 significant digits.
  The last digit of a float result can differ between the native program and the browser.
- `len(s)` and `s[i]` walk the text, and building a long text with `s = s + x` in a loop is slow.
- The memory limit is approximate.
- Named arguments of a struct are evaluated in field order.
