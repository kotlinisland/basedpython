#!/usr/bin/env python3
"""Ecosystem test runner for basedpython.

For each project in ecosystem_projects.toml:
  1. Clone the repo at the pinned ref (shallow).
  2. Run `by transpile` on every .py file; report files that error.

With --round-trip:
  3. Run `by transpile --reverse` then `by transpile` on each file.
  4. Compare ASTs of original and round-tripped output.

Usage:
    python scripts/ecosystem_test.py                   # transpile-only
    python scripts/ecosystem_test.py --round-trip      # + AST round-trip check
    python scripts/ecosystem_test.py --project requests # single project
    python scripts/ecosystem_test.py --by path/to/by   # custom binary
"""

from __future__ import annotations

import argparse
import ast
import subprocess
import sys
import tempfile
from concurrent.futures import ThreadPoolExecutor, as_completed

import tomllib
from pathlib import Path

SCRIPT_DIR = Path(__file__).parent
PROJECTS_CONFIG = SCRIPT_DIR / "ecosystem_projects.toml"


def load_projects(only: str | None) -> list[dict]:
    with open(PROJECTS_CONFIG, "rb") as f:
        config = tomllib.load(f)
    projects = config["projects"]
    if only:
        projects = [p for p in projects if p["name"] == only]
        if not projects:
            sys.exit(f"error: no project named {only!r} in {PROJECTS_CONFIG}")
    return projects


def clone_project(project: dict, dest: Path) -> None:
    url = project["url"]
    ref = project.get("ref", "main")
    subprocess.run(
        ["git", "clone", "--depth=1", "--branch", ref, url, str(dest)],
        check=True,
        capture_output=True,
    )


def collect_py_files(root: Path, subpaths: list[str] | None) -> list[Path]:
    search_roots = [root / p for p in subpaths] if subpaths else [root]
    files = []
    for sr in search_roots:
        files.extend(sr.rglob("*.py"))
    # Skip files in .git, __pycache__, build artefacts.
    return [
        f for f in files
        if not any(part.startswith(".") or part in {"__pycache__", "build", "dist"}
                   for part in f.parts)
    ]


def run_by(by: str, args: list[str], source: str) -> tuple[bool, str]:
    """Run `by transpile [args]` with source on stdin.  Returns (ok, output)."""
    result = subprocess.run(
        [by, "transpile", *args],
        input=source,
        capture_output=True,
        text=True,
    )
    return result.returncode == 0, result.stdout if result.returncode == 0 else result.stderr


def asts_equivalent(a: str, b: str) -> bool:
    """Return True if both strings parse to structurally identical Python ASTs."""
    try:
        tree_a = ast.parse(a)
        tree_b = ast.parse(b)
        return ast.dump(tree_a) == ast.dump(tree_b)
    except SyntaxError:
        return False


def test_file(path: Path, by: str, round_trip: bool) -> list[str]:
    """Return a list of failure strings (empty = pass)."""
    source = path.read_text(encoding="utf-8", errors="replace")
    failures = []

    ok, out = run_by(by, [], source)
    if not ok:
        failures.append(f"transpile error: {out.strip()}")
        return failures  # can't round-trip if forward fails

    if round_trip:
        ok_rev, reversed_src = run_by(by, ["--reverse"], source)
        if not ok_rev:
            failures.append(f"reverse error: {reversed_src.strip()}")
            return failures

        ok_fwd, round_tripped = run_by(by, [], reversed_src)
        if not ok_fwd:
            failures.append(f"round-trip forward error: {round_tripped.strip()}")
            return failures

        if not asts_equivalent(out, round_tripped):
            failures.append("round-trip AST mismatch")

    return failures


def test_project(project: dict, by: str, round_trip: bool) -> dict:
    name = project["name"]
    subpaths = project.get("paths")
    results = {"name": name, "errors": 0, "total": 0, "failures": []}

    with tempfile.TemporaryDirectory(prefix=f"bpeco_{name}_") as tmpdir:
        dest = Path(tmpdir) / name
        try:
            clone_project(project, dest)
        except subprocess.CalledProcessError as e:
            results["failures"].append(f"clone failed: {e.stderr.decode()[:200]}")
            return results

        files = collect_py_files(dest, subpaths)
        results["total"] = len(files)

        for path in files:
            rel = path.relative_to(dest)
            file_failures = test_file(path, by, round_trip)
            for msg in file_failures:
                results["failures"].append(f"{rel}: {msg}")
            if file_failures:
                results["errors"] += 1

    return results


def main() -> None:
    parser = argparse.ArgumentParser(description="basedpython ecosystem tests")
    parser.add_argument("--project", metavar="NAME", help="test only this project")
    parser.add_argument("--round-trip", action="store_true", help="enable AST round-trip checks")
    parser.add_argument("--by", default="by", metavar="PATH", help="path to the `by` binary")
    parser.add_argument("--jobs", type=int, default=4, metavar="N", help="parallel projects")
    args = parser.parse_args()

    projects = load_projects(args.project)
    print(f"testing {len(projects)} project(s) with `{args.by}`")
    if args.round_trip:
        print("round-trip AST check enabled")
    print()

    all_results = []
    with ThreadPoolExecutor(max_workers=args.jobs) as pool:
        futures = {
            pool.submit(test_project, p, args.by, args.round_trip): p["name"]
            for p in projects
        }
        for future in as_completed(futures):
            result = future.result()
            all_results.append(result)
            name = result["name"]
            errors = result["errors"]
            total = result["total"]
            if errors:
                print(f"FAIL  {name}: {errors}/{total} files failed")
                for msg in result["failures"][:10]:
                    print(f"      {msg}")
                if len(result["failures"]) > 10:
                    print(f"      ... and {len(result['failures']) - 10} more")
            else:
                print(f"ok    {name}: {total} files")

    total_errors = sum(r["errors"] for r in all_results)
    total_files = sum(r["total"] for r in all_results)
    print()
    print(f"{'FAIL' if total_errors else 'ok'}  {total_errors} error(s) across {total_files} files")
    sys.exit(1 if total_errors else 0)


if __name__ == "__main__":
    main()
