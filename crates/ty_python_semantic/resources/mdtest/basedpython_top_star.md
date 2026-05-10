# basedpython: `X[*]` shorthand for `Top[X[Any]]`

basedpython provides `X[*]` as sugar for `Top[X[Any]]` — the top materialization of a generic class
parameterized with `Any`. This is useful as a concise way to denote "any specialization of `X`",
particularly for invariant generic classes where the top materialization cannot simplify further.

## list

```by
def _(a: list[*]):
    reveal_type(a)  # revealed: list[*]
```

## dict

```by
def _(a: dict[*, *]):
    reveal_type(a)  # revealed: dict[*, *]
```

## in function signature

```by
def f(x: list[*]) -> set[*]:
    return set()

reveal_type(f)  # revealed: def f(x: list[*]) -> set[*]
```

## in union

```by
def _(a: list[*] | int):
    reveal_type(a)  # revealed: list[*] | int
```

## mixed concrete + star

`*` can be interleaved with concrete type arguments — each `*` is `Any` for its positional typevar:

```by
def _(data: dict[str, *]):
    reveal_type(data)  # revealed: dict[str, *]

def _(data: dict[*, int]):
    reveal_type(data)  # revealed: dict[*, int]
```

## equivalent to explicit Top form

```by
from ty_extensions import Top
from typing import Any

def _(a: list[*], b: Top[list[Any]]):
    reveal_type(a)  # revealed: list[*]
    reveal_type(b)  # revealed: list[*]
```

## star nested in union

`*` can appear as one branch of a type-position union. The whole subscript is top-materialized:

```by
def _(a: list[int | *]):
    reveal_type(a[0])  # revealed: object
    a[0] = ""  # error: [invalid-assignment]
    a[0] = 1

def _(a: list[* | int]):
    reveal_type(a[0])  # revealed: object

def _(a: list[int | * | str]):
    reveal_type(a[0])  # revealed: object

def _(a: dict[str, int | *]):
    reveal_type(a["k"])  # revealed: object
```

## Top and Bottom nested in type position

`ty_extensions.Top` and `Bottom` are normally used as wrappers (`Top[X]`, `Bottom[X]`). basedpython
also accepts them in nested type positions inside a subscript slice, where they trigger
top/bottom-materialization of the enclosing subscript:

```by
from ty_extensions import Top, Bottom

def _(a: list[int | Top]):
    reveal_type(a[0])  # revealed: object
    a[0] = ""  # error: [invalid-assignment]
    a[0] = 1

def _(a: list[int | Bottom]):
    reveal_type(a[0])  # revealed: int
    a[0] = ""  # error: [invalid-assignment]
    a[0] = 1

def _(a: list[Top]):
    reveal_type(a[0])  # revealed: object

def _(a: list[Bottom]):
    reveal_type(a[0])  # revealed: Never

def _(a: dict[str, int | Top]):
    reveal_type(a["k"])  # revealed: object
```

Nested subscripts bind the marker to the innermost enclosing subscript only:

```by
from ty_extensions import Top

def _(a: list[dict[int, Top]]):
    reveal_type(a[0])  # revealed: dict[int, *]
```

Bare `Top` / `Bottom` outside any subscript still errors as today (one argument required).
