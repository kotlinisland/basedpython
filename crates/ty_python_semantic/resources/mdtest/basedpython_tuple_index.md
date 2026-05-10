# basedpython: tuple member dot access

basedpython lets you access tuple elements by position with dot syntax. `expr.N` (where `N` is a
non-negative integer) lowers to `expr[N]`, and ty resolves each access to the type at that index in
the tuple spec.

```toml
[environment]
python-version = "3.12"
```

## fixed tuple — per-index types

```by
def f(pair: tuple[int, str]) -> None:
    reveal_type(pair.0)  # revealed: int
    reveal_type(pair.1)  # revealed: str
```

## tuple literal — element type at the index

```by
reveal_type((1, "a").0)  # revealed: 1
reveal_type((1, "a").1)  # revealed: "a"
```

## chained — `expr.N.M` walks nested tuples

```by
def f(nested: tuple[int, tuple[str, bool]]) -> None:
    reveal_type(nested.1.0)  # revealed: str
    reveal_type(nested.1.1)  # revealed: bool
```

## multi-digit indices

```by
def f(
    ten: tuple[int, int, int, int, int, int, int, int, int, int, str],
) -> None:
    reveal_type(ten.10)  # revealed: str
```

## out of bounds — falls through to normal attribute resolution

```by
def f(pair: tuple[int, str]) -> None:
    pair.5  # error: [unresolved-attribute] "Object of type `(int, str)` has no attribute `5`"
```

## non-tuple value — falls through

```by
x = 1
x.0  # error: [unresolved-attribute] "Object of type `1` has no attribute `0`"
```
