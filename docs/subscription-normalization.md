# subscription normalization

when the subscript key is a tuple, basedpython normalizes it into an explicit 1-tuple so that the intent is unambiguous

## transformation rules

| basedpython | Python output |
|---|---|
| `x[(a, b)]` | `x[(a, b),]` |
| `x[a, b]` | `x[(a, b),]` |
| `x[a]` | `x[a]` *(unchanged)* |

Both the parenthesized form `x[(a, b)]` and the bare form `x[a, b]` produce identical output. 
a scalar subscript is never modified.

## motivation

in Python, `x[a, b]` and `x[(a, b)]` are identical: both pass the tuple `(a, b)` as the subscript key. 
the parentheses in `x[(a, b)]` are purely cosmetic — they do not change the semantics, and a reader cannot 
tell whether the author intended a multi-index or a single tuple-valued key

this is inconsistent with call expressions `x(a, b)`/`x((a, b))`

basedpython resolves this by making tuple keys explicit. the trailing comma in `x[(a, b),]` is unambiguous: 
the subscript is a 1-tuple whose single element is `(a, b)`. A custom `__getitem__` that receives `((a, b),)` 
instead of `(a, b)` can trivially unwrap it; more importantly, the source clearly communicates the intent

```python
# basedpython: "I am indexing with the key (a, b)"
grid[(row, col)]     # → grid[(row, col),]

# basedpython: same output regardless of how you wrote it
grid[row, col]       # → grid[(row, col),]

# basedpython: scalar key, untouched
grid[row]            # → grid[row]
```

## breaking change from Python

any Python code that passes a tuple key using the parenthesized form `x[(a, b)]` will behave differently after compilation. in Python, `x[(a, b)]` calls `__getitem__` with `(a, b)`; in basedpython it calls `__getitem__` with `((a, b),)`.

this affects classes whose `__getitem__` is defined outside basedpython (third-party libraries, C extensions). if you are indexing into such a type with a tuple key, use a variable to avoid the rewrite:

```python
key = (a, b)
x[key]   # scalar subscript — not rewritten
```

## interaction with `__getitem__`

if you are implementing `__getitem__` on a basedpython-compiled class, expect tuple keys to always arrive wrapped in a 1-tuple:

```python
class Grid:
    def __getitem__(self, key):
        # key is ((row, col),) — unwrap before use
        (row, col), = key
        ...
```

for classes consumed by external Python code that is not compiled by basedpython, be aware that the key shape changes.
this is intentional: it makes the calling convention explicit in the source
