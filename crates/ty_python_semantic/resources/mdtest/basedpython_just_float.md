# basedpython: `float` and `complex` are exact

Python's typing spec special-cases `float` to mean `int | float` and `complex` to mean
`int | float | complex`. basedpython does not — `float` is just `float`, and `complex` is just
`complex`. The transpiler restores Python semantics by rewriting bare `float` / `complex` in type
positions to `ty_extensions.JustFloat` / `ty_extensions.JustComplex`.

## bare `float` annotation in `.by`

`float` in a `.by` annotation rejects `int` values

```by
x: float = 1.0
y: float = 1  # error: [invalid-assignment]
```

## bare `complex` annotation in `.by`

`complex` in a `.by` annotation rejects `int` and `float` values

```by
a: complex = 1j
b: complex = 1.0  # error: [invalid-assignment]
c: complex = 1  # error: [invalid-assignment]
```

## `float` parameter rejects `int` argument

```by
def f(x: float) -> None: ...

f(1.0)
f(1)  # error: [invalid-argument-type]
```

## `float` propagates through generic subscript

```by
def takes(xs: list[float]) -> None: ...

takes([1.0, 2.0])
takes([1, 2])  # error: [invalid-argument-type]
```

## `.py` keeps Python's special-case semantics

A `.py` file uses the typing-spec special case — `float` annotation accepts `int`, `complex` accepts
`int` and `float`.

`mod.py`:

```py
def takes_float(x: float) -> None: ...
def takes_complex(x: complex) -> None: ...

takes_float(1)  # accepted under typing spec
takes_complex(1.0)  # accepted under typing spec
```

## `.py` exporter, `.by` consumer

A function imported from a `.py` file keeps its Python type semantics. The `.py` annotation permits
`int`, even when called from a `.by` file.

`pylib.py`:

```py
def takes_float(x: float) -> None: ...
```

`consumer.by`:

```by
from pylib import takes_float

takes_float(1.0)
takes_float(1)  # accepted: pylib.py uses Python's `float` special case
```

## `.by` exporter, `.py` consumer

A `.by` annotation rewrites to `JustFloat`, so a `.py` consumer importing it gets the strict type.
Passing an `int` is rejected.

`bylib.by`:

```by
def takes_float(x: float) -> None: ...
```

`consumer.py`:

```py
from bylib import takes_float

takes_float(1.0)
takes_float(1)  # error: [invalid-argument-type]
```

## composes with union

```by
def f(x: float | None) -> None: ...

f(1.0)
f(None)
f(1)  # error: [invalid-argument-type]
```

## shadowed `float` is left alone

A local rebinding shadows the builtin — the annotation keeps that local meaning.

```by
float = int
x: float = 1  # accepted: `float` here is the local alias for `int`
```
