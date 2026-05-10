# typeof

basedpython adds a `typeof` keyword that yields the static type of an expression
in any annotation position:

```by
b: int = 1

a: typeof b = 1                # `a: int`
def f(x: typeof b) -> typeof b: ...
cb: typeof b | None            # `int | None`
```

transpiles to:

```python
from ty_extensions import TypeOf

b: int = 1

a: TypeOf[b] = 1
def f(x: TypeOf[b]) -> TypeOf[b]: ...
cb: TypeOf[b] | None
```

## scope

`typeof` is a basedpython-only keyword: a `.py` file using it produces a parse
error. when the next token cannot start an expression (e.g. `typeof = 5`),
`typeof` is parsed as an ordinary identifier so existing python code that uses
`typeof` as a name continues to parse

## polyfill

`typeof X` lowers to `ty_extensions.TypeOf[X]`. there is no runtime equivalent —
`TypeOf` is a type-checker-only construct evaluated by ty
