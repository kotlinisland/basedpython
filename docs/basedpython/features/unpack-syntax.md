# unpack syntax

basedpython polyfills the PEP 646 starred-type syntax in variadic parameter annotations for Python versions below 3.11

## transformation

```python
# basedpython
def f(*args: *tuple[int, ...]):
    pass
```

```python
# generated Python
from typing import Unpack
def f(*args: Unpack[tuple[int, ...]]):
    pass
```

the starred form (`*T`) is native in Python 3.11+. for earlier targets the equivalent `Unpack[T]` form is used instead

## when it applies

the transform applies only to `*args` parameter annotations. starred expressions in other positions (unpacking in assignments, function calls, etc.) are never affected

## `--min-version` interaction

when targeting Python 3.11 or later, the starred form is valid natively and the transform is a no-op
