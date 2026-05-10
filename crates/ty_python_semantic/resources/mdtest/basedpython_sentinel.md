# basedpython: sentinel declarations

`sentinel NAME` declares a module-level sentinel. it lowers to `NAME = Sentinel("NAME")` (where
`Sentinel` is the `typing_extensions` polyfill of [PEP 661](https://peps.python.org/pep-0661/)). ty
treats the declared target as unknown — the declaration is accepted without diagnostics and does not
require a value

```toml
[environment]
python-version = "3.12"
```

## simple

```by
sentinel MISSING
```

## multiple in same module

```by
sentinel MISSING
sentinel UNSET
```

## not a name reference

the `sentinel` keyword is not a name lookup — there is no spurious "undefined name" diagnostic

```by
sentinel A
```
