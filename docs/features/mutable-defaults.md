# mutable default arguments

basedpython automatically rewrites mutable default arguments to the sentinel pattern, preventing the classic Python gotcha where mutable defaults are shared across calls

## the problem

in Python, default argument values are evaluated once at function definition time. mutable defaults like `[]` or `{}` are shared between all calls:

```python
def append(item, items=[]):
    items.append(item)
    return items

append(1)  # [1]
append(2)  # [1, 2] — not [2]!
```

## the fix

basedpython detects mutable default arguments (`[]`, `{}`, `set()`, etc.) and rewrites them using a sentinel object:

```python
# basedpython
def f(x=[], y={}):
    pass
```
```python
# generated Python
_MISSING = object()
def f(x=_MISSING, y=_MISSING):
    if x is _MISSING:
        x = []
    if y is _MISSING:
        y = {}
    pass
```

each call gets a fresh mutable object, matching the behavior most programmers expect

## what triggers the rewrite

the rewrite applies when a default value is a mutable literal:

- list literals: `[]`, `[1, 2]`
- dict literals: `{}`, `{"a": 1}`
- set literals: `{1, 2}`

immutable defaults like `None`, `0`, `""`, `()`, and `frozenset()` are left unchanged
