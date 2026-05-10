# based enums

> **STATUS: planned for version 0.2, not yet implemented.** the `enum`
> keyword described below is not recognised by the parser. tracking item:
> based enums v0.2

basedpython supports algebraic sum types — "based enums" — modeled after
rust and swift enums but with pythonic surface syntax. variants can carry
typed payloads, support pattern matching, and integrate with ty's
exhaustiveness checking. methods, classmethods, properties, and generics
live directly on the enum body. type-directed emit lowers the surface form
to a sealed dataclass hierarchy at runtime

## goals

- sum types (tagged unions) like rust/swift
- pythonic body: `def` for methods, `[T]` for generics, no foreign keywords
- type-directed emit: clean surface, valid python 3.10+ runtime
- no syntax cages: variants can be unit, tuple, or struct-like; methods,
    properties, classmethods, and staticmethods all live on the enum;
    recursive and generic forms supported

## surface syntax

```by
enum Shape:
    Circle(radius: float)
    Rectangle(width: float, height: float)
    Point
    Polygon { sides: list[Point], closed: bool = True }

    def area(self) -> float:
        match self:
            case Circle(r): 3.14 * r * r
            case Rectangle(w, h): w * h
            case Point: 0.0
            case Polygon { sides, closed: True }: shoelace(sides)
            case Polygon: 0.0

    @classmethod
    def unit_circle(cls) -> Shape:
        return Circle(1.0)
```

four variant kinds, mix freely within one enum:

- **unit** — `Point` — singleton, no payload
- **tuple positional** — `Circle(float)` — positional construct, anonymous fields
- **tuple named** — `Circle(radius: float)` — positional construct,
    named fields available for pattern matching and attribute access
- **struct-like** — `Polygon { sides: ..., closed: bool = True }` —
    kwargs construct, named-only pattern, supports defaults

generic and recursive forms:

```by
enum Tree[T]:
    Leaf
    Node(T, Tree[T], Tree[T])

    def depth(self) -> int:
        match self:
            case Leaf: 0
            case Node(_, l, r): 1 + max(l.depth(), r.depth())

enum Result[T, E]:
    Ok(T)
    Err(E)
```

bounds use the same syntax as based generics elsewhere:
`enum E[T: Hashable]`, `enum E[T: constraints (int, str)]`

## variant access

inside the enum body and in pattern contexts, variants are referred to bare
(`Circle(2.0)`). outside the body, the qualified form is `Shape.Circle(2.0)`.
both are legal at module scope — ty resolves which `Circle` is meant from the
expected type. when there is no expected type, qualification is required to
disambiguate

variant constructors are real classes at runtime, so `isinstance(x, Circle)`
works, and `type(x) is Circle` is fast

## methods and other members

the enum body is a regular suite. anything that can appear in a python class
body can appear in an enum body:

- `def` methods (dispatched on the union; usually implemented via `match self`)
- `@classmethod`, `@staticmethod`, `@property`
- nested types
- class-level constants

methods attach to the sealed base class. variant-specific methods can be
declared by narrowing the receiver type: `def f(self: Circle) -> float`

## pattern matching

based enums plug directly into python's `match` statement. variant patterns
use the variant constructor form:

```by
match shape:
    case Circle(r): ...
    case Rectangle(w, h): ...
    case Point: ...
    case Polygon { sides, closed: True }: ...
    case Polygon { sides, closed }: ...
```

ty checks **exhaustiveness**: a `match` over a based enum that fails to
cover every variant produces a diagnostic. wildcard `case _:` opts out of
exhaustiveness for that match. exhaustiveness is enforced in both statement
and expression positions

## expression-form match

`match` can appear in expression position (e.g. the body of a one-line
method, the rhs of `=`):

```by
def area(self) -> float:
    match self:
        case Circle(r): 3.14 * r * r
        case Rectangle(w, h): w * h
        case Point: 0.0
```

every arm's body must produce a value of the common type. ty infers the
union and applies exhaustiveness as usual

## variant as type

a single variant name is usable as a type. ty narrows the receiver:

