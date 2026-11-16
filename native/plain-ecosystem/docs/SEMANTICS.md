# Plain language semantics (implemented behaviour)

This page describes what the compiler and runtime actually do. If code and this page disagree, that is a bug; please report it.

## Values and types

Values are dynamically typed: integers (64-bit), floats, booleans, strings, lists, dictionaries, errors, and `null` (what a function returns when it returns nothing). Type annotations (`AS int`, `GIVES string`, ...) are checked at compile time only where the compiler can see the types (for example literal arguments to `EXTERN` functions and `LET x AS int = "text"`); they do not change run-time behaviour.

## Numbers

- Integer `+ - *` are exact 64-bit two's-complement and **wrap on overflow** (the same as the compiler's native-integer fast path).
- Integer `/` truncates toward zero; `%` has the sign of the dividend. Division or modulo by zero is an error value.
- If either operand is a float, the operation is done in floating point and returns a float.
- `ABS`, `FLOOR`, `CEIL` keep integers exact.

## Equality and ordering

- `=` / `!=` are type-aware: `1 = 1.0` is TRUE, `1 = "1"` is FALSE. Lists and dictionaries compare element by element.
- `< <= > >=` work on two numbers or two strings. Anything else (for example `1 < "2"`) is an error. Comparisons involving NaN are FALSE.
- An error operand makes the result that error (it is never silently turned into TRUE).

## Truthiness and boolean operators

Falsy: `FALSE`, `0`, `0.0`, `""`, empty list, empty dictionary, `null`, and errors. Everything else is truthy. `NOT`, `AND` and `OR` return booleans. `AND` / `OR` **short-circuit**. `NOT` applies to the whole comparison that follows it: `NOT x > 5` means `NOT (x > 5)`.

## Scoping

Variables are block-scoped: a `LET` inside `IF`, `REPEAT`, `TRY` or `CATCH` is not visible after the block. Declare the variable before the block and use `SET` inside it. A loop variable shadows an outer variable of the same name. Functions see only their parameters and locals (not top-level variables). In methods, fields are visible by name; parameters and locals shadow fields (use `self.name` to reach the field).

`SET a, b = b, a` evaluates every right-hand side first, then assigns (so it swaps).

## Loops

`REPEAT i FROM a TO b` evaluates `a` and `b` once and includes both ends. `FOR EACH` evaluates the collection once. `CONTINUE` runs the loop's increment; `BREAK` leaves the loop. Both are compile errors outside a loop.

## Errors

Errors are values. Operations that fail (division by zero, bad index, missing dictionary key, failed file access, `FAIL "message"`) produce an error value. Inside `TRY`, every statement - including statements nested in `IF` / `REPEAT` blocks, conditions, `SET` targets and constructor calls - checks its result; the first error jumps to `CATCH`, where the caught variable holds the error. Outside `TRY` an error value flows on as an ordinary value (printing it shows `Error: message`; adding it to a string contributes the message). `FAIL` is only allowed inside a function or a `TRY`.

## Classes

```
CLASS Animal
    LET name AS string          # fields are declared with a type; they start as a default of that type
    LET legs AS int = 4         # or with a constant default
    TO NEW n AS string          # constructor
        SET name = n
    END TO
END CLASS
CLASS Bird EXTENDS Animal
    TO NEW n AS string
        SUPER.NEW(n)            # run the parent constructor
        SET legs = 2
    END TO
    TO speak GIVES string
        GIVE BACK SUPER.speak() + " (tweet)"   # call the parent's version
    END TO
END CLASS
```

- `NEW Class(args)` creates an instance (a dictionary tagged with its class), sets **every field** (inherited ones first) to its declared default or to a default for its type (`0`, `0.0`, `FALSE`, `""`, `[]`, `{}`, `null` for `any`), then calls the constructor `TO NEW`. Defaults must be constant expressions of the field's type.
- A subclass without its own `TO NEW` inherits the parent's. A subclass constructor does **not** call the parent's automatically: use `SUPER.NEW(args)`.
- A method call dispatches on the **run-time class** of the object, so subclasses can override methods. `SUPER.method(args)` calls the parent's implementation directly.
- Inside a method, `name(args)` (without `self.`) calls a method of the same class hierarchy; `self.name(args)` does the same explicitly. Parameters and locals shadow fields; use `self.field` to reach a shadowed field.
- `value IS Class` is `TRUE` when `value` is an instance of `Class` or one of its subclasses (and `FALSE` for anything else, including non-objects).
- `EXTENDS` must name a class defined in the program; circular inheritance is an error. The arity of constructors, methods, functions and built-ins is checked at compile time.

## Printing

`SAY` prints integers, floats, `TRUE`/`FALSE`, strings, lists as `[1, 2, 3]` (recursively), dictionaries as `{key: value}` with keys sorted, and objects as `ClassName{field: value}`.

## Limits

`RANGE` builds at most 10,000,000 items. Memory is never reclaimed (see `SECURITY.md`). `ASK` returns an error value for unparsable input.

## Standard library

67 built-in functions are documented in [`STDLIB.md`](STDLIB.md), which is generated from `stdlib.json` (`python3 scripts/gen_stdlib_docs.py`). Built-in names are upper case and case-sensitive. They never panic: a failure is an error value. A function you define yourself with the same name as one of the newer library functions (for example `SUM` or `MAX`) takes priority over the library function. Neural-network functions (`plain_nn_*`) are described in [`ML.md`](ML.md).

## Command line

- `plain file.plain` compiles and runs a program; `plain build file.plain` only builds it; `plain emit-ir file.plain` writes the LLVM IR.
- `plain check file.plain` runs the whole compiler (parser, semantic checks, code generation and LLVM verification) and writes nothing. Editors use it for live diagnostics.
- `PLAIN_OPT` (`-O0` .. `-O3`, `-Os`, `-Oz`) sets the optimisation level (default `-O2`). `PLAIN_ALLOW_EXTERNAL_IMPORTS=1` lets `USE` reach outside the project directory.
