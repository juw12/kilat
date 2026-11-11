"""Checks the playground (docs/index.html) and the Cairn plugin (main.js) in a headless browser.

    python tests/page_test.py

The playground is driven in a frame the way a visitor would use it. The plugin is loaded
under the rules of Cairn's plugin loader (one file, at most 400,000 characters, imported
from a blob) against a stand-in for Cairn's plugin API; it is not run inside Cairn itself.
Where the native kilat program has been built, what the pages show is compared with what
it prints. Standard library only; exit code 0 pass, 1 fail, 2 no browser.
"""

import json
import subprocess
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import web_test  # noqa: E402

ROOT = web_test.ROOT
PAGES = (
    ("playground", "tests/playground_runner.html"),
    ("Cairn plugin", "tests/cairn_runner.html"),
)


def playground_examples():
    """The programs in docs/examples.js, which is `export const examples = [...];`."""
    text = (ROOT / "docs" / "examples.js").read_text(encoding="utf-8")
    return json.loads(text[text.index("[") : text.rindex("]") + 1])


def native_result(source):
    """What the native program prints for `source`, or None when it has not been built."""
    native = web_test.native_kilat()
    if native is None:
        return None
    folder = web_test.WORK / "pages"
    folder.mkdir(parents=True, exist_ok=True)
    # The pages call the program main.kilat, so the same name gives the same messages.
    (folder / "main.kilat").write_text(source, encoding="utf-8", newline="\n")
    finished = subprocess.run([str(native), "run", "main.kilat"], cwd=folder, capture_output=True, timeout=60)
    return {
        "output": finished.stdout.decode("utf-8").replace("\r\n", "\n"),
        "error": finished.stderr.decode("utf-8").replace("\r\n", "\n"),
    }


def run_checks(timeout=240.0):
    """Runs both pages; returns ([(page, check, problems)], {timing name: value})."""
    examples = playground_examples()
    for example in examples:
        example["expected"] = native_result(example["source"])
    outcomes = []
    timings = {}
    for label, page in PAGES:
        try:
            report = web_test.run_in_browser({"examples": examples}, timeout=timeout, page=page)
        except (RuntimeError, TimeoutError) as problem:
            outcomes.append((label, "the checks ran", [str(problem)]))
            continue
        timings.update(report.get("timings", {}))
        for result in report["results"]:
            outcomes.append((label, result["name"], result["problems"]))
    return outcomes, timings


def main():
    try:
        outcomes, timings = run_checks()
    except web_test.BrowserMissing as missing:
        print(f"skipped: {missing}")
        return 2
    failed = 0
    for label, name, problems in outcomes:
        if problems:
            failed += 1
            print(f"FAIL  {label}: {name}")
            for problem in problems:
                print(f"        {problem}")
        else:
            print(f"ok    {label}: {name}")
    for name, value in sorted(timings.items()):
        print(f"      {name} = {value}")
    print(f"{len(outcomes) - failed} of {len(outcomes)} page checks passed")
    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main())
