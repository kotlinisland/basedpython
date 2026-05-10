# basedpython: `?.` optional chaining

`a?.b` short-circuits to `None` when `a is None`, otherwise evaluates `a.b`. the result type is the
attribute type unioned with `None`.

```toml
[environment]
python-version = "3.12"
```

## simple attribute chain

```by
class C:
    name: str

def f(c: C | None) -> None:
    result = c?.name
    reveal_type(result)  # revealed: str | None
```

## composes with `??`

```by
class C:
    name: str

def f(c: C | None) -> str:
    return c?.name ?? "anonymous"
```
