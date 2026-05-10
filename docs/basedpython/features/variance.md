# typevar variance keywords

basedpython adds `in` and `out` keywords on PEP 695 type parameters to
declare variance directly at the declaration site:

```by
class Source[out T]: ...
class Sink[in T]: ...
class Both[in out T]: ...
```

`out T` declares `T` covariant, `in T` contravariant, and `in out T`
bivariant. variance affects subtyping in the obvious way:

- `Source[Dog]` is assignable to `Source[Animal]` (covariant — `T` is
    produced)
- `Sink[Animal]` is assignable to `Sink[Dog]` (contravariant — `T` is
    consumed)

## transpilation

on Python 3.12+ the keywords are stripped because PEP 695 itself does not
yet support inline variance declarations:

```python
class Source[T]: ...
class Sink[T]: ...
class Both[T]: ...
```

on pre-3.12 targets the keywords are passed through to the `TypeVar` polyfill,
which emits the corresponding `covariant=True` / `contravariant=True`
arguments:

```python
_T = TypeVar("_T", covariant=True)
class Source(Generic[_T]): ...

_T_contra = TypeVar("_T_contra", contravariant=True)
class Sink(Generic[_T_contra]): ...
```

## scope

variance keywords are recognized in two surface positions:

1. on a PEP 695 type-parameter declaration (`class C[out T]:`), as
    shown above — affects the **declared** variance of `T`
1. on a subscript argument (`list[out int]`), described below — affects
    only **this one annotation** without touching `list`'s declaration

they are not allowed on bare `TypeVar(...)` calls (use the `covariant=` /
`contravariant=` arguments directly there).

## use-site variance

writing `Container[out T]`, `Container[in T]`, or `Container[in out T]`
gives an annotation a read-only, write-only, or read-write view over a
generic container, without affecting the container's own declared
variance:

```by
def read(data: list[out int]):
    data[0]        # int
    data[0] = 1    # error — write rejected

def write(data: list[in int]):
    data[0] = 1    # ok — int accepted
    data[0]        # error — read rejected

def both(data: list[in out int]):
    data[0]        # int
    data[0] = 1    # ok
```

the outer container's nominal identity is dropped on purpose — only the
variance-restricted view that the surface form promises survives. that
makes `list[out int]` and `set[out int]` evaluate to the same view type.

only single-argument subscripts are supported; multi-argument variance
(e.g. `dict[K, out V]`) reports a transpile error.
