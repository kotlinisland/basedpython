#!/usr/bin/env -S uv run --script
#
# /// script
# requires-python = ">=3.11"
# dependencies = ["mypy-primer"]
#
# [tool.uv.sources]
# mypy-primer = { git = "https://github.com/hauntsaninja/mypy_primer" }
# ///

"""Round-trip ecosystem check for the basedpython transpiler.

For every ``.py`` file in a target project, run::

    by transpile --reverse <file>   # python -> basedpython
    by transpile                    # basedpython -> python (via stdin)

and compare the result against the original source. A divergence means
either the reverse transform missed an idiom or the forward transform
isn't a left-inverse of it.

Two modes:

* ``check_ecosystem_roundtrip.py <by> <project-dir>`` — round-trip a
  pre-cloned project on disk.

* ``check_ecosystem_roundtrip.py <by> --primer <name> [--checkout DIR]``
  — fetch a mypy-primer project (using ``setup_primer_project.py``)
  and round-trip it.

The default project list is ``crates/ty_python_semantic/resources/primer/good.txt``.
"""

from __future__ import annotations

import argparse
import asyncio
import difflib
import logging
import sys
import tempfile
from asyncio.subprocess import PIPE, create_subprocess_exec
from collections.abc import Iterable
from contextlib import asynccontextmanager, nullcontext
from pathlib import Path
from signal import SIGINT, SIGTERM
from typing import TYPE_CHECKING, NamedTuple

if TYPE_CHECKING:
    from collections.abc import AsyncIterator

logger = logging.getLogger(__name__)

# directories that don't represent first-party project source
SKIP_PARTS = frozenset(
    {
        ".venv",
        "venv",
        "env",
        ".env",
        "site-packages",
        "__pycache__",
        ".git",
        ".tox",
        ".mypy_cache",
        ".ruff_cache",
        ".pytest_cache",
        "build",
        "dist",
        "node_modules",
    }
)

# pin both passes to the highest supported version so compat lowering
# doesn't add or remove polyfills between the two passes
ROUNDTRIP_MIN_VERSION = "3.15"


class FileResult(NamedTuple):
    path: Path
    ok: bool
    error: str | None
    diff: str


class ProjectResult(NamedTuple):
    name: str
    files_checked: int
    failures: list[FileResult]
    errors: list[FileResult]


def iter_python_files(root: Path) -> Iterable[Path]:
    for p in root.rglob("*.py"):
        if any(part in SKIP_PARTS for part in p.relative_to(root).parts):
            continue
        if p.is_symlink():
            continue
        yield p


async def _run(
    program: Path,
    args: list[str],
    *,
    stdin: bytes | None = None,
    cwd: Path | None = None,
) -> tuple[int, bytes, bytes]:
    proc = await create_subprocess_exec(
        str(program),
        *args,
        stdin=PIPE if stdin is not None else None,
        stdout=PIPE,
        stderr=PIPE,
        cwd=str(cwd) if cwd else None,
    )
    out, err = await proc.communicate(stdin)
    return proc.returncode or 0, out, err


async def roundtrip_file(
    by: Path,
    file: Path,
    project_root: Path,
) -> FileResult:
    """Reverse-transpile then forward-transpile, comparing to the original."""
    rel = file.relative_to(project_root)

    try:
        original = file.read_bytes()
    except OSError as e:
        return FileResult(rel, False, f"read failed: {e}", "")

    # pass 1: python -> basedpython
    rc, reversed_src, err = await _run(
        by,
        ["transpile", "--reverse", "--min-version", ROUNDTRIP_MIN_VERSION, str(file)],
    )
    if rc != 0:
        return FileResult(
            rel, False, f"reverse failed: {err.decode(errors='replace').strip()}", ""
        )

    # pass 2: basedpython -> python (stdin so is_python defaults to false)
    rc, forward_src, err = await _run(
        by,
        ["transpile", "--min-version", ROUNDTRIP_MIN_VERSION],
        stdin=reversed_src,
    )
    if rc != 0:
        return FileResult(
            rel, False, f"forward failed: {err.decode(errors='replace').strip()}", ""
        )

    if forward_src == original:
        return FileResult(rel, True, None, "")

    # decode lazily for the diff; tolerate non-utf8 by replacing
    diff = "".join(
        difflib.unified_diff(
            original.decode("utf-8", errors="replace").splitlines(keepends=True),
            forward_src.decode("utf-8", errors="replace").splitlines(keepends=True),
            fromfile=f"a/{rel}",
            tofile=f"b/{rel}",
            n=2,
        )
    )
    return FileResult(rel, False, None, diff)


async def roundtrip_project(
    by: Path,
    project_root: Path,
    name: str,
    *,
    file_concurrency: int,
) -> ProjectResult:
    files = list(iter_python_files(project_root))
    sem = asyncio.Semaphore(file_concurrency)

    async def task(file: Path) -> FileResult:
        async with sem:
            return await roundtrip_file(by, file, project_root)

    results = await asyncio.gather(*(task(f) for f in files))
    failures = [r for r in results if not r.ok and r.error is None]
    errors = [r for r in results if r.error is not None]
    return ProjectResult(name, len(files), failures, errors)


