# Built-in Functions

seuil-rs implements all 47 JSONata built-in functions. Functions are called with the `$` prefix.

## String Functions

| Function | Signature | Description |
|----------|-----------|-------------|
| `$string` | `$string(arg)` | Cast to string |
| `$length` | `$length(str)` | String length |
| `$substring` | `$substring(str, start[, length])` | Extract substring |
| `$substringBefore` | `$substringBefore(str, chars)` | Substring before first occurrence |
| `$substringAfter` | `$substringAfter(str, chars)` | Substring after first occurrence |
| `$uppercase` | `$uppercase(str)` | Convert to uppercase |
| `$lowercase` | `$lowercase(str)` | Convert to lowercase |
| `$trim` | `$trim(str)` | Trim whitespace |
| `$pad` | `$pad(str, width[, char])` | Pad string to width |
| `$contains` | `$contains(str, pattern)` | Test if string contains pattern |
| `$split` | `$split(str, separator[, limit])` | Split string into array |
| `$join` | `$join(array[, separator])` | Join array into string |
| `$replace` | `$replace(str, pattern, replacement[, limit])` | Replace occurrences |
| `$match` | `$match(str, pattern)` | Regex match, returns match object |
| `$base64encode` | `$base64encode(str)` | Encode to base64 |
| `$base64decode` | `$base64decode(str)` | Decode from base64 |

### Examples

```text
$string(42)                          /* "42" */
$length("hello")                     /* 5 */
$substring("hello world", 6)         /* "world" */
$substringBefore("hello-world", "-") /* "hello" */
$substringAfter("hello-world", "-")  /* "world" */
$uppercase("hello")                  /* "HELLO" */
$lowercase("HELLO")                  /* "hello" */
$trim("  hello  ")                   /* "hello" */
$pad("hello", 10)                    /* "hello     " */
$pad("hello", -10)                   /* "     hello" */
$contains("hello", "ell")            /* true */
$split("a,b,c", ",")                 /* ["a", "b", "c"] */
$join(["a", "b", "c"], "-")          /* "a-b-c" */
$replace("hello", "l", "L")          /* "heLLo" */
$base64encode("hello")               /* "aGVsbG8=" */
$base64decode("aGVsbG8=")            /* "hello" */
```

Patterns can be strings or regular expressions (`/pattern/flags`):

```text
$contains("hello", /^he/)            /* true */
$split("a1b2c3", /[0-9]/)            /* ["a", "b", "c"] */
$replace("hello", /l+/, "L")         /* "heLo" */
$match("abc 123", /(\d+)/)           /* {"match": "123", ...} */
```

## Numeric Functions

| Function | Signature | Description |
|----------|-----------|-------------|
| `$number` | `$number(arg)` | Cast to number |
| `$abs` | `$abs(n)` | Absolute value |
| `$floor` | `$floor(n)` | Floor (round down) |
| `$ceil` | `$ceil(n)` | Ceiling (round up) |
| `$round` | `$round(n[, precision])` | Round to precision |
| `$power` | `$power(base, exp)` | Exponentiation |
| `$sqrt` | `$sqrt(n)` | Square root |
| `$random` | `$random()` | Random float in [0, 1) |
| `$sum` | `$sum(array)` | Sum of numeric array |
| `$max` | `$max(array)` | Maximum value |
| `$min` | `$min(array)` | Minimum value |
| `$average` | `$average(array)` | Arithmetic mean |

### Examples

```text
$number("42")                  /* 42 */
$abs(-5)                       /* 5 */
$floor(3.7)                    /* 3 */
$ceil(3.2)                     /* 4 */
$round(3.456, 2)               /* 3.46 */
$power(2, 10)                  /* 1024 */
$sqrt(144)                     /* 12 */
$sum([1, 2, 3])                /* 6 */
$max([1, 5, 3])                /* 5 */
$min([1, 5, 3])                /* 1 */
$average([1, 2, 3, 4])         /* 2.5 */
```

Aggregation functions are commonly used with the `~>` pipe operator:

```text
orders.amount ~> $sum()
prices ~> $max()
scores ~> $average()
```

