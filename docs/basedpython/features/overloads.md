# implicit overload stubs

a run of two or more consecutive bodyless `def`s with the same name is
recognized as an overload group. basedpython injects the `@overload`
decorator and a `: ...` body on each stub:

```by
def parse(s: str) -> int
def parse(s: bytes) -> int
def parse(s):
    return int(s)
```

transpiles to:

```python
from typing import overload

@overload
def parse(s: str) -> int: ...
@overload
def parse(s: bytes) -> int: ...
def parse(s):
    return int(s)
```

## scope

works at module, class body, or nested function scope. modifiers on the
implementation (e.g. `final def f(...)`) are preserved when the stub group
above it is decorated
