# `super` keyword

basedpython lets you reference `super` like a name instead of a call:

```by
class B(A):
    def f(self):
        super.greet()
```

transpiles to:

```python
class B(A):
    def f(self):
        super().greet()
```

## the bare form — `super.x`

`super.x` desugars to `super().x`. python's zero-arg `super()` requires a
method context (it pulls the enclosing class from the `__class__` cell), so
`super.x` outside a method is a runtime error — exactly like the python it
lowers to

## targeted form — `super[T].x`

`super[T].x` resolves `x` against the MRO entry that **precedes** `T` in the
enclosing class' MRO

example:

```by
class A:
    def f(self): ...

class B:
    def f(self): ...

class C(A, B):
    def f(self):
        super[B].f()  # → super(A, self).f()
```

C's MRO is `(C, A, B, object)`; the entry preceding `B` is `A`, so the bound
super starts the search after `A` and lands on `B.f`
