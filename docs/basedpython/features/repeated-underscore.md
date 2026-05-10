# repeated `_` parameters

basedpython lets `_` appear more than once in a function or lambda signature.
python rejects this as a duplicate-parameter error; basedpython only
exempts `_` (the unused-argument convention), so signatures that ignore
several arguments don't need invented placeholders:

```by
def f(_, _):
    print(_)

g = lambda _, _: 1
```

references to `_` in the body resolve to the first parameter. duplicates of
any other name remain a syntax error