@asynccontextmanager
async def setup_primer_project(name: str, parent: Path) -> AsyncIterator[Path]:
    """Clone a mypy-primer project into ``parent / name`` if not already there."""
    target = parent / name
    if not target.exists():
        script = Path(__file__).with_name("setup_primer_project.py")
        logger.info("setting up primer project %s in %s", name, target)
        proc = await create_subprocess_exec(
            sys.executable,
            str(script),
            name,
            str(target),
            stdout=PIPE,
            stderr=PIPE,
        )
        _, err = await proc.communicate()
        if proc.returncode != 0:
            raise RuntimeError(
                f"setup_primer_project.py failed for {name!r}:\n"
                f"{err.decode(errors='replace')}"
            )
    yield target


def render_report(results: list[ProjectResult]) -> tuple[str, bool]:
    lines: list[str] = []
    total_files = sum(r.files_checked for r in results)
    total_failures = sum(len(r.failures) for r in results)
    total_errors = sum(len(r.errors) for r in results)

    if total_failures == 0 and total_errors == 0:
        return (
            f"✅ round-trip clean across {total_files} files in {len(results)} projects.",
            True,
        )

    lines.append(
        f"ℹ️ round-trip detected changes "  # noqa: RUF001
        f"(files: {total_files}, divergences: {total_failures}, errors: {total_errors})"
    )
    lines.append("")
    for r in results:
        if not r.failures and not r.errors:
            continue
        lines.append(
            f"### {r.name} — {len(r.failures)} divergence(s), {len(r.errors)} error(s) "
            f"out of {r.files_checked} files"
        )
        for fr in r.errors:
            lines.append(f"- ERROR `{fr.path}`: {fr.error}")
        for fr in r.failures:
            lines.append(f"<details><summary>{fr.path}</summary>")
            lines.append("")
            lines.append("```diff")
            lines.append(fr.diff.rstrip())
            lines.append("```")
            lines.append("")
            lines.append("</details>")
        lines.append("")

    return "\n".join(lines), False


async def main_async(args: argparse.Namespace) -> int:
    by: Path = args.by.resolve()
    if not by.is_file():
        logger.error("by binary not found: %s", by)
        return 2

    project_concurrency = args.project_concurrency
    file_concurrency = args.file_concurrency
    sem = asyncio.Semaphore(project_concurrency)

    async def run_one(name: str, root: Path) -> ProjectResult:
        async with sem:
            logger.info("round-tripping %s", name)
            return await roundtrip_project(
                by, root, name, file_concurrency=file_concurrency
            )

    results: list[ProjectResult] = []

    if args.project_dir is not None:
        root = args.project_dir.resolve()
        results.append(await run_one(root.name, root))
    else:
        # Primer-driven mode
        if args.checkout:
            location_ctx = nullcontext(args.checkout)
        else:
            location_ctx = tempfile.TemporaryDirectory()

        with location_ctx as parent_str:
            parent = Path(parent_str)
            parent.mkdir(parents=True, exist_ok=True)

            primer_names = list(args.primer)
            if not primer_names:
                projects_file = (
                    args.projects
                    or Path(__file__).resolve().parent.parent
                    / "crates/ty_python_semantic/resources/primer/good.txt"
                )
                primer_names = [
                    line.strip()
                    for line in projects_file.read_text().splitlines()
                    if line.strip() and not line.startswith("#")
                ]
                if args.limit:
                    primer_names = primer_names[: args.limit]

            async def setup_and_run(name: str) -> ProjectResult:
                try:
                    async with setup_primer_project(name, parent) as root:
                        return await run_one(name, root)
                except Exception as e:
                    logger.warning("project %s failed setup: %s", name, e)
                    return ProjectResult(
                        name,
                        0,
                        [],
                        [FileResult(Path("."), False, f"setup failed: {e}", "")],
                    )

            results = await asyncio.gather(*(setup_and_run(n) for n in primer_names))

    report, clean = render_report(results)
    print(report)
    return 0 if clean else 1


def parse_args(argv: list[str] | None = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("by", type=Path, help="path to the `by` binary")
    parser.add_argument(
        "project_dir",
        type=Path,
        nargs="?",
        help="pre-cloned project directory to check (skips primer fetch)",
    )
    parser.add_argument(
        "--primer",
        action="append",
        default=[],
        help="mypy-primer project name (repeatable). overrides --projects",
    )
    parser.add_argument(
        "--projects",
        type=Path,
        help="newline-delimited list of mypy-primer project names "
        "(default: crates/ty_python_semantic/resources/primer/good.txt)",
    )
    parser.add_argument(
        "--checkout",
        type=Path,
        help="reuse a directory for primer checkouts (default: temp dir)",
    )
    parser.add_argument(
        "--limit",
        type=int,
        help="limit the number of primer projects to check",
    )
    parser.add_argument("--project-concurrency", type=int, default=4)
    parser.add_argument("--file-concurrency", type=int, default=8)
    parser.add_argument("-v", "--verbose", action="store_true")
    return parser.parse_args(argv)


def main() -> int:
    args = parse_args()
    logging.basicConfig(
        level=logging.DEBUG if args.verbose else logging.INFO,
        format="%(asctime)s %(levelname)s %(message)s",
    )

    if args.checkout:
        args.checkout.mkdir(parents=True, exist_ok=True)

    loop = asyncio.new_event_loop()
    asyncio.set_event_loop(loop)
    main_task = asyncio.ensure_future(main_async(args))
    for sig in (SIGINT, SIGTERM):
        loop.add_signal_handler(sig, main_task.cancel)
    try:
        return loop.run_until_complete(main_task)
    finally:
        loop.close()


if __name__ == "__main__":
    sys.exit(main())
