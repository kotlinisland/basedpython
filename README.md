# basedpython

a Python-like language that transpiles to pure Python

## installation

```sh
uv add --dev basedpython
```

## usage

```sh
# Run a module
by run main

# Build all .by files to out/
by build

# Low-level: transpile a single file to stdout
by transpile file.by
echo 'x[(a, b)]' | by transpile
```

## options

```sh
# Target a minimum Python version (default: 3.10)
by --min-version 3.11 run main
by --min-version 3.12 build
```

## features

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

#### TODO

### python version polyfills

when `--min-version` is below the version that introduced a feature, basedpython rewrites it to an equivalent that runs on the target. all polyfills are no-ops when targeting a version that has the feature natively

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

#### TODO

### None operators

`?.` `??`
#### TODO 

### modifier keywords

```python
final data class A:
    final foo = 1

    class a = 1

    override def foo(): ...

    class def bar(): ...

    static def baz(): ...

enum B:
    a, b, c

protocol C:
    a: int

newtype MyInt = int
```

### visibility modifiers

`export` (alias `public`) and `private` mark module-level declarations:

```python
# input
export def api(): ...
private def helper(): ...
def untouched(): ...

# output
def api(): ...
def _helper(): ...
def untouched(): ...
__all__ = ["api"]
```

`export`/`public` adds the symbol's name to an auto-generated `__all__` list. `private` strips the modifier and renames the declaration with a leading underscore (the conventional Python "internal" marker). Both apply to `def` and `class` at module scope; inside a class body the modifier is stripped without renaming or affecting `__all__`

### subscription normalization

tuple subscripts are normalized so the key is an unambiguous 1-tuple:

```python
# input
x[(a, b)]
x[a, b]

# output
x[(a, b),]
x[(a, b),]
```

this eliminates the inconsistency with call expressions and visual ambiguity between `x[(a, b)]` (tuple key) and `x[a]` (scalar key)

