# callable syntax

basedpython lets you write callable types using arrow syntax instead of `typing.Callable`

## transformation rules

| basedpython | Python output |
|---|---|
| `(int) -> int` | `Callable[[int], int]` |
| `(int, str) -> bool` | `Callable[[int, str], bool]` |
| `() -> None` | `Callable[[], None]` |
| `(int) -> (str) -> bool` | `Callable[[int], Callable[[str], bool]]` |

the `Callable` import from `typing` is added automatically when needed

## examples

basic callable annotation:

```python
# basedpython
a: (int) -> int
```
```python
# generated Python
from typing import Callable
a: Callable[[int], int]
```

multiple parameters:

```python
# basedpython
a: (int, str) -> bool
```
```python
# generated Python
from typing import Callable
a: Callable[[int, str], bool]
```

no parameters:

```python
# basedpython
a: () -> None
```
```python
# generated Python
from typing import Callable
a: Callable[[], None]
```

nested callables:

```python
# basedpython
a: (int) -> (str) -> bool
```
```python
# generated Python
from typing import Callable
a: Callable[[int], Callable[[str], bool]]
```

works inside generic types:

```python
# basedpython
a: list[(int) -> int]
```
```python
# generated Python
from typing import Callable
a: list[Callable[[int], int]]
```

## annotation context only

the arrow syntax is only rewritten in type annotation positions. in value expressions, parenthesized tuples are left unchanged:

```python
x = (int)  # not rewritten — this is a value expression
```
