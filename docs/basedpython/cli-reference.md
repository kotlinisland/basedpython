# `by` CLI reference

basedpython ships with two executables: `by` and `buff`. `by` is the basedpython driver — an extension of `ty` and includes the
type-checker, the transpiler, and a few project-level commands

`buff` is the basedpython version of `ruff`

```text
by <command> [args...]
```

in addition to the cli provided by `ty`, `by` includes:

| command             | what it does                                                       |
| ------------------- | ------------------------------------------------------------------ |
| `run`               | transpile and run a module with `python -m <module>`               |
| `build`             | transpile every `.by`/`.byi` file and write to `out/`              |
| `generate-api-file` | write a public-api lockfile (see [api-lock](features/api-lock.md)) |
| `transpile`         | transpile a single file to stdout (reads stdin if no file)         |

## `by run`

```sh
by run MODULE [ARGS...]             # transpile + run with `python -m MODULE`
by run MODULE --min-version 3.12    # target a specific runtime python version
```

equivalent to `by build && python -m MODULE`, but only transpiles the
modules required to import `MODULE`

## `by build`

```sh
by build                            # transpile every .by/.byi under the project root
by build --min-version 3.12         # target a specific runtime python version
```

writes the transpiled python to `./out/` mirroring the source tree. the
`out/` directory is **not** considered first-party source for `by check`
or `by generate-api-file` — it is regenerated on every build

## `by generate-api-file`

```sh
by generate-api-file                       # writes ./api.lock
by generate-api-file --stdout              # writes lockfile to stdout
by generate-api-file -o public.lock        # custom output path
by generate-api-file --python-version 3.10 # target a specific python version
```

see [api-lock](features/api-lock.md) for the lockfile format and workflow

## `by transpile`

```sh
by transpile FILE           # read FILE, write transpiled python to stdout
by transpile                # read from stdin, write to stdout
by transpile FILE --reverse         # convert python source into basedpython idioms
by transpile FILE --min-version 3.12 # target a specific runtime python version
echo 'x: int = 1' | by transpile
```

stops at the first transpile error and prints a diagnostic
