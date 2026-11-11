"""Kilat for Python: run and check Kilat programs through the kilat_core library.

Needs nothing but Python 3.8+ and the library built by `python build.py`
(kilat_core.dll on Windows, libkilat_core.so on Linux, libkilat_core.dylib on macOS).

    import kilat

    result = kilat.run('print("stress =", 250 N / (4 mm * 3 mm) in MPa)')
    print(result.output)                              # stress = 20.8333333333

    print(kilat.check("let gap: mm = 2 s").error)     # a unit error, found without running

A Kilat program runs in a sandbox. It has a fuel limit and a memory limit, and it cannot
touch files, the network or anything else unless you pass a `host` function:

    def host(name, payload):
        if name == "read_file":
            return open(payload, encoding="utf-8").read()
        return None                                   # everything else stays refused

    kilat.run(source, host=host)

`host` returns a string to answer `Ok(text)`, returns None to refuse, or raises an
exception to answer `Err(message)`. The requests a program can make are read_file,
write_file (payload "path\ntext"), http_get, ask_ai, input, clock, time and args.
"""

from __future__ import annotations

import ctypes
import os
import sys
import time
from dataclasses import dataclass

__all__ = ["run", "check", "version", "load", "Result"]

_ERROR_KINDS = {0: "", 1: "compile", 2: "runtime", 3: "fuel", 4: "memory"}
_OUTPUT, _ERROR, _HOST_NAME, _HOST_PAYLOAD, _VERSION = 0, 1, 2, 3, 4
_DONE, _HOST_CALL, _MORE, _FAILED = 0, 1, 2, 3
_SLICE = 2_000_000.0

_library = None


@dataclass(frozen=True)
class Result:
    """What a run or a check produced. `bool(result)` is True when there was no error."""

    ok: bool
    output: str
    error: str
    kind: str  # "", "compile", "runtime", "fuel" or "memory"
    fuel_used: int

    def __bool__(self) -> bool:
        return self.ok


def _library_name() -> str:
    if sys.platform == "win32":
        return "kilat_core.dll"
    if sys.platform == "darwin":
        return "libkilat_core.dylib"
    return "libkilat_core.so"


def _candidates():
    name = _library_name()
    here = os.path.dirname(os.path.abspath(__file__))
    if os.environ.get("KILAT_LIB"):
        yield os.environ["KILAT_LIB"]
    yield os.path.join(here, name)
    yield os.path.join(here, "..", "..", "target", "release", name)


def load(path=None):
    """Load the Kilat library. Called for you on first use; pass a path to pick a specific build."""
    global _library
    if path is None:
        if _library is not None:
            return _library
        found = [candidate for candidate in _candidates() if os.path.isfile(candidate)]
        if not found:
            raise FileNotFoundError(
                _library_name() + " was not found. Build it with `python build.py`, "
                "or set the KILAT_LIB environment variable to its full path."
            )
        path = found[0]
    library = ctypes.CDLL(os.path.abspath(path))
    pointer, size, number, real, data = (
        ctypes.c_void_p,
        ctypes.c_size_t,
        ctypes.c_uint32,
        ctypes.c_double,
        ctypes.c_char_p,
    )
    signatures = {
        "kilat_session_new": (pointer, []),
        "kilat_session_free": (None, [pointer]),
        "kilat_add_file": (None, [pointer, data, size, data, size]),
        "kilat_set_seed": (None, [pointer, real]),
        "kilat_start": (number, [pointer, data, size, number, real, real]),
        "kilat_step": (number, [pointer, real]),
        "kilat_resume": (None, [pointer, number, data, size]),
        "kilat_text": (pointer, [pointer, number]),
        "kilat_text_len": (size, [pointer, number]),
        "kilat_clear_output": (None, [pointer]),
        "kilat_error_kind": (number, [pointer]),
        "kilat_fuel_used": (real, [pointer]),
    }
    for name, (result_type, argument_types) in signatures.items():
        function = getattr(library, name)
        function.restype = result_type
        function.argtypes = argument_types
    _library = library
    return library


def _text(library, session, which) -> str:
    length = library.kilat_text_len(session, which)
    if length == 0:
        return ""
    return ctypes.string_at(library.kilat_text(session, which), length).decode("utf-8", "replace")


