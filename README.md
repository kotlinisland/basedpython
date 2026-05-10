# basedpython

a Python-like language that transpiles to pure Python

## acknowledgements

basedpython is a fork of [astral-sh/ruff](https://github.com/astral-sh/ruff). the
transpiler reuses ruff's parser (`ruff_python_parser`), AST (`ruff_python_ast`),
and fix-application machinery (`ruff_diagnostics::Edit`/`Fix`), and the type
checker is built on [ty](https://github.com/astral-sh/ty). none of this would
exist without the work of the astral team and the wider ruff community

## installation

```shell
uv add --dev basedpython
```

## usage

```shell
# run a module
by run main

# build all .by files to out/
by build

# low-level: transpile a single file to stdout
by transpile file.by
echo 'x[(a, b)]' | by transpile
```

## options

```shell
# target a minimum Python version (default: 3.10)
by --min-version 3.11 run main
by --min-version 3.12 build
```

## features

### anonymous named tuple syntax

write a structural record inline, no separate `class` or `NamedTuple` import.
identical shapes anywhere in the module collapse to a single hoisted
`typing.NamedTuple` subclass, so structural equality is preserved at the type
level:

```bython
def user(x: (name: str, age: int)) -> (name: str, age: int):
    return ("charlie", 36)

a = (name: str, age: int)
```

mixed positional/named shapes are allowed

### mutable default argument via lazy evaluation

mutable default arguments are automatically rewritten to the sentinel pattern:

```python
# input
def f(x=[], y={}):
    pass

# output
_MISSING = object()
def f(x=_MISSING, y=_MISSING):
    if x is _MISSING:
        x = []
    if y is _MISSING:
        y = {}
    pass
```

### callable syntax

write callable types with arrow syntax. denotable shapes transpile to
`typing.Callable`; non-denotable shapes (named params, `/` / `*` markers,
variadics, kwargs) synthesize a hoisted `typing.Protocol` with `__call__`:

```bython
# input
f: (int, str) -> bool
g: () -> None
h: (a: int, *args: str) -> bool

# output
from typing import Callable, Protocol
f: Callable[[int, str], bool]
g: Callable[[], None]

class _Callable_abcde(Protocol):
    def __call__(self, a: int, /, *args: str) -> bool: ...

h: _Callable_abcde
```

identical non-denotable shapes anywhere in the module collapse to a single
synthesized protocol. nested arrows (`(int) -> (str) -> bool`) nest the
`Callable`s

### python version polyfills

basedpython lets you write code in modern python syntax and run it on older
interpreters. when `--min-version` is below the version that first introduced a
feature, the transpiler rewrites that feature into an equivalent shape that
runs on the target interpreter — a "polyfill". if the target already has the
feature natively, the polyfill is a no-op and the source survives unchanged

a few rules hold for every polyfill:

- **opt-in by target** — only triggered when `--min-version` is below the
    feature's introduction version. raise the floor to drop the rewrite
- **shape-preserving** — the rewritten code has the same runtime semantics and,
    where reasonable, the same static-typing behaviour as the original
- **no runtime dependency on basedpython** — output is plain python; the
    generated code does not call back into a basedpython runtime

**PEP 695 generics** (3.12 → 3.10):

```python
# input
class Stack[T]:
    items: list[T]

def identity[T](x: T) -> T:
    return x

type Vector = list[float]

# output
from typing import TypeVar, Generic, TypeAlias
_T = TypeVar("_T")
class Stack(Generic[_T]):
    items: list[_T]

_T = TypeVar("_T")
def identity(x: _T) -> _T:
    return x

Vector: TypeAlias = list[float]
```

**`typing` import redirect** — names not available in stdlib until a later version are automatically redirected to `typing_extensions`:

```python
# input (targeting 3.10)
from typing import Self, Never, override

# output
from typing_extensions import Self, Never, override
```

**expression rewrites** (targeting < 3.11):

```python
datetime.UTC          →  datetime.timezone.utc
sys.exception()       →  sys.exc_info()[1]
math.exp2(x)          →  2 ** (x)
```

### multiline strings

triple-quoted strings opening with `"""\n` and consistent leading indentation
get their common indent stripped at compile time. no `textwrap.dedent` import,
no runtime cost:

```python
# input
text = """
    hello
        world
    """

# output
text = """\
hello
    world\
"""
```

### None operators

#### `?.` — optional attribute access

`a?.b` short-circuits to `None` when `a` is `None`. chains use a walrus to
avoid evaluating compound prefixes twice:

```bython
# input
x = user?.profile?.name

# output
x = None if user is None else None if (_t := user.profile) is None else _t.name
```

#### `??` — None-coalesce

`a ?? b` returns `a` when non-`None`, otherwise `b`:

```bython
# input
x = a ?? b

# output
x = a if a is not None else b
```

composes with `?.` — the expanded chain is shared via a walrus so the prefix
runs once:

```bython
# input
y = a?.a.b ?? 1

# output
y = _t if (_t := None if a is None else a.a.b) is not None else 1
```

### modifier keywords

basedpython exposes the common decorator-driven idioms as bare keywords so
declarations stay readable. each keyword lowers to the equivalent decorator,
base class, or annotation and the matching import is added automatically:

| keyword (input)            | output                                       |
| -------------------------- | -------------------------------------------- |
| `final class A`            | `@final` on `class A`                        |
| `final def f()`            | `@final` on `def f()`                        |
| `override def f()`         | `@override` on `def f()`                     |
| `abstract def f()`         | `@abstractmethod` on `def f()`               |
| `static def f()`           | `@staticmethod` on `def f()`                 |
| `class def f()`            | `@classmethod` on `def f()`                  |
| `data class A`             | `@dataclass(slots=True)` on `A`              |
| `frozen data class A`      | `@dataclass(frozen=True, slots=True)` on `A` |
| `enum class B`             | `class B(Enum)`                              |
| `protocol Foo`             | `class Foo(Protocol)`                        |
| `let x = 5`                | `x: Final = 5`                               |
| `class a = 1` (class body) | `a: ClassVar = 1`                            |
| `newtype MyInt = int`      | `MyInt = NewType("MyInt", int)`              |

modifiers stack — `final data class A` and `override final def f()` both work.
example:

```bython
# input
final data class A:
    let x = 1
    class y = 2

    override def render(self): ...
    class def from_str(cls, s): ...
    static def helper(): ...

protocol Drawable:
    def draw(self): ...

enum class Color:
    RED = 1
    GREEN = 2

newtype UserId = int

# output
from abc import abstractmethod
from dataclasses import dataclass
from enum import Enum
from typing import ClassVar, Final, NewType, Protocol, final

@final
@dataclass(slots=True)
class A:
    x: Final = 1
    y: ClassVar = 2

    @override
    def render(self): ...
    @classmethod
    def from_str(cls, s): ...
    @staticmethod
    def helper(): ...

class Drawable(Protocol):
    def draw(self): ...

class Color(Enum):
    RED = 1
    GREEN = 2

UserId = NewType("UserId", int)
```

### visibility modifiers

`public` and `private` mark `def` and `class` declarations.
behaviour depends on whether the declaration is at module scope or inside a
class body:

```bython
# input
public def api(): ...
private def helper(): ...
def untouched(): ...

# output
def api(): ...
def _helper(): ...
def untouched(): ...
__all__ = ["api"]
```

- `public` at module scope — modifier stripped, name appended to an
    auto-generated `__all__`
- `private` at module scope — modifier stripped, declaration renamed with a
    leading `_` (the conventional python "internal" marker)
- `private` inside a class body — declaration renamed with a leading `__` so
    python's name-mangling hides it from subclass scope
- `public` inside a class body — modifier stripped, no rename, no
    `__all__` impact

### api lock file

`by generate-api-file` walks every module and emits a deterministic,
line-oriented summary of the project's public type-level surface to `api.lock`

the file is meant to be **diffed, not parsed**. any meaningful change to a
public symbol — a new parameter, a widened return type, a renamed class,
a removed attribute — surfaces as a line-level diff in code review

usage:

```shell
# write api.lock at the project root
by generate-api-file

# pick a path
by generate-api-file -o public.lock

# print to stdout (useful in CI to compare against committed lockfile)
by generate-api-file --stdout
```

commit `api.lock` and treat any unexpected diff in a PR as a public-api
breakage signal
