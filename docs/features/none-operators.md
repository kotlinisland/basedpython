# none operators

basedpython adds two operators for working with `None` values: the none-coalescing operator `??` and the none-chaining operator `?.`

## none-coalescing `??`

`a ?? b` evaluates to `a` when `a` is not `None`, otherwise `b`

```python
# basedpython
x = a ?? b
```
```python
# generated Python
x = a if a is not None else b
```

### interaction with `?.`

when the left-hand side of `??` contains a `?.` chain, the two operators compose naturally:

```python
# basedpython
x = a?.a.b ?? 1
```
```python
# generated Python
x = _t if (_t := None if a is None else a.a.b) is not None else 1
```

## none-chaining `?.`

`a?.b` evaluates to `None` when `a` is `None`, otherwise `a.b`. this avoids verbose `if x is not None` checks

```python
# basedpython
x = a?.b
```
```python
# generated Python
x = None if a is None else a.b
```

### chained access

multiple `?.` operators can be chained. basedpython uses walrus assignments to avoid evaluating intermediate expressions more than once:

```python
# basedpython
x = a?.a?.b
```
```python
# generated Python
x = None if a is None else None if (_t := a.a) is None else _t.b
```

longer chains work the same way:

```python
# basedpython
x = a?.b?.c?.d
```
```python
# generated Python
x = None if a is None else None if (_t := a.b) is None else None if (_t := _t.c) is None else _t.d
```

### mixed chains

regular `.` and optional `?.` access can be mixed freely:

```python
# basedpython
x = a?.b.c
```
```python
# generated Python
x = None if a is None else a.b.c
```

when a `?.` follows a regular attribute access, the compound expression is assigned to a temporary:

```python
# basedpython
x = a.b?.c
```
```python
# generated Python
x = None if (_t := a.b) is None else _t.c
```

### temporary variable naming

the generated code uses `_t` as the temporary variable name. if `_t` is already in scope, basedpython falls back to `_t0`, `_t1`, etc:

```python
# basedpython
_t = 1
x = a?.a?.b
```
```python
# generated Python
_t = 1
x = None if a is None else None if (_t0 := a.a) is None else _t0.b
```
