# roadmap

THIS IS THE SECRET INTERNAL ROADMAP, DO NOT MAKE PUBLIC

## implementation strategy

the project forks `ruff` and `ty` for ast and type features respectivly

## planned features

### misc

- [x] fork ruff/ty
  - [x] drop-in replacement for `ty`
  - [x] drop-in replacement for `ruff`
- [ ] ide plugins
  - [ ] pycharm
  - [ ] vscode
- [ ] unified executable (`by`) with `transpile`, `check`, `lint`, `format`, `server`
- [x] source maps
- [ ] `by build` builds wheels 
  - [ ] built wheel depends on the correct deps for polyfill (`tomli`/`typing_extensions` etc)
- [ ] config file as part of `pyproject.toml`
  - [ ] enable all rules/strictness by default (with a "drop-in" option or something)
  - [ ] high level configs:
    - `based` - all features enabled in most based way
    - `enhanced` - only language additions (`??`), not reworks (new generics, tuple semantics) 
    - `pure-python` - only features that are in cpython, i.e. polyfils
  - [ ] config option to enforce `open`
  - [ ] config option for `abstract` impl: `abc.ABCMeta`, or `by_extensions.abstract`
  - [ ] config option for `final` by default
- [ ] web playground like ty, runs client side
- [x] textmate grammar
- [ ] compile to bytecode instead of python
- [x] subscription normalization: `x[(x, y)]` → `x[(x, y),]`
- [x] mutable default argument fix: `def f(x=[]):` → sentinel pattern
- [x] PEP 695 generics polyfill: `class A[T]`, `def f[T]()`, `type X = ...` → 3.10-compatible equivalents including TypeVar name renaming
- [x] `typing` import redirect: names unavailable in stdlib until later versions are redirected to `typing_extensions`
- [x] expression compat rewrites: `datetime.UTC`, `sys.exception()`, `math.exp2()` → 3.10-compatible equivalents
- [ ] type soundness:
  ```bython
  def f(a: int):
      print(a + 1)
  ```
  ```python
  def f(a: int):
      if not isinstance(a, int):
          raise TypeError
      print(a + 1)
  ```
- [ ] callable syntax:
  - [x] `(int) -> int` -> `Callable[[int], int]`
  - [ ] non-denotable: generate a `Protocol` definition
    - [ ] named: `(a: int) -> str`
    - [ ] other forms: `(int, /, a: str, *args: int, **kwargs: str) -> None`
    - [ ] type parameters `[T](int, t: T) -> T`
- [x] annotations on `lambda`: `lambda (a: int, b: str) -> bool: a + b` -> `lambda a, b: a + b` #easy
- [x] unpack syntax in type annotations: `def f(*args: *tuple[int, ...])` -> `def f(*args: Unpack[tuple[int, ...]])` #easy
- [x] intersection types: `A & B` -> `ty_extensions.Intersection[A, B]` #easy
- [x] auto type quoting where mandatory: `class A(list[A])` -> `class A(list["A"])` #easy
- [x] empty class declarations: `class A` -> `class A: ...`
- [x] empty abstract functions: `abstract def f(self) -> int` -> `def f(self) -> int: raise NotImplementedError`
- [ ] `by_extensions` package with things like `abstract`/`generic`
- [ ] syntax for `TypeIs`: `def f(a) -> a is int` -> `def f(a) -> TypeIs[int]` #easy
- [ ] keyword parameters in subscriptions: `x[y=z]`
- [ ] specify generics via keyword:
  ```bython
  class A[B=int, C=int]
  
  A[C=str]()
  ```
- [ ] explicit variance keywords: `class A[out T]`, `in`, `in out`
- [ ] reify generics
  ```bython
  a: list[int]
  a = []
  ```
  ```python
  a: list[int]
  a = list[int]()
  ```
- [ ] multi-expression lambdas: (needs design)
  ```bython
  x(: print(it))
  # trailing
  x: print(it)
  if x: print(it) # error, ambigious, need parens
  a = (): print(1)
  foo(:
      a = 1
      a + 1,
      :
      b = 2
      b + 1
  )
  ```
