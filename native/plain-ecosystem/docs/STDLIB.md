# Plain standard library

Generated from `docs/stdlib.json` by `scripts/gen_stdlib_docs.py` - edit the JSON, not this file.
Functions never panic: a failure returns an error value that `TRY`/`CATCH` and `IS_ERROR` can handle. A function you define yourself with the same name as a *new* library function (`SUM`, `MAX`, ...) takes priority.

## Strings

| Function | Returns | Description |
|---|---|---|
| `LENGTH(value)` | int | Characters in a string, items in a list, or keys in a dictionary. |
| `UPPER(text)` | string | Upper-case copy of a string. |
| `LOWER(text)` | string | Lower-case copy of a string. |
| `TRIM(text)` | string | Removes whitespace from both ends of a string. |
| `SPLIT(text, separator)` | list | Splits text at every separator. An empty separator splits into characters. |
| `JOIN(list, separator)` | string | Joins the items of a list into one string. |
| `REPLACE(text, old, new)` | string | Replaces every occurrence of `old` with `new`. An empty `old` leaves the text unchanged. |
| `SUBSTRING(text, start, length)` | string | Up to `length` characters starting at character index `start` (counted in characters, not bytes). Negative arguments are an error. |
| `INDEX_OF(text_or_list, item)` | int | Character index of the first match in a string, or index of the first equal item in a list. -1 when not found. |
| `REVERSE(text_or_list)` | string or list | Reversed copy of a string or a list. |
| `REPEAT_STRING(text, times)` | string | The text repeated `times` times (result limited to 100,000,000 bytes). |
| `CONTAINS(text, part)` | bool | TRUE when `part` occurs inside `text`. |
| `STARTS_WITH(text, prefix)` | bool | TRUE when the text begins with `prefix`. |
| `ENDS_WITH(text, suffix)` | bool | TRUE when the text ends with `suffix`. |

```plain
SAY LENGTH("héllo")
SAY UPPER("plain")
SAY LOWER("PLAIN")
SAY TRIM("  hi  ")
LET parts = SPLIT("a,b,c", ",")
SAY JOIN(["a", "b"], "-")
SAY REPLACE("a-b", "-", "+")
SAY SUBSTRING("héllo", 1, 3)
SAY INDEX_OF("hello", "ll")
SAY REVERSE("abc")
SAY REPEAT_STRING("ab", 3)
IF CONTAINS(line, "ERROR") THEN
SAY STARTS_WITH("plain", "pl")
SAY ENDS_WITH("plain", "in")
```

## Conversion and types

| Function | Returns | Description |
|---|---|---|
| `TO_STRING(value)` | string | Text form of any value (the same text SAY would print for a string). |
| `TO_INT(value)` | int | Converts a string (surrounding spaces allowed), float (truncated), boolean or int to an integer. Invalid input is an error. |
| `TO_FLOAT(value)` | float | Converts a string, int or boolean to a float. Invalid input is an error. |
| `TYPE_OF(value)` | string | "int", "float", "bool", "string", "list", "dict", "error", "null", "handle", or the class name for an object. |
| `IS_ERROR(value)` | bool | TRUE when the value is an error value. |
| `ERROR_MESSAGE(value)` | string | The message of an error value. |

```plain
SAY TO_STRING(42) + "!"
LET n = TO_INT("42")
LET x = TO_FLOAT("2.5")
SAY TYPE_OF(dog)
IF IS_ERROR(result) THEN
SAY ERROR_MESSAGE(result)
```

## Math

| Function | Returns | Description |
|---|---|---|
| `ABS(number)` | number | Absolute value. Integers stay exact. |
| `SQRT(number)` | float | Square root. A negative number is an error. |
| `POW(base, exponent)` | float | `base` raised to `exponent`. |
| `FLOOR(number)` | int | Largest integer not above the number. |
| `CEIL(number)` | int | Smallest integer not below the number. |
| `ROUND(number)` | int | Nearest integer; halves round away from zero. |
| `SIN(angle)` | float | Sine of an angle in radians. |
| `COS(angle)` | float | Cosine of an angle in radians. |
| `TAN(angle)` | float | Tangent of an angle in radians. |
| `EXP(number)` | float | e raised to the number. |
| `LOG(number)` | float | Natural logarithm. A number that is not positive is an error. |
| `PI()` | float | The constant pi. |
| `SIGN(number)` | int | -1, 0 or 1 according to the sign. |
| `MOD(a, b)` | number | Remainder (the same as `a % b`). A zero divisor is an error. |
| `MIN(a, b)` | number or string | The smaller of two numbers or two strings (the original value is returned). |
| `MAX(a, b)` | number or string | The larger of two numbers or two strings. |
| `CLAMP(value, low, high)` | number | Limits `value` to the range low..high. `low` must not exceed `high`. |

