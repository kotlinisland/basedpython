# basedpython: `is` / `is not` keyword narrowing

In basedpython, the `is` and `is not` keyword pair perform instance checks (they transpile to
`isinstance(...)` / `not isinstance(...)`). The `===` and `!==` operators retain Python's identity
comparison semantics. Narrowing in `.by` files mirrors this swap.

## `is not` narrows to negation of the instance type

```by
def f(a: object):
    if a is not int:
        reveal_type(a)  # revealed: not int
```

## `is` narrows to the instance type

```by
def f(a: object):
    if a is int:
        reveal_type(a)  # revealed: int
```

## `!==` keeps Python identity semantics

```by
def f(a: object):
    if a !== None:
        reveal_type(a)  # revealed: not None
```

## `===` keeps Python identity semantics

```by
def f(a: int | None):
    if a === None:
        reveal_type(a)  # revealed: None
```

## `is` with literal RHS keeps Python identity semantics

`isinstance(x, None)` is invalid at runtime, so `is`/`is not` against literal singletons (`None`,
`True`/`False`, numbers, strings, bytes, `...`) must transpile as Python `is`/`is not` rather than
`isinstance`.

```by
def f(a: int | None):
    if a is None:
        reveal_type(a)  # revealed: None
    if a is not None:
        reveal_type(a)  # revealed: int
```

```by
def f(a: bool | int):
    if a is True:
        reveal_type(a)  # revealed: True
    if a is False:
        reveal_type(a)  # revealed: False
```

```by
def f(a: int | None):
    if a is ...:
        reveal_type(a)  # revealed: Never
```
