# basedpython: typed lambda annotations

basedpython lambdas can carry parameter annotations and a return annotation that ty uses to infer
the full signature. annotations are type-only at runtime.

```toml
[environment]
python-version = "3.12"
```

## annotated lambda signature

```by
f = lambda (a: int, b: str) -> bool: a > 0
reveal_type(f(1, "hi"))  # revealed: bool
```

## return-annotation-only

```by
g = lambda (x): str(x)
h = lambda (x) -> str: str(x)
reveal_type(h(1))  # revealed: str
```

## default values still allowed

```by
greet = lambda (name: str = "world") -> str: f"hello, {name}"
reveal_type(greet())  # revealed: str
reveal_type(greet("ty"))  # revealed: str
```
