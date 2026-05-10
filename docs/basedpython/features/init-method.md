# init method shorthand

basedpython lets a class declare its constructor with the bare keyword `init`
instead of `def __init__`. parameters prefixed with `let` are auto-assigned to
`self.<name>` at the top of the method body, so the constructor and the
instance-attribute declarations share a single signature

```by
class Point:
    init(self, let x: int, let y: int)
```

transpiles to:

```python
class Point:
    def __init__(self, x: int, y: int):
        self.x: int = x
        self.y: int = y
```

## scope

`init(...)` is only recognised inside a class body. at module scope
`init(...)` is still an ordinary function call expression. the parser tracks
class nesting so the keyword does not leak into module-level call sites

## bodyless and body forms

both shapes are accepted:

```by
class A:
    init(self, let a: int, b: str)
```

```by
class B:
    init(self, a: int):
        self.b = str(a)
```

a bodyless `init(...)` is filled in with `: ...` when no `let` parameter
produces a body line. body-bearing `init(...)` keeps the user's statements;
`let` self-assignments are prepended ahead of them

## `let` parameter modifier

`let` may appear on any positional, positional-only, keyword-only, `*args`,
or `**kwargs` parameter inside `init`. each `let` parameter emits a
self-assignment using the parameter's annotation (`self.<name>: <ann> = <name>`)
or, if unannotated, a bare assignment (`self.<name> = <name>`)

a non-`let` parameter is just a parameter — no attribute is created for it

## why

constructors with `self.x = x; self.y = y` boilerplate are ubiquitous.
collapsing the parameter list and the attribute declarations into one line
removes that duplication without depending on `@dataclass` semantics
