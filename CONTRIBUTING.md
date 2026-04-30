# contributing

## prerequisites

- **Rust** (stable toolchain)
- **Python 3.10+** (for running transpiled output)
- **uv** (recommended for Python dependency management)

## building

basedpython is a Rust project managed with Cargo:

```sh
cargo build
```

the binary is built to `target/debug/by`. for a release build:

```sh
cargo build --release
```

## running tests

```sh
cargo test
```

- **inline tests** — most transform files in `src/transforms/` have `#[cfg(test)]` modules at the bottom with unit tests that call `transpile()` on a snippet and assert the output
- **end-to-end tests** — `tests/e2e.rs` runs larger integration scenarios

## project structure

```
src/
├── main.rs              # CLI entry point (clap argument parsing)
├── lib.rs               # transpile() — orchestrates the full pipeline
├── args.rs              # CLI argument definitions
├── config.rs            # Config struct and PythonVersion enum
├── source_map.rs        # source map generation
├── symbol_table.rs      # module-level symbol collection
├── transforms/          # forward transforms (basedpython → Python)
│   ├── mod.rs
│   ├── subscript.rs
│   ├── mutable_defaults.rs
│   ├── generics.rs
│   ├── modifiers.rs
│   ├── coalesce.rs
│   ├── none_chain.rs
│   └── ...
└── reverse_transforms/  # reverse transforms (Python → basedpython)
    ├── mod.rs
    ├── empty_class.rs
    ├── literal_types.rs
    └── subscript.rs
crates/                  # forked ruff/ty crates
docs/                    # documentation pages
tests/                   # end-to-end tests
```

## adding a new transform

1. **create the transform file** in `src/transforms/`, e.g. `my_feature.rs`

2. **define a struct** that holds the source text and a `Vec<(TextRange, String)>` for edits:

   ```rust
   pub struct MyFeature<'src> {
       source: &'src str,
       pub edits: Vec<(ruff_text_size::TextRange, String)>,
   }
   ```

3. **implement `Visitor`** from `ruff_python_ast::visitor` — walk the AST nodes you care about and push edits:

   ```rust
   impl Visitor<'_> for MyFeature<'_> {
       fn visit_stmt(&mut self, stmt: &Stmt) {
           // detect your pattern, push to self.edits
           walk_stmt(self, stmt);
       }
   }
   ```

4. **register the transform** in `src/transforms/mod.rs` and wire it into the pipeline in `src/lib.rs`:
   - instantiate it alongside the other transforms
   - call `visitor.visit_stmt(stmt)` in the main loop
   - extend the edits vec with your transform's edits
   - if your transform needs generated imports (e.g. `from typing import TypeVar`), integrate with the **preamble system** — after all edits are applied, `lib.rs` collects import requests from transforms and prepends them to the output file. see how `generics.needed_imports` or `literal_types.needs_literal_import` for examples

5. **add tests** in a `#[cfg(test)]` module at the bottom of your file:

   ```rust
   #[cfg(test)]
   mod tests {
       use crate::{Config, transpile};

       fn t(input: &str) -> String {
           transpile(input, &Config::default()).unwrap()
       }

       #[test]
       fn basic() {
           assert_eq!(t("input code"), "expected output");
       }
   }
   ```

6. **add a doc page** in `docs/` describing the syntax and showing examples

## running the docs locally

the documentation site is built with [zensical](https://github.com/kotlinisland/zensical). to view it locally, use:

```sh
uv run zensical serve
```

this starts a local dev server (usually at `http://localhost:8000`) with live reload. pages are in `docs/` and the site configuration is in `zensical.toml`

## adding a reverse transform

reverse transforms live in `src/reverse_transforms/` and follow the same pattern. they detect standard Python patterns that correspond to basedpython idioms and rewrite them back. register new reverse transforms in `src/reverse_transforms/mod.rs`
