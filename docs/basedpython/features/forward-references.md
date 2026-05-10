# automatic forward references

a class that refers to itself in a *subscript* base or in a method-signature
annotation triggers a `NameError` at class definition time in standard
Python, because the name is not yet bound. the conventional fix is to quote
the reference: `class Tree(list["Tree"])`. basedpython does the quoting
automatically:

```by
class Tree(list[Tree]):
    children: list[Tree]
    def add(self, child: Tree) -> Tree: ...
```

transpiles to:

```python
class Tree(list["Tree"]):
    children: list["Tree"]
    def add(self, child: "Tree") -> "Tree": ...
```

## scope

quoting fires in three positions inside a class definition:

1. **subscript bases** — `class A(list[A])`, `class T(Node[T | None])`. the
    entire subscript value is quoted (`"T | None"`), not just the
    self-reference, so unions and nested generics work uniformly
1. **class-body annotations** — attribute and method-parameter / return
    types whose textual form contains the class name
1. **inherited generic typevars** that name the class

direct base classes — `class A(A)` — are *not* quoted. that pattern is
always a runtime error and quoting it would only mask the bug. method
*bodies* are also not rewritten: by the time the body executes, the class
name is bound, and quoting would produce a string instead of a value

## why automatic

forward-reference quoting is mechanical: the only information that controls
it is whether the name is the enclosing class. doing it at the transpiler
level lets ty type-check the *unquoted* form (which it already understands)
and emits the quoted form for the runtime

## when quoting is skipped

quoting is only emitted when the annotation would otherwise be evaluated
eagerly. it is skipped when:

- the target is python 3.14 or newer — annotations are deferred natively
    (PEP 649), so the bare name resolves lazily
- the file already defers every annotation through
    `from __future__ import annotations`, whether you wrote it yourself or
    opted into the blanket injection
