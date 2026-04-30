# intersection types

basedpython uses the `&` operator in type annotations to express intersection types

## transformation rules

| basedpython | Python output |
|---|---|
| `a: A & B` | `a: Intersection[A, B]` |
| `a: A & B & C` | `a: Intersection[A, B, C]` |
| `a: (A & B) \| C` | `a: Intersection[A, B] \| C` |
| `a: list[A & B]` | `a: list[Intersection[A, B]]` |

`Intersection` is imported from `ty_extensions` automatically when needed

## examples

basic intersection:

```python
# basedpython
a: A & B
```
```python
# generated Python
from ty_extensions import Intersection
a: Intersection[A, B]
```

three-way intersection:

```python
# basedpython
a: A & B & C
```
```python
# generated Python
from ty_extensions import Intersection
a: Intersection[A, B, C]
```

combined with union types:

```python
# basedpython
a: (A & B) | C
```
```python
# generated Python
from ty_extensions import Intersection
a: Intersection[A, B] | C
```

nested inside generic types:

```python
# basedpython
a: list[A & B]
```
```python
# generated Python
from ty_extensions import Intersection
a: list[Intersection[A, B]]
```

## annotation context only

the `&` rewrite applies only in type annotation positions. bitwise-AND in value expressions and augmented assignments is never affected:

```python
x = A & B    # unchanged — value expression
x &= B      # unchanged — augmented assignment
```
