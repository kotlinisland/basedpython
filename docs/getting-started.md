# getting started

## installation

```sh
uv add --dev basedpython
```

this installs the `by` CLI. verify it works:

```sh
by --help
```

## your first file

basedpython source files use the `.by` extension. Create `main.by`:

```bython
message = "hello"
print(message)
```

run it directly:

```sh
by run main
```

`by run main` finds `main.by` in the current directory, transpiles it (and all other `.by` files in the project) to a temporary directory, then executes `python -m main` from there.

## project layout

a typical basedpython project looks like:

```
myproject/
├── main.by
├── utils.by
├── out/          # transpiled .py output — gitignore this
└── pyproject.toml
```

add the output directory to `.gitignore`:

```
out/
```

## building

`by build` transpiles all `.by` files in the project and writes the output to `out/`, mirroring the source structure:

```sh
by build
```

```
main.by -> out/main.py
utils.by -> out/utils.py

build complete (2 files)
```

the generated `.py` files are ordinary Python. Run them with any Python tool:

```sh
python out/main.py
pytest out/
mypy out/
ruff check out/
```

type checkers and linters operate on the generated Python. if your editor shows type errors in `.by` files, point it at the corresponding `.py` output instead

## CI integration

```yaml
- name: Build
  run: |
    uv add --dev basedpython
    by build

- name: Test
  run: pytest out/
```

## low-level: single file transpilation

`by transpile` is the low-level command for single-file transforms. it reads a file (or stdin) and writes the transpiled Python to stdout:

```sh
by transpile hello.by
echo 'x[(a, b)]' | by transpile
# → x[(a, b),]
```

overwrite the source file in-place:

```sh
by transpile --in-place hello.by
```