## Array Functions

| Function | Signature | Description |
|----------|-----------|-------------|
| `$count` | `$count(array)` | Number of elements |
| `$append` | `$append(arr1, arr2)` | Concatenate arrays |
| `$sort` | `$sort(array[, comparator])` | Sort array |
| `$reverse` | `$reverse(array)` | Reverse array |
| `$shuffle` | `$shuffle(array)` | Randomly shuffle |
| `$distinct` | `$distinct(array)` | Remove duplicates |
| `$zip` | `$zip(arr1, arr2, ...)` | Zip multiple arrays |
| `$flatten` | `$flatten(array)` | Flatten nested arrays |

### Examples

```text
$count([1, 2, 3])                    /* 3 */
$append([1, 2], [3, 4])              /* [1, 2, 3, 4] */
$sort([3, 1, 2])                     /* [1, 2, 3] */
$reverse([1, 2, 3])                  /* [3, 2, 1] */
$distinct([1, 2, 2, 3, 3])           /* [1, 2, 3] */
$zip([1,2,3], ["a","b","c"])         /* [[1,"a"], [2,"b"], [3,"c"]] */
$flatten([[1,2], [3, [4,5]]])        /* [1, 2, 3, 4, 5] */
```

Custom sort with comparator:

```text
$sort(people, function($a, $b) { $a.age > $b.age })
```

## Object Functions

| Function | Signature | Description |
|----------|-----------|-------------|
| `$keys` | `$keys(obj)` | Array of keys |
| `$lookup` | `$lookup(obj, key)` | Get value by key |
| `$spread` | `$spread(obj)` | Array of `{key: value}` objects |
| `$merge` | `$merge(array)` | Merge array of objects |
| `$sift` | `$sift(obj, fn)` | Filter object entries |
| `$each` | `$each(obj, fn)` | Map over object entries |
| `$error` | `$error(msg)` | Throw an error |
| `$assert` | `$assert(condition, msg)` | Assert condition |
| `$type` | `$type(value)` | Type name as string |

### Examples

```text
$keys({"a": 1, "b": 2})                     /* ["a", "b"] */
$lookup({"a": 1, "b": 2}, "a")              /* 1 */
$spread({"a": 1, "b": 2})                   /* [{"a": 1}, {"b": 2}] */
$merge([{"a": 1}, {"b": 2}])                /* {"a": 1, "b": 2} */
$type(42)                                     /* "number" */
$type("hello")                                /* "string" */
$type([1, 2])                                 /* "array" */
$type({"a": 1})                               /* "object" */
$type(true)                                   /* "boolean" */
$type(null)                                   /* "null" */
```

## Type Functions

| Function | Signature | Description |
|----------|-----------|-------------|
| `$boolean` | `$boolean(arg)` | Cast to boolean |
| `$not` | `$not(arg)` | Boolean negation |
| `$exists` | `$exists(arg)` | Test if value exists (not undefined) |

### Examples

```text
$boolean(0)          /* false */
$boolean(1)          /* true */
$boolean("")         /* false */
$boolean("hello")    /* true */
$not(true)           /* false */
$exists(missing)     /* false */
$exists(name)        /* true (if name is defined) */
```

## Date/Time Functions

| Function | Signature | Description |
|----------|-----------|-------------|
| `$now` | `$now()` | Current timestamp as ISO 8601 string |
| `$millis` | `$millis()` | Current time as milliseconds since epoch |
| `$fromMillis` | `$fromMillis(ms[, picture[, tz]])` | Milliseconds to formatted string |
| `$toMillis` | `$toMillis(str[, picture])` | Formatted string to milliseconds |

### Examples

```text
$now()                                 /* "2024-01-15T10:30:00.000Z" */
$millis()                              /* 1705312200000 */
$fromMillis(1705312200000)             /* "2024-01-15T10:30:00.000Z" */
$toMillis("2024-01-15T10:30:00.000Z") /* 1705312200000 */
```

Date/time functions use the injectable `Environment` trait, so they are deterministic when using `MockEnvironment`. See [Deterministic Testing](../api/deterministic.md).
