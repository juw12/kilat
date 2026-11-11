"""Installs the Kilat plugin in Cairn's real web app and checks what Cairn draws.

    python tests/cairn_app_test.py <folder holding a checkout of github.com/juw12/cairn>

The checkout's `docs` folder (Cairn's web app) is copied to a work folder, `main.js` is put
beside Cairn's own plugins, and a headless browser opens the app with its demo brain. The page
tests/cairn_app_runner.html then installs the plugin through Cairn's plugin loader and reads
the results off the page. Nothing in the checkout is changed.

What this does not cover: installing over the network from GitHub (the test browser has no
network), and the desktop build of Cairn.
"""

import json
import re
import shutil
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(ROOT / "tests"))

import web_test  # noqa: E402

STAGE = ROOT / "judge_work" / "cairn_stage"

PROGRAMS = {
    "stress": {
        "source": 'print("{250 N / (4 mm * 3 mm):MPa.1}")',
        "output": "20.8 MPa\n",
    },
    "note": {
        "source": 'let load = 1.5 kN * 4\nprint("cairn note ok {load:kN.1}")',
        "output": "cairn note ok 6.0 kN\n",
    },
    "mistake": {
        "source": "let wrong = 2 m + 3 kg\nprint(wrong)",
        "errorHas": "cannot add m and kg",
    },
}


def plugin_version(code):
    found = re.search(r'export const manifest = \{.*?version: "([^"]+)"', code, re.S)
    if not found:
        raise SystemExit("main.js has no manifest version")
    return found.group(1)


def stage(cairn_checkout):
    """Copies Cairn's web app and the plugin into the work folder and returns the plugin text."""
    docs = Path(cairn_checkout) / "docs"
    if not (docs / "plugins.js").is_file():
        raise SystemExit(f"{docs} does not look like Cairn's web app (no plugins.js)")
    if STAGE.exists():
        shutil.rmtree(STAGE)
    shutil.copytree(docs, STAGE)
    # newline="" keeps the text exactly as the browser will receive it.
    with open(ROOT / "main.js", encoding="utf-8", newline="") as handle:
        code = handle.read()
    shutil.copyfile(ROOT / "main.js", STAGE / "plugins" / "kilat.js")
    shutil.copyfile(ROOT / "tests" / "cairn_app_runner.html", STAGE / "kilat_app_runner.html")
    return code


def run(cairn_checkout, timeout=150.0):
    code = stage(cairn_checkout)
    expected = dict(PROGRAMS)
    expected["version"] = plugin_version(code)
    expected["codeLength"] = len(code)
    return web_test.run_in_browser(expected, timeout=timeout, page="kilat_app_runner.html", root=STAGE)


def main():
    if len(sys.argv) != 2:
        raise SystemExit(__doc__)
    report = run(sys.argv[1])
    failed = 0
    for result in report["results"]:
        mark = "ok  " if not result["problems"] else "FAIL"
        print(f"{mark} {result['name']}")
        for problem in result["problems"]:
            print(f"       {problem}")
        failed += bool(result["problems"])
    print(f"{len(report['results']) - failed} of {len(report['results'])} checks passed")
    (ROOT / "judge_work").mkdir(exist_ok=True)
    (ROOT / "judge_work" / "cairn_app_report.json").write_text(json.dumps(report, indent=1), encoding="utf-8")
    return 1 if failed or not report["results"] else 0


if __name__ == "__main__":
    sys.exit(main())
