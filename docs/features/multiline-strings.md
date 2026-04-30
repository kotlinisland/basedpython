# multiline string dedenting

basedpython strips common leading indentation from triple-quoted multiline strings at compile time, so you can indent string content to match the surrounding code without affecting the runtime value

## transformation

```python
# basedpython
text = """
    start-of-line
    a
    """
```
```python
# generated Python
text = """\
start-of-line
a\
"""
```

the generated string has no leading indentation and uses backslash continuations to avoid extra newlines at the start and end

## when it applies

the transform applies to all triple-quoted single-part strings (plain, f-string, t-string, r-string) that:

- open with `"""\n` (the opening quotes are immediately followed by a newline)
- have content that is consistently indented relative to the closing `"""`

## motivation

in Python, triple-quoted strings preserve all whitespace literally. this forces you to either break indentation:

```python
def example():
    text = """\
start-of-line
a"""
```

or accept unwanted leading spaces at runtime:

```python
def example():
    text = """
        start-of-line
        a
        """
    # text starts with "\n        start-of-line\n..."
```

basedpython lets you write naturally indented strings and removes the common indentation at compile time, similar to Kotlin's `trimIndent()` or Java's text blocks