- [ ] extension functions: 
  needs design 
  ```bython
  def Foo.bar(self):
      return self.foo()
  
  # OR
  
  extension Foo:
      def bar(self):
          return self.foo()
  ```
- [ ] property syntax: (needs design)
  ```
  var a: int
      get(): 1
      set(value): field = value.lower()
  ```
- [x] mutable default argument fix — `def f(x=[])`
- [x] `None`-coalescing operator: `a ?? b` #easy
- [x] `None` chaining: `a?.b`, `a?.b()` → `None if a is None else a.b` etc #easy
- [ ] trailing closure: if the last argument is a lambda, it can be written outside the call parens: 
  needs design
  ```bython
  xs.map: it + 1
  # → xs.map(lambda it: it + 1)
  ```
- [ ] compile-time multiline string de-indenting:
  ```bython
  text = """
      start-of-line
      a
      """
  ```
  ```python
  text = """\
  start-of-line
  a\
  """
  ```
  - [x] transform
  - [ ] error when the text is positioned before the ending quotes
  - [ ] reverse transform
  - [ ] formatting support
- [ ] range literals: `1..10` `1..<10`
- [ ] lazy values: needs design
- [ ] `late` keyword: `late name: int`
- [ ] class delegation syntax #hard
  ```bython
  class A(list[int] by data):
      init(data: list[int])
  ```
- [ ] top level descriptors #harder
- [ ] `return`/`raise` expressions
- [ ] instances as attributes #harder
  ```bython
  class A: 
      class a = A()
  ```
- [ ] decorate anything
  ```bython
  f: @MyDecorator int # f: Annotated[int, MyDecorator()]
  
  @deprecated("this is deprecated")
  g = 1  # this one can also become `Annotated`
  
  # sometimes it might need to be dropped, maybe into a comment or something 
  ```
- [ ] lazy imports by default
- [ ] inline/multiline comments
- [ ] more flexable parameters
  ```bython
  def f1(*args, other): ...
  f1(1, 2, 3)  # `3` is passed as `other`
  
  def f2(a, b, c)
  
  f2(1, b=2, 3)
  ```
  - the first arg after `*args` can't have a default
- [ ] unpacks in function defs `def f((a, b), c)` -> `def f(_ab, c):\n a, b = _ab`
  - where does the type annotation go?
- [ ] conditional elements: `[if bool(): 1, 2]`
  - maybe also call expressions
- [ ] make ternary expressions bearable: `if bool() 1 else 2`
- [ ] `match`/`try` as an expression #easy
- [ ] reified type parameters #types
  ```bython
  def f[T](x: object) -> bool:
      return isinstance(x, T)
  f[int](1)
  ```
  ```python
  # this decorator evaluates the function but replaces `T` (in `__closure__`) with the passed value
  @generic
  def f[T](x: object) -> bool:
      return isinstance(x, T)
  f[int]
  ```
- [ ] generic function calls 
  ```bython
  def f[T](t1: T, t2: T) -> T: ...
  f[object](1, "a")
  ```
- [ ] `private` class members should be prefixed with `__`
- [ ] `===` operator and `x is y` -> `instanceof(x, y)`
- [ ] string function names: `def "this doesn't fail"()`
- [ ] mutate by copy: #veryhard
  needs design
  ```bython
  data class A:
      init(copy var i: int)
  a = A(1)
  b = a
  a.i = 2  # a = a.copy(i = 2) 
  b === a # false
  ```
- [ ] macros #veryhard
- [ ] shape type `closed` should be a keyword
- [ ] enum shorthand syntax `enum E: a, b, c, d` #easy
  ```python
  class E(Enum):
      a = auto()
      ...
  ```
- [ ] rust stype enums?
- [ ] syntax for `cast`
  needs design 
  `a as int`/`a as? int`/`a.as[int]`
- [ ] `super` syntax
  ```by
  class A(int, str):
      def f():
          super[int].__str__()
  ```

### anonymous named tuple:
```bython
def foo(x: (name: str, age: int)) -> (name: str, age: int):
    return ("asdf", 1)

foo(("asdf", 1))


a = (name: str, age: int)
```

