# basedpython: typed dict literal type expressions

In basedpython, `{"key": T, ...}` in a type position is sugar for an inline `typing.TypedDict`
subclass. ty synthesizes a `TypedDict` class per unique shape (matching the transpiler) so key
access on an instance resolves to the field's declared type. Identity is shape-based: two
structurally identical typed-dict literals in the same file resolve to the same class.

## Type-expression position

### As a variable annotation

```by
def make(name: str, age: int) -> None:
    a: {"name": str, "age": int} = {"name": name, "age": age}
    reveal_type(a["name"])  # revealed: str
    reveal_type(a["age"])  # revealed: int
```

### As a parameter annotation

```by
def f(x: {"name": str, "age": int}) -> None:
    reveal_type(x["name"])  # revealed: str
    reveal_type(x["age"])  # revealed: int
```

### As a return annotation

```by
def g() -> {"name": str, "age": int}:
    return {"name": "asdf", "age": 1}

reveal_type(g()["name"])  # revealed: str
reveal_type(g()["age"])  # revealed: int
```

### Single-field typed dict

```by
def h(x: {"only": int}) -> None:
    reveal_type(x["only"])  # revealed: int
```

### Extra-items marker `**: T`

A `**: T` entry in a dict literal type lowers to `extra_items=T` on the synthesized `TypedDict`.
ty's extra-items semantics are still TODO, so the declared fields still type-check but extra keys
aren't yet enforced.

```by
def f(x: {"name": str, **: int}) -> None:
    reveal_type(x["name"])  # revealed: str
```

## Python-passthrough — dict literal in type position is still an error

```py
a: {"name": str, "age": int}  # error: [invalid-type-form]
```
