# basedpython: `&` intersection operator

basedpython exposes intersections via the `&` operator in type positions. ty narrows the operand
types intersectionally; the same semantics as `ty_extensions.Intersection[A, B]` but expressed as
surface syntax.

```toml
[environment]
python-version = "3.12"
```

## simple two-type intersection

```by
class P: ...
class Q: ...

def f(x: P & Q) -> None:
    reveal_type(x)  # revealed: P & Q
```

## intersection of attribute presence

```by
class HasA:
    a: int

class HasB:
    b: str

def f(x: HasA & HasB) -> tuple[int, str]:
    reveal_type(x.a)  # revealed: int
    reveal_type(x.b)  # revealed: str
    return (x.a, x.b)
```

## intersection inside a generic

```by
class A: ...
class B: ...

def f(items: list[A & B]) -> None:
    if items:
        reveal_type(items[0])  # revealed: A & B
```

## three-arm intersection

```by
class A: ...
class B: ...
class C: ...

def f(x: A & B & C) -> None:
    reveal_type(x)  # revealed: A & B & C
```
