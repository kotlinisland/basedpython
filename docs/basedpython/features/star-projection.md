# type projections

basedpython adds `X[*]` to represent the top materialization of a generic class.
useful as a concise way to denote "any value of `X`, with safety guaranteed"

```by
def f(data: list[*]):
    reveal_type(data[0])  # object
    data[0] = 1  # error
```

this can also be understood as `list[out object]`. the type parameters are taken to their bounds

## semantics

the upper-bound materialization of `T` — every `Any` is replaced by
the maximal concrete type allowed by its variance position. for an invariant
generic like `list[Any]`, the materialization stays as `Top[list[Any]]` (no
single concrete type subsumes every `list[T]`). for covariant positions, `Any`
collapses to `object`; for contravariant, to `Never`

`reveal_type` shows the original surface form in `.by` files:

```by
def _(a: list[*]):
    reveal_type(a)               # revealed: list[*]
```

## scope

`X[*]` is basedpython-only: a `.py` file using `list[*]` produces a parse error
("bare `*` in subscription is not valid in .py files")

## polyfill

`X[*, *, ...]` lowers to `ty_extensions.Top[X[Any, ..., Any]]`.
`Top` is a type-checker-only construct evaluated by `ty`
