# auto-quoting

basedpython automatically quotes forward self-references in class definitions, so you don't need `from __future__ import annotations` or manual string literals

## transformation

```python
# basedpython
class A(list[A]):
    children: list[A]
```
```python
# generated Python
class A(list["A"]):
    children: list["A"]
```

## when it applies

the transform applies when the class's own name appears as a subscript slice argument in:

- base class expressions (e.g. `list[A]` in `class A(list[A])`)
- the class body (e.g. `list[A]` in an annotation)

direct base class references (`class A(A):`) are left alone — that is a runtime error regardless of quoting

## motivation

in Python, a class name is not yet bound when its body is being evaluated. using the class name in a generic base class like `class Tree(list[Tree])` raises a `NameError` at runtime. the standard workaround is either `from __future__ import annotations` (which defers all annotation evaluation) or manually writing `list["Tree"]`

basedpython handles this automatically — you write the natural syntax, and the transpiler inserts the quotes where needed
