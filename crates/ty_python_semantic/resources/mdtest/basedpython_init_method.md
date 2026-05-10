# basedpython: `init(...)` method shorthand

basedpython lets a class declare its constructor with `init(...)` instead of `def __init__(...)`.
Parameters prefixed with `let` are auto-assigned to `self.<name>`, giving the class an instance
attribute of the annotated type. The transpiler lowers `init` to `def __init__` and emits the
self-assignments at the top of the method body.

## bodyless `init` with `let` params

```by
class A:
    init(self, let a: int, b: str)

x = A(1, "y")
reveal_type(x.a)  # revealed: int
```

## `init` with explicit body

```by
class A:
    init(self, a: int):
        self.b = str(a)

x = A(1)
reveal_type(x.b)  # revealed: str
```

## `let` parameter inside body-bearing `init`

```by
class A:
    init(self, let a: int):
        self.b = a * 2

x = A(5)
reveal_type(x.a)  # revealed: int
reveal_type(x.b)  # revealed: int
```

## multiple `let` parameters

```by
class Point:
    init(self, let x: int, let y: int)

p = Point(1, 2)
reveal_type(p.x)  # revealed: int
reveal_type(p.y)  # revealed: int
```

## non-`let` parameters are not attributes

A parameter without `let` is just a parameter — no `self.<name>` is created for it.

```by
class A:
    init(self, let a: int, b: str)

x = A(1, "y")
# `b` is a parameter, not an attribute
x.b  # error: [unresolved-attribute]
```
