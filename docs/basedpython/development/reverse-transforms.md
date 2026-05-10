# reverse transforms

basedpython can convert standard Python source back into basedpython syntax. inverse of normal transpilation

## usage

```sh
by transpile --reverse file.py
```

## what it does

reverse transforms detect patterns in standard Python that correspond to basedpython idioms and rewrite them back. enables round-tripping: a Python file passed through `--reverse` then transpiled forward should produce code with the same AST as the original

after the rewrites run, any `from … import …` line whose bindings are no longer referenced (e.g. `from typing import Callable` after `Callable[...]` was rewritten to the arrow form) is pruned from the output so the reversed source isn't carrying dead imports

## design

each reverse transform lives in `src/reverse_transforms/` and mirrors the forward transform of the same name in `src/transforms/`. they share the visitor-based approach: walk the AST, detect the lowered shape, emit text edits to rewrite back to the basedpython surface form

forward transforms drive lowering; reverse transforms drive the inverse rewrite. the two directions stay paired — a new forward transform should be accompanied by a reverse transform unless the lowering is intentionally lossy or unobservable in the produced Python
