#!/usr/bin/env python3
"""Builds Kilat.

    python build.py           everything below, in this order
    python build.py native    the kilat program and the kilat_core library
    python build.py wasm      docs/kilat.wasm
    python build.py web       docs/examples.js and main.js (the Cairn plugin), from the
                              files that are already there; needs no Rust

Only the Python standard library is used. `native` and `wasm` need Rust (cargo), and
`wasm` needs the target once: rustup target add wasm32-unknown-unknown
"""

import base64
import json
import os
import re
import shutil
import subprocess
import sys
import zlib
from pathlib import Path

ROOT = Path(__file__).resolve().parent
DOCS = ROOT / "docs"
WASM_TARGET = "wasm32-unknown-unknown"
# Cairn refuses a plugin file that is longer than this many characters.
PLUGIN_LIMIT = 400_000

# The programs offered in the playground: (file, name in the picker).
EXAMPLES = [
    ("examples/hello.kilat", "Hello, units"),
    ("examples/bolt_joint.kilat", "Bolted joint"),
    ("examples/shaft_power.kilat", "Motor shaft"),
    ("examples/beam_deflection.kilat", "Cantilever beam"),
    ("examples/bom_mass.kilat", "Bill of materials"),
    ("bugs/units_wrong_formula.kilat", "A unit mistake, caught"),
]


def fail(message):
    sys.exit(f"build.py: {message}")


def run(command):
    print("+ " + " ".join(command), flush=True)
    code = subprocess.run(command, cwd=ROOT).returncode
    if code != 0:
        fail(f"`{command[0]}` failed with exit code {code}")


def has_microsoft_linker():
    """True when the Visual C++ build tools are installed, which the MSVC Rust toolchain needs."""
    program_files = os.environ.get("ProgramFiles(x86)", r"C:\Program Files (x86)")
    finder = Path(program_files) / "Microsoft Visual Studio" / "Installer" / "vswhere.exe"
    if not finder.is_file():
        return False
    query = [
        str(finder),
        "-products",
        "*",
        "-requires",
        "Microsoft.VisualStudio.Component.VC.Tools.x86.x64",
        "-property",
        "installationPath",
    ]
    found = subprocess.run(query, capture_output=True, text=True)
    return found.returncode == 0 and bool(found.stdout.strip())


def native_toolchain():
    """Picks the Rust toolchain for the native build, as a list of extra cargo arguments.

    On Windows the default (MSVC) toolchain cannot link without Microsoft's build tools.
    When they are missing and the GNU toolchain is installed, that one is used instead.
    Set KILAT_TOOLCHAIN to choose yourself, for example stable-x86_64-pc-windows-gnu.
    """
    chosen = os.environ.get("KILAT_TOOLCHAIN")
    if chosen:
        return ["+" + chosen]
    if sys.platform != "win32" or shutil.which("rustup") is None:
        return []
    listing = subprocess.run(["rustup", "toolchain", "list"], capture_output=True, text=True).stdout
    names = [line.split()[0] for line in listing.splitlines() if line.strip()]
    # rustup marks the default as "(default)" or, since 1.28, "(active, default)".
    default = next(
        (line.split()[0] for line in listing.splitlines() if "default" in line.partition("(")[2]),
        "",
    )
    if "-msvc" not in default or has_microsoft_linker():
        return []
    gnu = next((name for name in names if name.endswith("-pc-windows-gnu")), None)
    if gnu is None:
        fail(
            "the default Rust toolchain needs Microsoft's linker, which is not installed. "
            "Install the Visual C++ build tools, or run: "
            "rustup toolchain install stable-x86_64-pc-windows-gnu"
        )
    print(f"Microsoft's linker was not found; building with {gnu}.")
    return ["+" + gnu]


def build_native():
    run(["cargo", *native_toolchain(), "build", "--release"])
    program = ROOT / "target" / "release" / ("kilat.exe" if sys.platform == "win32" else "kilat")
    if not program.is_file():
        fail(f"{program} was not produced")
    print(f"built {program.relative_to(ROOT)} ({program.stat().st_size:,} bytes)")


def build_wasm():
    run(["cargo", "build", "--release", "--lib", "--target", WASM_TARGET])
    built = ROOT / "target" / WASM_TARGET / "release" / "kilat_core.wasm"
    if not built.is_file():
        fail(f"{built} was not produced")
    shutil.copyfile(built, DOCS / "kilat.wasm")
    print(f"copied docs/kilat.wasm ({built.stat().st_size:,} bytes)")


def write_text(path, text):
    path.write_text(text, encoding="utf-8", newline="\n")
    print(f"wrote {path.relative_to(ROOT).as_posix()} ({len(text):,} characters)")


def version():
    cargo = (ROOT / "Cargo.toml").read_text(encoding="utf-8")
    found = re.search(r'^version\s*=\s*"([^"]+)"', cargo, re.MULTILINE)
    if found is None:
        fail("Cargo.toml has no version")
    return found.group(1)


def build_examples():
    entries = []
    for file, title in EXAMPLES:
        source = (ROOT / file).read_text(encoding="utf-8")
        entries.append({"name": Path(file).stem, "title": title, "source": source})
    listing = json.dumps(entries, indent=2, ensure_ascii=False)
    header = "// Generated by build.py from examples/ and bugs/. Do not edit by hand.\n"
    write_text(DOCS / "examples.js", f"{header}export const examples = {listing};\n")


def build_plugin():
    """Writes main.js: the Cairn plugin, one file with the WebAssembly module inside it."""
    template = (ROOT / "cairn" / "main.template.js").read_text(encoding="utf-8")
    glue = (DOCS / "kilat.js").read_text(encoding="utf-8")
    exported = "export async function loadKilat"
    if glue.count(exported) != 1 or len(re.findall(r"^export\b", glue, re.MULTILINE)) != 1:
        fail("docs/kilat.js is expected to export exactly one thing, loadKilat")
    glue = glue.replace(exported, "async function loadKilat")
    packed = base64.b64encode(zlib.compress((DOCS / "kilat.wasm").read_bytes(), 9)).decode("ascii")
    text = template
    for placeholder, value in (
        ("KILAT_VERSION", version()),
        ("/*KILAT_GLUE*/", glue.strip()),
        ("KILAT_WASM", packed),
    ):
        if text.count(placeholder) != 1:
            fail(f"cairn/main.template.js must contain {placeholder} exactly once")
        text = text.replace(placeholder, value)
    if len(text) > PLUGIN_LIMIT:
        fail(f"main.js would be {len(text):,} characters; Cairn accepts at most {PLUGIN_LIMIT:,}")
    write_text(ROOT / "main.js", text)


def build_web():
    build_examples()
    build_plugin()


STEPS = {"native": build_native, "wasm": build_wasm, "web": build_web}


def main():
    wanted = sys.argv[1:] or list(STEPS)
    unknown = [name for name in wanted if name not in STEPS]
    if unknown:
        fail(f"unknown step {unknown[0]!r}; choose from {', '.join(STEPS)}")
    for name in wanted:
        STEPS[name]()


if __name__ == "__main__":
    main()
