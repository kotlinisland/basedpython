# empty declarations

class and function declarations may be written without a body. basedpython
fills in `: ...` at transpile time so the output is valid Python:

```by
class Empty
class Stub(Base)
def stub(x: int) -> int
```

transpiles to:

```python
class Empty: ...
class Stub(Base): ...
def stub(x: int) -> int: ...
```

## scope

the bodyless form is recognized for both `class` and `def`. an empty class
gets `: ...`. a single empty `def` not in an overload run also gets `: ...`.
empty defs that *are* part of an overload run instead receive
`@overload` decorators (see [overloads](overloads.md))

an `abstract def` with no body is given `: raise NotImplementedError` rather
than `: ...`

## interaction with modifiers

modifiers stack as expected:

```by
final class Sentinel
data class Empty
```

```python
@final
class Sentinel: ...

@dataclass(slots=True)
class Empty: ...
```

## why

a great deal of stub and protocol code consists of declaration-only
signatures. allowing `class Foo` / `def foo()` without a placeholder body
avoids `: ...` noise, particularly in stub-heavy modules and overload
clusters
