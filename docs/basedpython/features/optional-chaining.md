# optional chaining

`a?.b` short-circuits to `None` when `a is None`, otherwise evaluates `a.b`:

```by
city = user?.address.city
```

transpiles to:

```python
city = None if user is None else user.address.city
```

## chains

each `?.` introduces a new short-circuit guard. multi-step chains share a
temp variable so each prefix is evaluated only once:

```by
country = user?.address?.country
```

transpiles to:

```python
country = None if user is None else None if (_t := user.address) is None else _t.country
```

mixed chains — `?.` followed by regular `.` — only guard at the explicit
optional steps:

```by
zip = user?.address.zip
# → None if user is None else user.address.zip
```

`a.b?.c` works in the obvious way: only the part after `a.b` is short-circuited
through a temp variable

## scope

`?.` is recognized in attribute-access expressions only. method calls,
subscripts, and arbitrary call expressions on the optional value are not yet
supported in the surface syntax — fall back to a guard expression

## interaction with `??`

see [none-coalesce operator](none-coalesce.md). `?.` composes with `??`
without re-evaluating the chained prefix