def _answer(host, name, payload, started):
    """Returns (ok, text) for one request from the program."""
    if host is not None:
        try:
            reply = host(name, payload)
        except Exception as problem:  # the program gets Err(message) and decides what to do
            return False, str(problem) or type(problem).__name__
        if reply is not None:
            return True, str(reply)
    if name == "clock":
        return True, "{:.9f}".format(time.perf_counter() - started)
    if name == "time":
        return True, "{:.6f}".format(time.time())
    if name == "args":
        return True, ""
    return False, "`{}` is not allowed here".format(name)


def _execute(source, mode, fuel, mem_mb, host, files, seed, on_output) -> Result:
    library = load()
    started = time.perf_counter()
    session = library.kilat_session_new()
    try:
        for name, content in (files or {}).items():
            name_bytes = name.encode("utf-8")
            content_bytes = content.encode("utf-8")
            library.kilat_add_file(session, name_bytes, len(name_bytes), content_bytes, len(content_bytes))
        if seed is not None:
            library.kilat_set_seed(session, float(seed))
        source_bytes = source.encode("utf-8")
        memory_bytes = float(mem_mb or 0) * 1024.0 * 1024.0
        status = library.kilat_start(session, source_bytes, len(source_bytes), mode, float(fuel or 0), memory_bytes)
        pieces = []
        while status == 0:
            step = library.kilat_step(session, _SLICE)
            piece = _text(library, session, _OUTPUT)
            if piece:
                pieces.append(piece)
                if on_output is not None:
                    on_output(piece)
                library.kilat_clear_output(session)
            if step == _DONE:
                break
            if step == _FAILED:
                status = 3
            elif step == _HOST_CALL:
                name = _text(library, session, _HOST_NAME)
                payload = _text(library, session, _HOST_PAYLOAD)
                ok, reply = _answer(host, name, payload, started)
                reply_bytes = reply.encode("utf-8")
                library.kilat_resume(session, 1 if ok else 0, reply_bytes, len(reply_bytes))
        kind = _ERROR_KINDS.get(library.kilat_error_kind(session), "runtime")
        error = _text(library, session, _ERROR) if status != 0 else ""
        return Result(
            ok=(status == 0),
            output="".join(pieces),
            error=error,
            kind=kind if status != 0 else "",
            fuel_used=int(library.kilat_fuel_used(session)),
        )
    finally:
        library.kilat_session_free(session)


def run(source, *, fuel=None, mem_mb=None, host=None, files=None, seed=None, print_last=False, on_output=None) -> Result:
    """Check and run a Kilat program.

    fuel        the most loop turns and function calls the program may make (default 200 million)
    mem_mb      the most memory it may use, in MB (default 64)
    host        answers the program's requests for files, the network and so on (see the module text)
    files       {"name.kilat": "source"} made available to `import "name.kilat"`
    seed        a number that makes rand() repeatable
    print_last  also print the value of a final expression, like a calculator
    on_output   called with each piece of printed text while the program runs
    """
    return _execute(source, 2 if print_last else 0, fuel, mem_mb, host, files, seed, on_output)


def check(source, *, files=None) -> Result:
    """Check a Kilat program for errors (names, types, units, missing cases) without running it."""
    return _execute(source, 1, None, None, None, files, None, None)


def version() -> str:
    library = load()
    session = library.kilat_session_new()
    try:
        return _text(library, session, _VERSION)
    finally:
        library.kilat_session_free(session)


def _main(arguments) -> int:
    if len(arguments) != 1 or arguments[0] in ("-h", "--help"):
        print("usage: python kilat.py <file.kilat>     (runs it in the sandbox, without file access)")
        return 2
    with open(arguments[0], encoding="utf-8-sig") as handle:
        source = handle.read()
    out = sys.stdout
    if hasattr(out, "reconfigure"):
        out.reconfigure(encoding="utf-8", newline="\n")
    result = run(source, mem_mb=2048, fuel=1e15, on_output=lambda piece: (out.write(piece), out.flush()))
    if not result.ok:
        print(result.error, file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(_main(sys.argv[1:]))
