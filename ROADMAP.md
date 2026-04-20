# roadmap

THIS IS THE SECRET INTERNAL ROADMAP, DO NOT MAKE PUBLIC

## parser strategy

the project currently depends on `ruff_python_parser` and `ruff_python_ast` via git, pointing to the upstream
[astral-sh/ruff](https://github.com/astral-sh/ruff) repository. this is sufficient for transformations that operate on standard Python syntax

when the language needs to diverge from Python's grammar (new keywords, new operators, syntax that is not valid Python),
we will fork the relevant ruff parser crates (`ruff_python_parser`, `ruff_python_ast`, `ruff_python_lexer`) into this
repository and switch the `Cargo.toml` dependencies from git to path. at that point the fork becomes the canonical
parser for basedpython

## semantic analysis strategy

many planned features require knowledge that goes beyond the syntax of a single file: which names are types vs values,
what class a method call resolves to, where a symbol is defined across the project. this is needed for features like
literal literal types (distinguishing `1 | 2` as a type annotation vs a bitwise-or expression), extension functions
(rewriting `obj.bar()` call sites), sealed class enforcement, and annotation context detection inside subscripts

the plan has two phases:

**phase 1 — project-wide symbol table (no type inference)**
a pre-pass that collects class definitions, type aliases, extension function names, and imports across all `.by` files
in the project. this is sufficient for the majority of features: extension function call site rewriting, annotation
context detection for literal types, sealed class checking, auto type quoting. this is the next step and does not
require any new heavy dependencies

**phase 2 — fork `ty`**
[astral-sh/ty](https://github.com/astral-sh/ty) is the type inference engine being built alongside ruff. when
basedpython needs flow-sensitive type information (e.g. knowing the type of `x` after assignments, conditional
narrowing, cross-file inference beyond simple symbol lookup), we will fork the relevant `ty` crates into this
repository, the same way the parser will be forked when grammar divergence is needed. at that point `ty` becomes the
semantic backend for basedpython

**why not fork `ty` now:** `ty` is under very active development with unstable internal APIs and incomplete
functionality. a premature fork would create an enormous maintenance burden with no path to stay in sync with
upstream improvements. the symbol table phase handles nearly all near-term features, so the fork is deferred until
`ty` reaches an API stability point that makes the integration tractable

## planned features

- [ ] language server (fork ty)
- [ ] source maps
- [ ] build wheels (`by build`?)
  - ensure that built wheel depends on the correct deps for polyfill (`tomli`/`typing_extensions` etc)
- [ ] config file as part of `pyproject.toml`
  - [ ] enable all rules/strictness by default (with a "drop-in" option or something)
  - [ ] high level configs:
    - `based` - all features enabled in most based way
    - `enhanced` - only language additions (`??`), not reworks (new generics, tuple semantics) 
    - `pure-python` - only features that are in cpython, i.e. polyfils
  - [ ] config option to enforce `open`
  - [ ] config option for `abstract` impl: `abc.ABCMeta`, or `by_extensions.abstract`
- [ ] web playground like ty, runs client side
- [ ] tree-sitter/textmate grammar
- [x] subscription normalization: `x[(x, y)]` → `x[(x, y),]`
- [x] mutable default argument fix: `def f(x=[]):` → sentinel pattern
- [x] PEP 695 generics polyfill: `class A[T]`, `def f[T]()`, `type X = ...` → 3.10-compatible equivalents including TypeVar name renaming
- [x] `typing` import redirect: names unavailable in stdlib until later versions are redirected to `typing_extensions`
- [x] expression compat rewrites: `datetime.UTC`, `sys.exception()`, `math.exp2()` → 3.10-compatible equivalents
- [ ] callable syntax: `(int) -> int` -> `Callable[[int], int]` / `[T](int, t: T) -> T` → a `Protocol` definition
  `(int, /, a: str, *args: int, **kwargs: str) -> None`
- [x] unpack syntax in type annotations: `def f(*args: *tuple[int, ...])` -> `def f(*args: Unpack[tuple[int, ...]])` #easy
- [x] intersection types: `A & B` -> `ty_extensions.Intersection[A, B]` #easy
- [x] auto type quoting where mandatory: `class A(list[A])` -> `class A(list["A"])` #easy
- [x] empty class declarations: `class A` -> `class A: ...`
- [x] empty abstract functions: `abstract def f(self) -> int` -> `def f(self) -> int: raise NotImplementedError`
- [ ] `by_extensions` package with things like `abstract`
- [ ] keyword parameters in subscriptions: `x[y=z]`
- [ ] syntax for `TypeIs`: `def f(a) -> a is int` -> `def f(a) -> TypeIs[int]` #easy
- [ ] specify generics via keyword:
  ```bython
  class A[B=int, C=int]
  
  A[C=str]()
  ```
- [ ] explicit variance keywords: `class A[out T]`, `in`, `in out`
- [ ] multi-expression lambdas: (needs design)
  ```bython
  x(\( x, y ->
     print(x)
     print(y)
  ))
  ```
- [ ] extension functions: (needs design) `def Foo.bar():`
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
  xs.map() \( it + 1 )
  # → xs.map(lambda it: it + 1)
  ```
- [x] compile-time multiline string de-indenting:
  ```bython
  text = """
      start-of-line
      """
  ```
  ```python
  text = """\
  start-of-line\
  """
  ```
  - [ ] reverse transform
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

### anonymous named tuple:
```
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
- [ ] `Iterable.first_or_none` / `.last_or_none`
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

- [ ] `class a = 1` -> `a: ClassVar = 1` #easy
- [ ] `let a = 1` -> `a = 1` (there is no such `ReadOnly` in python) #easy
- [ ] `final a = 1` -> `a: Final = 1` #easy
- [x] `const x = 5` → `x: Final = 5` #easy
- [x] `final class Foo:` → `@final\nclass Foo:` (from `typing`) #easy
- [x] `final def foo():` → `@final\ndef foo():` (from `typing`) #easy
- [x] `open class Foo:` — explicitly permits subclassing (documentation/lint intent; `final` is the default for classes in strict mode) #easy
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
- [ ] `sealed class Foo:` — subclassing allowed only within the same module; enforced at transpile time #hard
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
    
    f(???)
    ```

### class keywords

- [x] `protocol Foo:` → `class Foo(Protocol):` (from `typing`) #easy
- [x] `data class Foo:` → `@dataclass(slots=True)\nclass Foo:` #easy
- [x] `frozen data class Foo:` → `@dataclass(frozen=True, slots=True)\nclass Foo:` #easy
- [x] `enum class Foo:` → `class Foo(Enum):` (from `enum`) #easy
- [x] `newtype Foo = int` -> `Foo = NewType("Foo", int)` #easy

#### to discuss
- [ ] `dict Foo:` -> `class Foo(TypedDict):`
- [ ] `tuple Foo:` -> `class Foo(NamedTuple):`

could we generalise this with some new syntax:
```bython
# typed dict
type Foo = {a: int}

# named tuple
type Bar = (name: str, age: int)
```

- [ ] ### primary constructors

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

# error object system

needs design

`!` means "return if error":

```bython
error class MyError

def f() -> str | MyError: ...

def g() -> int | MyError:
    f()!.length  # -> if isinstance(r := f(), MyError): return r ... 
```

do we need a syntax for `assert not None`? maybe `!!`, maybe an extension `.safe`?


# type system

- see `docs/generics.md` #hard
- [ ] a true top type: maybe Void/Unit: doesn't have **any** members
- [ ] rename `Any` to `dynamic`
- [ ] `final`/`Final` means can't override, not immutable
- [ ] mapped type parameters:
  needs design
  ```bython
  class A[T: {"a": int, "b": str, int: bool}]
      def __getattr__(self, key: T.Key) -> T.Value: ...
  ```
- [x] literal literal types: `a: "asdf" | 5 = "asdf"` -> `a: Literal["asdf", 5]` (single-file symbol resolution; cross-file resolution pending)
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
- `Ts` as a paramspec argument
  ```bython
  def f[*Ts](fn: Callable[*Ts, None])  # should be valid
- ```