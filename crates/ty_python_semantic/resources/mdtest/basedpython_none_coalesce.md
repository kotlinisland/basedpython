# basedpython: `??` none-coalesce operator

`a ?? b` evaluates to `a` if `a is not None`, otherwise `b`. the result type is `T | U` where `T` is
the non-None portion of `a`'s type and `U` is `b`'s type.

```toml
[environment]
python-version = "3.12"
```

## simple coalesce with plain names

```by
def f(maybe: int | None, fallback: int) -> int:
    result = maybe ?? fallback
    reveal_type(result)  # revealed: int
    return result
```

## non-None literal short-circuits

```by
def f() -> int:
    reveal_type(5 ?? 10)  # revealed: 5 | 10
    return 5 ?? 10
```

## chained coalesce

```by
def f(a: int | None, b: int | None, c: int) -> int:
    result = a ?? b ?? c
    reveal_type(result)  # revealed: int
    return result
```
