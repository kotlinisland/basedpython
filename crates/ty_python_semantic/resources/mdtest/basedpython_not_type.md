# basedpython: `not T` negation operator

`not T` in a type position means "any type except `T`". equivalent to `ty_extensions.Not[T]`
expressed in surface syntax.

```toml
[environment]
python-version = "3.12"
```

## not narrows away the type

```by
def f(x: int | str) -> None:
    if not isinstance(x, int):
        y: not int = x
        reveal_type(y)  # revealed: str
```

## not in return annotation

```by
def make() -> not None:
    return 1

reveal_type(make())  # revealed: not None
```

## not inside a generic

```by
def f(xs: list[not None]) -> None:
    if xs:
        reveal_type(xs[0])  # revealed: not None
```
