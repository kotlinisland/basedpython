# basedpython: `cast` keyword

In basedpython, `<value> cast <type>` is sugar for `typing.cast(<type>, <value>)`. The transpiler
lowers it to a `cast()` call and adds the import.

## simple cast

```by
a = 1
b = a cast int
reveal_type(b)  # revealed: int
```

## cast to union

```by
a = 1
b = a cast int | str
reveal_type(b)  # revealed: int | str
```

## cast in call argument

```by
def f(x: int) -> int:
    return x

a = 1
reveal_type(f(a cast int))  # revealed: int
```

## not valid in `.py` files

`cast` as an infix soft keyword is basedpython-only. A `.py` file using it gets a parse error from
the parser.

```py
a = 1
# error: [invalid-syntax] "`cast` keyword is not valid in .py files"
b = a cast int
```

## regular `cast` call still works

A bare `cast(...)` call is parsed as an ordinary function call in both `.by` and `.py` files.

```py
from typing import cast

a = 1
b = cast(int, a)
reveal_type(b)  # revealed: int
```