### stdlib extensions

These compile to calls into a thin `basedpython` runtime module (or are rewritten inline where the expansion is small enough). No grammar fork needed.

**sequences / iterables**
- [ ] `.len` — `len(x)` as a member; avoids the global function syntax
- [ ] `Sequence.get(i, default=None)` — index with a fallback, like `dict.get`
- [ ] `Iterable.first` / `.last` — first or last element; raise on empty
- [ ] `Iterable.first_or_none` / `.last_or_none` (also .first_or_missing) sentinal value version
- [ ] `Iterable.first(predicate)` — first matching element
- [ ] `Iterable.is_empty` / `.is_not_empty` —> `len(s) == 0`
- [ ] `filter`/`map`/`reduce`

**strings**
- [ ] `str.to_int(base=10) -> int | None` — safe parse, returns `None` on failure
- [ ] `str.to_float() -> float | None`
- [ ] `str.chars()` — iterate over Unicode scalar values (not bytes)

**numbers**
- [ ] `int.clamp(min, max)` — clamp to inclusive range
- [ ] `float.clamp(min, max)`

### visibility modifiers

- [x] `export`/`public` — marks a symbol for inclusion in auto-generated `__all__`; unmarked module-level names are treated as internal #easy
- [x] `private` — compiles with `_`-prefix convention and excludes from `__all__` #easy
- [ ] `internal` — package-private; accessible within the package but not from outside

### declaration modifiers

- [x] `class a = 1` -> `a: ClassVar = 1` #easy
- [x] `override a = 1` -> `a = 1` #easy
- [x] `absract a`  #easy
- [x] `let a = 1` -> `a = 1` (there is no such `ReadOnly` in python, can use `Final` at top level) #easy
- [x] final variable: #easy
  ```bython
  class A:
      final a = 1
  final a = 1
  ```
  ```python
  class A:
      a = 1
  a: Final = 1
  ```
- [x] `final class Foo:` → `@final\nclass Foo:` (from `typing`) #easy
- [x] `final def foo():` → `@final\ndef foo():` (from `typing`) #easy
- [x] `open class Foo:` — `final` will be the default for classes in strict mode #easy
- [x] `abstract class Foo:` → `class Foo:` (don't use `abc`, it's too invasive) #easy
- [x] `abstract def foo():` → `@abstractmethod\ndef foo(): raise NotImplementedError` #easy
- [x] `override def foo():` → `@override\ndef foo():` (from `typing`) #easy
- [x] overload: #easy
  ```bython
  def f(a: int) -> int
  def f(a: int) -> int
  def f(a): ...
  ```
  ```python
  @overload
  def f(a: int) -> int: ...
  @overload
  def f(a: int) -> int: ...
  def f(a): ...
  ```
- [x] `static def foo():` inside a class → `@staticmethod\ndef foo():` #easy
- [x] `class a = 1` -> `a: ClassVar = 1` #easy
- [x] `class def f(cls):` -> `@classmethod\ndef f(cls):` #easy
- [ ] more flexible keyword arguments
  ```bython
  def f(**kwargs: dict[str, int]): ...

  f(foo.bar=1, "/*"=2)
  ```
  - [ ] can we have keyword arguments of an arbitrary dict???
    ```bython
    def f(**kwargs: dict[object, int]): ...
    
    f(???)  # not just str
    ```
- [ ] f-string by default: `"{1}"` -> `"f"{1}"`
- [ ] type-directed t-strings
  ```python
  def f(t: Template): ...
  
  f("asdf{abc}fdsa")
  ```
- [ ] t-string prefix functions: `foo"a{b}c"` -> `foo(t"a{b}c")`
- [ ] type-directed other things (need to think about what it is)
- [ ] dot access for tuples: `('a', 'b').1` -> `('a', 'b')[1]`

### class keywords

- [x] `protocol Foo:` → `class Foo(Protocol):` (from `typing`) #easy
- [x] `data class Foo:` → `@dataclass(slots=True)\nclass Foo:` #easy
- [x] `frozen data class Foo:` → `@dataclass(frozen=True, slots=True)\nclass Foo:` #easy
- [x] `enum class Foo:` → `class Foo(Enum):` (from `enum`) #easy
- [x] `newtype Foo = int` -> `Foo = NewType("Foo", int)` #easy

