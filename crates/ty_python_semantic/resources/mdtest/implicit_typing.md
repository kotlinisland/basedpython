# basedpython: implicit typing imports

In basedpython, members of the `typing` module are implicitly available without explicit imports.
Referencing a name like `Sequence`, `Optional`, or `Mapping` resolves as if `from typing import X`
were present. The transpiler inserts the matching import in the emitted Python.

## ABC reference resolves without explicit import

```by
def f(items: Sequence[int]) -> int:
    return items[0]

reveal_type(f([1, 2, 3]))  # revealed: int
```

## generic helper

```by
def f(x: Optional[int]) -> None:
    reveal_type(x)  # revealed: int | None
```

## multiple typing references in one file

```by
def f(x: Mapping[str, int], y: Sequence[int]) -> Iterable[int]:
    yield from x.values()
    yield from y

reveal_type(f({"a": 1}, [2, 3]))  # revealed: Iterable[int]
```

## existing import wins

A user-provided import binds the name; the implicit lookup is bypassed.

```by
from collections.abc import Sequence

def f(x: Sequence[int]) -> int:
    return x[0]

reveal_type(f([1, 2, 3]))  # revealed: int
```

## user-defined binding wins

A module-level binding shadows the implicit `typing` lookup.

```by
Sequence = 5
reveal_type(Sequence)  # revealed: 5
```

## syntax-covered names are not implicit

`Callable`, `Protocol`, `Generic`, `NewType`, `Final`, `ClassVar`, `Literal`, `TypeIs`, `TypeVar`,
`ParamSpec`, `TypeVarTuple`, `Unpack`, `NamedTuple`, `TypedDict`, and `TypeAlias` are excluded —
they have dedicated basedpython syntax.

```by
# error: [unresolved-reference]
x = Protocol
# error: [unresolved-reference]
y = NewType
# error: [unresolved-reference]
z = Generic
```

## runtime helpers are not implicit

`cast`, `get_type_hints`, `overload`, `assert_type`, and similar runtime helpers must be imported
explicitly.

```by
# error: [unresolved-reference]
x = cast
# error: [unresolved-reference]
y = get_type_hints
```

## version-gated names

`Self`, `LiteralString`, `Never`, `Required`, `NotRequired`, and `ReadOnly` are all available
implicitly. On older Python targets the transpiler emits the import from `typing_extensions`.

```by
class C:
    def clone(self) -> Self:
        return self

reveal_type(C().clone())  # revealed: C
```
