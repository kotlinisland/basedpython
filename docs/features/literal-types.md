# literal types

basedpython rewrites literal values in type annotation positions to `typing.Literal`

## transformation rules

| basedpython | Python output |
|---|---|
| `a: 5` | `a: Literal[5]` |
| `a: "asdf"` | `a: Literal["asdf"]` |
| `a: "asdf" \| 5` | `a: Literal["asdf", 5]` |
| `a: 1 \| 2 \| int` | `a: Literal[1, 2] \| int` |
| `a: (int, str)` | `a: tuple[int, str]` |

the `Literal` import from `typing` is added automatically when needed

## examples

single literal:

```python
# basedpython
a: 5
```
```python
# generated Python
from typing import Literal
a: Literal[5]
```

string literal:

```python
# basedpython
a: "asdf"
```
```python
# generated Python
from typing import Literal
a: Literal["asdf"]
```

union of literals — consecutive literal values in a union are merged into a single `Literal`:

```python
# basedpython
a: "asdf" | 5
```
```python
# generated Python
from typing import Literal
a: Literal["asdf", 5]
```

mixed union — non-literal types stay outside the `Literal`:

```python
# basedpython
a: 1 | 2 | int
```
```python
# generated Python
from typing import Literal
a: Literal[1, 2] | int
```

## tuple literal types

parenthesized type tuples in annotation positions are rewritten to `tuple`:

```python
# basedpython
a: (int, str)
```
```python
# generated Python
a: tuple[int, str]
```

## annotation context only

literal rewrites apply only in type annotation positions — variable annotations, function parameter annotations, return types, and subscript slices of type-like names. literal values in regular expressions are never modified