```by
def double_radius(c: Shape.Circle) -> Shape.Circle:
    return Circle(c.radius * 2)
```

assignability follows the obvious rule: `Shape.Circle` is a subtype of
`Shape`, but `Shape` is not a subtype of `Shape.Circle`

## destructuring with `if let`

shorthand for one-variant peel:

```by
if let Some(x) := opt:
    use(x)
```

equivalent to a single-arm `match` followed by the else branch. extends the
walrus operator with pattern syntax

## auto-derive

based enums auto-derive `__eq__`, `__hash__`, `__repr__`, and
`__match_args__` from the variant shape. opt-out via decorator:

```by
@no_derive(hash)
enum Big:
    Blob(bytes)
```

equality is structural: two `Circle(2.0)` values compare equal. hashing
requires every payload field to be hashable; ty flags violations

## prelude enums

based prelude ships `Option[T]` and `Result[T, E]` as based enums. user
code gets them without an import:

```by
def find(xs: list[int], target: int) -> Option[int]:
    for i, x in enumerate(xs):
        if x == target:
            return Some(i)
    return None
```

`None` here is the based enum unit variant, not python's `None`. at runtime
the prelude maps based `None` to a sentinel that is `==`-equivalent to
python `None` for ergonomic interop (open question, see below)

## transpiler output

each variant lowers to a frozen dataclass subclassing a sealed base. the
enum name becomes a union alias:

```python
from dataclasses import dataclass
from typing import final

class _ShapeBase:
    def area(self) -> float: ...  # methods attach here

@final
@dataclass(frozen=True, slots=True)
class Circle(_ShapeBase):
    radius: float

@final
@dataclass(frozen=True, slots=True)
class Rectangle(_ShapeBase):
    width: float
    height: float

@final
class Point(_ShapeBase):
    _instance = None
    def __new__(cls):
        if cls._instance is None:
            cls._instance = super().__new__(cls)
        return cls._instance

@final
@dataclass(frozen=True, slots=True)
class Polygon(_ShapeBase):
    sides: list[Point]
    closed: bool = True

Shape = Circle | Rectangle | Point | Polygon
```

method bodies are emitted on `_ShapeBase`. `match` arms with bare variant
names lower to qualified class patterns when the runtime emitter requires
them. `__match_args__` comes from dataclass for tuple variants and is set
explicitly for struct-like variants

generic enums lower to `Generic[T]` subclasses of the base; the alias
becomes a `TypeAlias`

## lowering provenance

new `LoweringKind` tags so later passes can identify enum-derived constructs
without re-parsing:

- `EnumBase` — the synthesized sealed base class
- `EnumVariant` — each variant dataclass
- `EnumUnionAlias` — the `Shape = Circle | ...` line
- `EnumMatchArm` — match arms whose pattern targets an enum variant

reverse transforms use these tags to reconstruct surface `enum` blocks from
the lowered python

## limitations / open questions

- **variant namespace at module scope** — bare `Circle` vs `Shape.Circle`.
    leaning: both legal, ty resolves; collisions across enums require
    qualification
- **mutability** — frozen by default. open: per-variant `mut` modifier? a
    project-wide `mut` keyword may make more sense
- **enum extension across modules** — rust forbids, swift allows. leaning:
    sealed (rust-style) for exhaustiveness
- **prelude `None` vs python `None`** — runtime identity vs equality
    semantics; needs a concrete decision before prelude lands
- **`match self` shorthand in methods** — `match: case ...:` with implicit
    `self`. nice ergonomics, ambiguous lookup. leaning: not in v1
- **derivable traits beyond eq/hash/repr** — `Ord`, `Copy`-equivalent,
    serialization hooks. probably opt-in via decorators rather than syntax

## next steps

1. pick variant-namespace rule and mutability default
1. prototype `enum` parser extension in `ruff_python_parser`
1. implement lowering transform in `crates/by_transforms/src/transforms/`
    emitting the dataclass tree
1. wire ty exhaustiveness check for `match` over enum types
1. write reverse transform consuming the new `LoweringKind` tags