```plain
SAY ABS(-5)
SAY SQRT(81)
SAY POW(2, 8)
SAY FLOOR(2.7)
SAY CEIL(2.1)
SAY ROUND(2.5)
SAY SIN(0)
SAY COS(0)
SAY TAN(0)
SAY EXP(1)
SAY LOG(10)
SAY PI()
SAY SIGN(-9)
SAY MOD(17, 5)
SAY MIN(3, 7)
SAY MAX(3, 7)
SAY CLAMP(15, 0, 10)
```

## Random numbers

| Function | Returns | Description |
|---|---|---|
| `RANDOM()` | float | A random float in [0, 1). |
| `RANDOM_INT(low, high)` | int | A random integer from low to high, both included. |
| `SEED(number)` | int | Makes the random sequence reproducible: the same seed always gives the same numbers. |

```plain
SAY RANDOM()
SAY RANDOM_INT(1, 6)
SEED(42)
```

## Lists and dictionaries

| Function | Returns | Description |
|---|---|---|
| `APPEND(list, item)` | bool | Adds an item to the end of a list (in place). Returns TRUE. |
| `REMOVE(list, index)` | any | Removes and returns the item at `index`. An index out of range is an error. |
| `RANGE(start, end)` | list | The integers from `start` to `end`, both included (at most 10,000,000 items). |
| `SORT(list)` | list | A new list in ascending order. Works for a list of numbers or a list of strings. |
| `SLICE(list_or_text, start, end)` | list or string | The part from `start` up to (not including) `end`. Indexes past the end are clamped; negative indexes are an error. |
| `CONCAT(list_a, list_b)` | list | A new list with the items of both lists. |
| `UNIQUE(list)` | list | The distinct items in first-seen order (quadratic time, meant for modest lists). |
| `COPY(value)` | list or dict | A shallow copy of a list or dictionary. |
| `SUM(list)` | number | Sum of a list of numbers. An empty list gives 0. |
| `LIST_MIN(list)` | any | Smallest item of a non-empty list of numbers or strings. |
| `LIST_MAX(list)` | any | Largest item of a non-empty list of numbers or strings. |
| `KEYS(dict)` | list | The keys of a dictionary, sorted. |
| `VALUES(dict)` | list | The values of a dictionary in key order. |
| `HAS_KEY(dict, key)` | bool | TRUE when the dictionary has the key. |
| `DELETE_KEY(dict, key)` | bool | Removes a key. Returns TRUE when the key existed. |

```plain
APPEND(nums, 4)
LET last = REMOVE(nums, 2)
REPEAT FOR EACH i IN RANGE(1, 5)
SAY SORT([3, 1, 2])
SAY SLICE([1, 2, 3, 4], 1, 3)
SAY CONCAT([1], [2, 3])
SAY UNIQUE([1, 2, 1])
LET copy = COPY(nums)
SAY SUM([1, 2, 3])
SAY LIST_MIN([4, 2, 9])
SAY LIST_MAX([4, 2, 9])
SAY KEYS(person)
SAY VALUES(person)
IF HAS_KEY(person, "age") THEN
DELETE_KEY(person, "age")
```

## Files, time and the process

| Function | Returns | Description |
|---|---|---|
| `READ_FILE(path)` | string | The whole contents of a text file. Failure is an error. |
| `WRITE_FILE(path, text)` | bool | Writes (and overwrites) a text file. |
| `APPEND_FILE(path, text)` | bool | Appends text to a file, creating it if needed. |
| `READ_LINES(path)` | iterator | Reads a file line by line; use it with REPEAT FOR EACH. |
| `FILE_EXISTS(path)` | bool | TRUE when the path exists. |
| `ENV(name)` | string | The value of an environment variable. A missing variable is an error. |
| `ARGS()` | list | The command-line arguments after the program name. |
| `NOW_MS()` | int | Milliseconds since the Unix epoch. |
| `SLEEP(milliseconds)` | int | Pauses the program (0 to 3,600,000 ms). Output is flushed first. |
| `EXIT(code)` | never returns | Flushes output and ends the program with an exit status. |

```plain
LET text = READ_FILE("notes.txt")
WRITE_FILE("out.txt", "hello")
APPEND_FILE("log.txt", "line\n")
REPEAT FOR EACH line IN READ_LINES("log.txt")
IF FILE_EXISTS("config.json") THEN
SAY ENV("HOME")
LET args = ARGS()
LET started = NOW_MS()
SLEEP(250)
EXIT(0)
```

## JSON

| Function | Returns | Description |
|---|---|---|
| `JSON_ENCODE(value)` | string | JSON text for a number, boolean, string, list, dictionary or null. Dictionary keys are sorted and an object loses its hidden class tag. Errors cannot be encoded. |
| `JSON_DECODE(text)` | any | Parses JSON into lists, dictionaries, numbers, booleans, strings and null. Whole numbers become integers. Invalid JSON is an error that names the position. |

```plain
SAY JSON_ENCODE({name: "Rex", tags: [1, 2]})
LET data = JSON_DECODE(READ_FILE("data.json"))
```
