# cast

basedpython adds a `cast` infix soft keyword for inline type casts:

```by
b = a cast int
```

transpiles to:

```python
from typing import cast

b = cast(int, a)
```

## syntax

`cast` is an infix soft keyword: `<value> cast <type>`. the left operand is
the value being cast, the right operand is the target type

```by
b = a cast int | str             # cast(int | str, a)
f(a cast int)                    # f(cast(int, a))
```

## scope

`cast` is a basedpython-only keyword: a `.py` file using it as an infix produces
a parse error. when the next token cannot start an expression (e.g. `cast = 5`
or `cast(int, a)`), `cast` is parsed as an ordinary identifier so existing
python code that uses `cast` as a name or function call continues to parse

## polyfill

`<value> cast <type>` lowers to `cast(<type>, <value>)` with an injected
`from typing import cast` import. this is a runtime construct — `typing.cast`
returns its second argument unchanged at runtime
