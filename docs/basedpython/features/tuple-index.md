# tuple member access

dot-access tuple elements by position. `expr.N` where `N` is a non-negative
integer lowers to `expr[N]`:

```by
pair = (1, 2)
first = pair.0
second = pair.1
```

transpiles to:

```python
pair = (1, 2)
first = pair[0]
second = pair[1]
```

works on any expression, not just name bindings. literal tuples need no
parentheses beyond the tuple itself:

```by
x = (1, 2).0
```

transpiles to:

```python
x = (1, 2)[0]
```

## chaining

multiple dot indices compose, and mix freely with regular attribute access,
calls, and subscripts:

```by
nested = (1, (2, 3)).1.0
attr   = pair.0.bit_length()
sliced = matrix.0[k]
```

transpiles to:

```python
nested = (1, (2, 3))[1][0]
attr   = pair[0].bit_length()
sliced = matrix[0][k]
```

## scope

`expr.N` is the only basedpython form of dot-indexing. negative indices,
slices, and non-digit selectors fall through to ordinary attribute access
and remain `Expr.attr` in the AST

## see also

- [keyword arguments in subscripts](kw-subscript.md) — `x[a, z=1]` lowering
