# overload syntax

basedpython lets you declare overloaded functions by writing consecutive bodyless signatures with the same name, followed by the implementation

## transformation rules

a run of ≥ 2 consecutive `def` statements with the same name where all but the last have an empty body becomes an overload group. each bodyless signature gets an `@overload` decorator and a `: ...` stub body

## example

```python
# basedpython
def f(a: int) -> int
def f(a: str) -> str
def f(a):
    return str(a)
```
```python
# generated Python
from typing import overload

@overload
def f(a: int) -> int: ...
@overload
def f(a: str) -> str: ...
def f(a):
    return str(a)
```

## standalone bodyless functions

a single bodyless function (not part of an overload run) gets `: ...` appended but no `@overload` decorator:

```python
# basedpython
def f(a: int) -> int
```
```python
# generated Python
def f(a: int) -> int: ...
```

## nesting level

overload detection applies at every scope — module level, class bodies, and nested functions:

```python
# basedpython
class Parser:
    def parse(self, data: str) -> dict
    def parse(self, data: bytes) -> dict
    def parse(self, data):
        ...
```
```python
# generated Python
from typing import overload

class Parser:
    @overload
    def parse(self, data: str) -> dict: ...
    @overload
    def parse(self, data: bytes) -> dict: ...
    def parse(self, data):
        ...
```

## interaction with `abstract`

bodyless functions with the `abstract` modifier are not treated as overload candidates — they are handled by the [modifier keywords](modifiers.md) transform instead