### shape type syntax

```bython
# typed dict
type Foo = {"a": int}

# named tuple
type Bar = (name: str, age: int) 
# OR
type Bar = (name = str, age = int)
```
- [ ] index types for typed dict
  ```python
  type a = {
      a: int,
      **: dict[str, str],
  }
  ```
- [ ] inline protocol
  needs design
  ```
  a: protocol (
    def foo()
  )
  ```

### primary constructors

- [ ]

```bython
class A:
    init(let age: int, var name: str, c: bool)

A(1, "asdf").age
```

### context parameters

needs design

```bython
def f(context a: int):
    print(a)
    
context i = 1  # adds `i` to the 'context scope', eligible for passing as a context parameter
f()  # i gets passed for `a` implicitly
# can also pass explicitly
f(a=i)
```

### inline instances

needs design

```bython
a = object: Base(
    def f(self):
        print("hi")
)
```

### error object system

needs design

`!` means "return if error":

```bython
error class MyError

def f() -> str | MyError: ...

def g() -> int | MyError:
    f()!.length  # -> if isinstance(r := f(), MyError): return r ... 
```

do we need a syntax for `assert not None`? maybe `!!`, maybe an extension `.safe`?

## type system

- [ ] see `docs/generics.md` #hard
- [ ] a true top type: maybe `Void`/`Unit`: doesn't have **any** members
- [ ] rename `Any` to `dynamic` #easy
- [ ] `final`/`Final` means can't override, not immutable
- [ ] mapped type parameters:
  needs design
  ```bython
  class A[T: {"a": int, "b": str, int: bool}]
      def __getattr__(self, key: T.Key) -> T.Value: ...
  ```
- [x] literal literal types: `a: "asdf" | 5 = "asdf"` -> `a: Literal["asdf", 5]`
- [x] tuple literal types: `a: (int, str)` -> `a: tuple[int, str]`
- [ ] tuples are variadic by default: `a: tuple[int]` -> `tuple[int, ...]`
- [ ] constraints need to use a keyword: `type A[T: constraints (int, str)]` -> `type A[T: (int, str)]`
- [ ] no number promotion: `a: float` -> `a: ty_extensions.JustFloat`
- [ ] no unpack needed: `def f(*args: tuple[int, ...])` -> `def f(*args: int)`
  this would be an optional feature
- [ ] attribute types: #hard
  ```bython
  class A1:
      a: ReadOnly[int]
  
  class A2(A1):
      a: ReadOnly[int]
  
  class B[T: A]:
      x: T.a
  
  B[A1].a  # int
  B[A2].a  # str
  ```
- [ ]  `Overload` types: `Overload[(int) -> int, (str) -> str]`
- [ ] flexible param spec:
  ```bython
  class A[**P]:
      args: P.args  # tuple of args passed positionally
      kwargs: P.kwargs  # dict of args passed with kw
  ```
- [ ] `TypedDict` as a bound:
  ```bython
  from typing import TypedDict, Unpack
  
  class A[ExtraArgs: TypedDict]:
  def do_it(self, **extra: Unpack[ExtraArgs]): ...
  
  class Thing(TypedDict):
      t: int
  
  A[Thing]().do_it(
      t=1,             # no error
      imposter="sus",  # yes error
  )
  ```
- [ ] `Ts` as a paramspec argument
  ```bython
  def f[*Ts](fn: Callable[*Ts, None])  # should be valid
  ```
- [ ] inline type plugins #veryhard
  needs design
  ```bython
  @type_function
  def tf():
      return datetime.now().seconds  # literal int type 
  
  def f() -> tf(): ...
  ```
- [ ] `sealed class Foo` — subclassing allowed only within the same project #hard
  we could compile the subtypes into the base class!
- [ ] literal `float` and `complex` types
- [ ] fix `type`, make constructors safe
- [ ] fix overloads (hmm)
- [ ] error with top level `final` variables