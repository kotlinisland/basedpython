# wrapped optional and result types

> **STATUS: planned for version 0.2, not yet implemented.** the `T?`,
> `T ? E` type forms, auto-wrap at return sites, and the `^` / `!` / `??` /
> `?.` postfix operators against wrapped values are not yet recognized by
> the parser. `??` and `?.` against plain `T | None` work today — see
> [none-coalesce](none-coalesce.md) and [optional-chaining](optional-chaining.md)

basedpython provides first-class wrapped types for absence (`Optional`) and
fallibility (`Result`) with auto-wrap at return sites and a symmetric set of
postfix operators for propagation, assertion, coalescing, and chaining

## type syntax

`T?` declares an optional value (Swift-style `Optional<T>`):

```by
def f() -> int?:
    return None   # auto-wraps to Optional.None_
    return 1      # auto-wraps to Optional.Some(1)
```

`T ? E` declares a result with value type `T` and error type `E`
(Rust-style `Result<T, E>`):

```by
def g() -> int ? TypeError:
    return 1                # auto-wraps to Result.Ok(1)
    return TypeError()      # auto-wraps to Result.Err(TypeError())
```

both forms compose: `T?? E`, `T ? E?`, etc

## auto-wrap

`return` inside a function whose return annotation is `T?` or `T ? E`
auto-wraps the bare value into the corresponding constructor:

| return annotation | bare expression   | wrapped form           |
| ----------------- | ----------------- | ---------------------- |
| `T?`              | `None`            | `Optional.None_`       |
| `T?`              | value of type `T` | `Optional.Some(value)` |
| `T ? E`           | value of type `T` | `Result.Ok(value)`     |
| `T ? E`           | value of type `E` | `Result.Err(value)`    |

dispatch is type-directed. when a value satisfies both `T` and `E`
(e.g. `E` is a subclass of `T`), `Err` wins

## operators

all five postfix/infix operators apply uniformly to both `Optional` and
`Result`. the "absent" case is `None_` for `Optional` and `Err(_)` for
`Result`

### `^` — propagate

`expr^` unwraps the inner value. on the absent case, the enclosing function
returns the absent value early. the enclosing function's return type must be
compatible (`T?` or `T ? E`):

```by
def caller() -> int?:
    x = foo()^     # returns None early if foo() is None_
    return x + 1
```

```by
def caller() -> int ? TypeError:
    x = bar()^     # returns the Err early if bar() is Err(_)
    return x + 1
```

cross-wrap propagation: `Result`-propagated inside an `Optional`-returning
function collapses `Err(_)` to `None_`. `Optional`-propagated inside a
`Result`-returning function requires an error coercion in scope (otherwise
a type error)

evaluation order: short-circuits at the propagation point. expressions to
the right of the propagated sub-expression are not evaluated when the
propagation fires

### `!` — force unwrap

`expr!` unwraps the inner value, panicking on the absent case:

```by
x = foo()!     # raises if foo() is None_ or Err(_)
```

panics raise `RuntimeError("force-unwrap of absent value")` and include the
wrapped error as `__cause__` when the value was an `Err`

### `??` — coalesce

`expr ?? default` evaluates to the inner value on the present case, and to
`default` on the absent case:

```by
x = foo() ?? 0     # works for Optional[int] and Result[int, _]
```

for `Result`, the error payload is discarded. when the wrapped error needs
to be inspected, use pattern matching or `^` propagation instead

`??` keeps its existing `is not None` semantics for plain (unwrapped)
expressions — see [none-coalesce operator](none-coalesce.md). when the left
operand has a wrapped type, the absent case is determined by the wrapper,
not by `is None`

### `?.` — chain

`expr?.attr` short-circuits on the absent case and yields a wrapped value
of the same shape:

```by
city = user?.address?.city    # Optional[str] if user: User?
```

for `Result`-typed receivers, `?.` forwards the `Err` unchanged:

```by
name = lookup()?.name         # Result[str, E] if lookup() -> User ? E
```

see [optional chaining](optional-chaining.md) for the temp-variable
mechanism — wrapped receivers reuse the same caching strategy

## auto-unwrap

assigning a wrapped value to a target whose type does not name the wrapper
triggers an implicit unwrap. when the target type still encodes every state
of the source (lossless), no diagnostic is emitted. when at least one state
collapses (lossy), the transpiler emits a warning at the assignment site

lossless cases:

```by
def f(x: int?):
    y: int | None = x        # ok — Optional[int] ≡ int | None, no info lost

def g(x: int ? TypeError):
    y: int | TypeError = x   # ok — both states preserved in the union
```

lossy cases (warn: `automatic unwrap loses information`):

```by
def f(x: int?):
    a: object = x            # warn — None vs Some(_) state collapsed
    b: int = x               # warn — None state dropped

def g(x: int??):
    y: int | None = x        # warn — outer None_ vs inner None_ collapsed
```

the lossless check operates on the structural decomposition of the wrapped
type:

- `T?` ⇒ `T | None`
- `T ? E` ⇒ `T | E`
- `T??` ⇒ `T | None | <outer-none-sentinel>` (no plain target is lossless)
- `(T ? E)?` ⇒ `T | E | None`

a target is lossless iff every variant of the decomposition is assignable
to it without merging two source variants into one target variant

suppress the warning with an explicit operator: `x!` to assert presence,
`x ?? default` to coalesce, `match x:` to destructure

## composition

operators chain in the obvious way. each operator consumes one wrapper
layer:

```by
def f() -> int ? TypeError:
    return (load()^.value ?? 0) + 1
```

- `load()^` propagates `Err` early, yields the inner value
- `.value` is plain attribute access on the unwrapped value
- `?? 0` is unrelated here — would apply if `.value` itself were wrapped

doubly-wrapped types (`T??`, `(T ? E)?`) require two operator applications
to fully unwrap

## interop

`T?` is runtime-compatible with `T | None` and `Optional[T]` from `typing`.
`Optional.Some(x)` is `x`; `Optional.None_` is `None`. existing python code
that returns `None`/value continues to work without modification

`T ? E` lowers to a tagged-union polyfill. see
[polyfills](polyfills.md) for the `Result` runtime shape

## scope

- auto-wrap fires only on `return` statements whose enclosing function has
    a wrapped return annotation. bare `return` inside an unannotated function
    is unchanged
- `^` is recognized as a postfix operator only when the operand is a
    wrapped type. on plain values it remains bitwise XOR (infix)
- `!` is recognized as a postfix operator only when the operand is a
    wrapped type. on plain values it remains logical-not (prefix)
- `??` and `?.` retain their plain-value behavior when the receiver is not
    wrapped, and pick up the wrapped behavior when type inference says the
    receiver is `Optional` or `Result`
