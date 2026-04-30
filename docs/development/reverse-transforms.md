# reverse transforms

basedpython can convert standard Python source back into basedpython syntax. this is the inverse of the normal transpilation pipeline

## usage

```sh
by transpile --reverse file.py
```

## what it does

reverse transforms detect patterns in standard Python that correspond to basedpython idioms and rewrite them back. this enables round-tripping: a Python file run through `--reverse` and then transpiled forward should produce code with the same AST as the original


## implemented reverse transforms

### empty class bodies

detects `class A: ...` (with a `pass` or `...` body) and rewrites it to the basedpython shorthand:

```python
# standard Python
class A: ...

# basedpython
class A
```

### literal types

detects `Literal["foo", 5]` in annotations and rewrites to basedpython's inline literal syntax:

```python
# standard Python
from typing import Literal
a: Literal["foo", 5]

# basedpython
a: "foo" | 5
```

### subscription normalization

detects the trailing-comma 1-tuple form `x[(a, b),]` and rewrites it back to the natural parenthesized form:

```python
# standard Python (transpiled)
x[(a, b),]

# basedpython
x[(a, b)]
```

## design

each reverse transform lives in `src/reverse_transforms/` and mirrors the structure of the forward transforms in `src/transforms/`. they use the same visitor-based approach: walk the AST, detect the polyfill pattern, and emit text edits to rewrite it back to basedpython syntax
