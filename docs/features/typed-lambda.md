# typed lambda

standard python lambdas cannot have type annotations, which makes it impossible for type checkers to understand their signatures without contextual `Callable` type hints

basedpython extends lambda syntax to support inline type annotations:

```bython
a = lambda (a: int, b: str) -> int: a + b
```

this is equivalent to the following python, with full type information preserved:

```python
a = lambda a, b: a + b
```

## syntax

```
lambda_typed ::= "lambda" "(" [params] ")" ["->" expr] ":" expr
```

parameters follow the same annotation rules as function definitions - all parameter kinds are supported:

```bython
f = lambda (x: int, y: str = "hi") -> bool: x > 0
g = lambda (*args: int, **kwargs: str) -> None: None
h = lambda (a: int, /, b: str, *, c: float) -> int: 0
```

## type checking

the type checker uses the annotations to infer the full signature of the lambda expression, enabling:

- parameter type errors at call sites
- return type mismatch errors when `->` annotation is present
- proper inference when passing typed lambdas to typed callables

example - type error when calling with wrong argument types:

```bython
f = lambda (x: int) -> str: str(x)
f("not an int")  # error: expected int, got str
```

## transpilation

the transform strips all annotations, producing a standard python lambda:

```bython
lambda (a: int, b: str) -> int: a + b
```
becomes:

```python
lambda a, b: a + b
```
