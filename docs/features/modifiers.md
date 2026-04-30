# modifier keywords

basedpython adds modifier keywords that compile to Python decorators, base classes, or type annotations. modifiers are placed before `class`, `def`, or variable declarations

## class modifiers

| basedpython | Python output |
|---|---|
| `final class Foo:` | `@final` from `typing` |
| `abstract class Foo:` | modifier deleted (no decorator) |
| `open class Foo:` | modifier deleted (no decorator) |
| `data class Foo:` | `@dataclass(slots=True)` from `dataclasses` |
| `frozen data class Foo:` | `@dataclass(frozen=True, slots=True)` |
| `enum Foo:` | base class `Enum` added from `enum` |
| `protocol Foo:` | base class `Protocol` added from `typing` |

### examples

```python
# basedpython
final class Config:
    host: str
```
```python
# generated Python
from typing import final

@final
class Config:
    host: str
```

```python
# basedpython
data class Point:
    x: int
    y: int
```
```python
# generated Python
from dataclasses import dataclass

@dataclass(slots=True)
class Point:
    x: int
    y: int
```

```python
# basedpython
frozen data class Point:
    x: int
    y: int
```
```python
# generated Python
from dataclasses import dataclass

@dataclass(frozen=True, slots=True)
class Point:
    x: int
    y: int
```

```python
# basedpython
enum Color:
    RED = auto()
    GREEN = auto()
```
```python
# generated Python
from enum import Enum

class Color(Enum):
    RED = auto()
    GREEN = auto()
```

```python
# basedpython
protocol Drawable:
    def draw(self) -> None: ...
```
```python
# generated Python
from typing import Protocol

class Drawable(Protocol):
    def draw(self) -> None: ...
```

## function modifiers

| basedpython | Python output |
|---|---|
| `final def foo():` | `@final` from `typing` |
| `abstract def foo():` | `@abstractmethod` from `abc`, body becomes `raise NotImplementedError` |
| `override def foo():` | `@override` from `typing` |
| `static def foo():` | `@staticmethod` |
| `class def foo(cls):` | `@classmethod` |

### examples

```python
# basedpython
class Base:
    abstract def process(self) -> int

class Child(Base):
    override def process(self) -> int:
        return 42

    static def create():
        return Child()

    class def from_config(cls, config):
        return cls()
```
```python
# generated Python
from abc import abstractmethod
from typing import override

class Base:
    @abstractmethod
    def process(self) -> int: raise NotImplementedError

class Child(Base):
    @override
    def process(self) -> int:
        return 42

    @staticmethod
    def create():
        return Child()

    @classmethod
    def from_config(cls, config):
        return cls()
```

## variable modifiers

| basedpython | Python output |
|---|---|
| `final x = 1` | `x: Final = 1` from `typing` |
| `let x = 1` | `x: Final = 1` from `typing` (at top level) |
| `class a = 1` | `a: ClassVar = 1` from `typing` |

### examples

```python
# basedpython
final MAX_SIZE = 100

class Config:
    class DEFAULT_HOST = "localhost"
    final VERSION = "1.0"
```
```python
# generated Python
from typing import Final, ClassVar

MAX_SIZE: Final = 100

class Config:
    DEFAULT_HOST: ClassVar = "localhost"
    VERSION: Final = "1.0"
```

## `newtype`

`newtype` creates a distinct type alias using `typing.NewType`:

```python
# basedpython
newtype UserId = int
```
```python
# generated Python
from typing import NewType
UserId = NewType("UserId", int)
```

## empty class declarations

basedpython allows class declarations without a colon or body:

```python
# basedpython
class Marker
```
```python
# generated Python
class Marker: ...
```

## required imports

all necessary imports (`typing.final`, `typing.Final`, `dataclasses.dataclass`, `enum.Enum`, `typing.Protocol`, `abc.abstractmethod`, `typing.override`, `typing.ClassVar`, `typing.NewType`) are added automatically when a modifier is used
