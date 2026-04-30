# vendored ruff crates

basedpython vendors six crates from [astral-sh/ruff](https://github.com/astral-sh/ruff)
under `crates/`:

| crate | role |
|---|---|
| `ruff_text_size` | byte-offset and range primitives |
| `ruff_source_file` | source file abstraction |
| `ruff_python_trivia` | comments, whitespace, indentation parsing |
| `ruff_python_ast` | the Python AST we walk and rewrite |
| `ruff_python_parser` | source → AST |
| `ruff_annotate_snippets` | error-message rendering (used by parser tests) |

vendored at SHA `67296f083980792a2e2868efc86fa105d2e565dd`. update this line and
the corresponding entry in `Cargo.toml` after every sync

## why we vendored

basedpython is a superset of Python. extending the grammar (e.g. property
syntax, primary constructors, error type system — see ROADMAP.md) requires
adding tokens, productions, and AST nodes. that work has to live in the
parser/AST crates, not above them, so we own a copy

the alternative — depending on a remote git fork — was rejected because every
grammar change would require a coordinated PR across two repos, and version
pinning makes basedpython development friction-heavy

## conventions

- **crate names match upstream** — keeps `git diff` against upstream
  meaningful and makes merges mechanical. don't rename to `by_*` / `buff_*`
  unless the fork has truly diverged
- **sync commits are separate from feature commits** — every upstream pull
  lands as its own commit titled `vendor: sync ruff @ <sha>`. that keeps
  `git log` honest about what is ours vs. upstream's
- **basedpython grammar additions go to upstream files** — when we add a new
  token or AST node, we edit the vendored ruff source in place rather than
  building a wrapper layer. this keeps the parser fast and avoids divergence
  between basedpython's view of the AST and ruff's
- **don't drop test infrastructure** — the upstream parser ships ~770 tests
  (217 unit, 539 fixture-driven, 10 doctest, etc.). these are how we catch
  parser regressions when our basedpython grammar changes accidentally break
  standard Python parsing. `cargo test --workspace` runs them all

## syncing from upstream

### 1. clone upstream at the new SHA

```sh
git clone --filter=blob:none https://github.com/astral-sh/ruff.git /tmp/ruff
git -C /tmp/ruff checkout <new-sha>
```

### 2. inspect the diff per crate

```sh
for c in ruff_text_size ruff_source_file ruff_python_trivia \
         ruff_python_ast ruff_python_parser ruff_annotate_snippets; do
    echo "=== $c ==="
    diff -ru "crates/$c" "/tmp/ruff/crates/$c" | head -100
done
```

scan for:
- AST node renames or shape changes (these break basedpython visitors)
- new AST nodes (basedpython visitors will silently skip them — usually fine,
  occasionally wrong)
- parser productions touching syntax basedpython extends (highest risk)
- `Cargo.toml` dep version bumps

### 3. merge

for files with no conflicts, copy upstream over ours:

```sh
cp -R /tmp/ruff/crates/<crate>/<path> crates/<crate>/<path>
```

for files where basedpython has extended the upstream code, do a manual
three-way merge. helpful pattern: keep a long-lived branch tracking the last
synced upstream so `git merge-base` works for true 3-way merges, instead of
re-deriving the conflicts every sync

### 4. update workspace

if upstream bumped any of:
- `[workspace.package]` (rust-version, edition)
- `[workspace.dependencies]` versions
- `[workspace.lints]`
- `Cargo.toml` per-crate features or deps

mirror the relevant change into our root `Cargo.toml`. our workspace.lints
and per-crate Cargo.toml overrides are deliberately kept close to upstream's

### 5. verify correctness

run the full workspace test suite:

```sh
cargo test --workspace
```

this should show ~1000 passing tests across:
- basedpython unit + e2e (~100)
- vendored ruff doctests, unit tests, and fixture tests (~900)

then run release-mode parser tests too — these catch optimization-sensitive
issues that don't show up in dev:

```sh
cargo test --workspace --release
```

if any vendored test fails:
- if basedpython's grammar additions changed parser behavior, regenerate the
  affected snapshots with `INSTA_UPDATE=1 cargo test -p ruff_python_parser`
  and review the diff carefully — only accept if the new output is what
  basedpython intends
- otherwise it's an upstream regression — investigate before accepting

if our basedpython tests fail, the AST or parser API likely changed. fix the
calling code in `src/transforms/`

### 6. commit

```sh
git add crates/ Cargo.toml Cargo.lock VENDORING.md
git commit -m "vendor: sync ruff @ <new-sha>"
```

VENDORING.md's SHA reference at the top must be updated in the same commit

## what we strip from upstream

very little. on the initial vendor we removed:

- README and CONTRIBUTING markdown (not needed in a fork)
- `ast.toml` and `generate.py` from `ruff_python_ast` (used by upstream's
  AST codegen workflow, which we don't run — if we extend the AST, we edit
  the generated source directly)

we **keep** `tests/`, `resources/`, `examples/`, and the dev-dependencies
that drive them — this is the regression safety net

## what we don't yet do

- **no automated `cargo deny` / supply-chain check** of vendored deps
- **no CI matrix** running the workspace tests on push (when CI lands, run
  `cargo test --workspace --release` on at least one sync-validation job)
- **no upstream-tracking branch** for true 3-way merges. for now, `diff -ru`
  + manual merge is enough; revisit if syncs become painful
- **no policy on how often to sync**. roughly: when we need a new
  upstream-only feature, when an upstream bug fix matters to us, or every
  ~3 months for hygiene
