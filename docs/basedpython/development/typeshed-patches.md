# typeshed patches

basedpython vendors typeshed as `.byi` stubs, regenerated from upstream ruff's
`.pyi` on every sync (see the upstream-sync workflow). the mechanical pipeline —
reverse-transpile, then a pinned pep 695 ruff-fix — recovers most of what
basedpython needs, but a few stubs carry semantics that no mechanical step can
reconstruct. typeshed patches restore those, deterministically, so a fresh sync
always reproduces the committed tree

the `by_typeshed_patch` crate owns this. each patch is a small rust module under
`crates/by_typeshed_patch/src/patches/`, registered in `all_patches()`

## where patches sit in the sync

`scripts/sync_typeshed_by.sh` runs three phases:

1. **reverse-transpile** — every upstream `.pyi` becomes a `.byi`
1. **ast patches** — `by_typeshed_patch` walks the tree and applies every
    registered patch
1. **pep 695 ruff-fix** — `UP046,UP047,UP040 --fix --unsafe-fixes` migrates
    legacy `TypeVar(...)` + `Generic[...]` headers to pep 695 class headers

patches run in phase 2, **before** the ruff-fix. so a patch sees the legacy
form: typevars are declared with `TypeVar(...)` and referenced as plain names in
class bases and method signatures. there are no pep 695 type-parameter lists yet,
and no `out`/`in` variance keywords — those are what phase 3 derives. write
patches against the legacy form

## the `Patch` trait

```rust
pub trait Patch {
    fn name(&self) -> &'static str;
    fn target_symbols(&self) -> &'static [&'static str];
    fn rewrite(&self, module_path: &Path, parsed: &Parsed<ModModule>, source: &str) -> Vec<Edit>;
}
```

- `name` — stable id for logs and drift alerts
- `target_symbols` — qualified symbols the patch depends on, e.g.
    `["typing.Mapping"]`. used for drift detection: if an upstream sync changes one
    of these symbols, the patch is flagged for review
- `rewrite` — return `Edit`s (disjoint byte spans + replacement text) for the
    module at `module_path`, relative to the typeshed `stdlib/` root. an empty vec
    is a no-op for this file

prefer driving edits off the parsed AST rather than raw text scanning — locate
the node, then emit an edit over its range. that keeps a patch precise (exact
identifier matches, correct scoping) and idempotent (re-running over
already-patched source produces no edits)

## worked example: `mapping-key-covariance`

upstream typeshed declares `Mapping` with an invariant key typevar:

```python
class Mapping(Collection[_KT], Generic[_KT, _VT_co]):
    def __getitem__(self, key: _KT, /) -> _VT_co: ...
```

basedpython treats mapping keys as covariant. the covariant typevar `_KT_co` is
already declared in `typing`, so the patch rewrites every `_KT` reference inside
the `Mapping` class to `_KT_co`:

```python
class Mapping(Collection[_KT_co], Generic[_KT_co, _VT_co]):
    def __getitem__(self, key: _KT_co, /) -> _VT_co: ...
```

variance inference can't recover this on its own — `_KT` sits in parameter
position in `__getitem__`, `get`, and friends, which would force an invariant
(or contravariant) reading. the covariance is a deliberate basedpython choice,
so it has to be applied explicitly

the patch walks the module, enters only the `Mapping` class (tracking depth so a
future `sys.version_info` guard wouldn't hide it), and collects the spans of
every `_KT` name within. `MutableMapping`, which needs an invariant key for
`__setitem__`, is a separate class and is left untouched.
`collections.abc.Mapping` and `_collections_abc.Mapping` both re-export
`typing.Mapping`, so the single rewrite covers every surface path

## adding a new patch

1. create `crates/by_typeshed_patch/src/patches/<name>.rs` implementing `Patch`,
    and declare it in `src/patches/mod.rs`
1. register it in `all_patches()` in `crates/by_typeshed_patch/src/lib.rs`
1. write unit tests exercising input → expected output on a minimal snippet,
    plus an idempotency case and a scoping case (what the patch must *not* touch)

after wiring it up, run the patch binary over the real stub to confirm it
reproduces the committed form:

```sh
cargo run --bin by_typeshed_patch -- crates/ty_vendored/vendor/typeshed/stdlib
git diff -- crates/ty_vendored/vendor/typeshed/
```
