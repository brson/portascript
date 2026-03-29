# Portascript

A single-binary, cross-platform scripting language in Rust.
Bash-like command-oriented semantics without the footguns.
Built on [uutils](https://github.com/uutils/coreutils) for portable coreutils builtins.

File extension: `.psc`

```portascript
let name = $(exec git config user.name)
let files = $(run ls src)

for f in lines(files) {
    if ends_with(f, ".rs") {
        run echo "{name} owns {f}"
    }
}
```

## Table of Contents

- [CLI Usage](#cli-usage)
- [Language Guide](#language-guide)
  - [Two Worlds](#two-worlds)
  - [Types](#types)
  - [Variables](#variables)
  - [Strings](#strings)
  - [Commands](#commands)
  - [Pipelines](#pipelines)
  - [Capture](#capture)
  - [Error Handling](#error-handling)
  - [Operators](#operators)
  - [Control Flow](#control-flow)
  - [Functions](#functions)
  - [Builtin Functions](#builtin-functions)
  - [Environment Variables](#environment-variables)
  - [Script Arguments](#script-arguments)
- [Bash Comparison](#bash-comparison)
- [Bash Conversion Guide](#bash-conversion-guide)
- [Supported Builtins](#supported-builtins)

## CLI Usage

```
portascript script.psc [args...]
```

Scripts receive arguments via the `args` builtin list variable.
`args[0]` is the script path; `args[1..]` are user arguments.

## Language Guide

### Two Worlds

Portascript has two syntactic modes with an explicit boundary:

- **Commands** (`run`/`exec` keywords) -- CLI-statement oriented,
  space-separated string arguments, stdin/stdout/stderr, exit codes.
- **Expressions** (everything else) -- typed values, function calls
  with `()`, operators, precedence.

The `run` and `exec` keywords are the only way to invoke commands.
A line starting with `run` or `exec` is a command. Anything else
is control flow or an expression statement.

### Types

Six types. No user-defined types. No null.

| Type  | Literal            | Notes                          |
|-------|--------------------|--------------------------------|
| str   | `"hello"` `'raw'`  | UTF-8. Default type.           |
| int   | `42` `-1`          | i64                            |
| float | `3.14`             | f64                            |
| bool  | `true` `false`     |                                |
| list  | `["a", "b"]`       | Heterogeneous                  |
| map   | `{key: "val"}`     | String keys, ordered (IndexMap)|

### Variables

```portascript
let name = "value"        # immutable
let mut count = 0         # mutable
count = count + 1         # reassignment (mutable only)
```

Block-scoped. All variables must be initialized at declaration.

### Strings

Double-quoted strings support `{expr}` interpolation and escape sequences:

```portascript
let name = "world"
print("hello {name}\n")    # hello world
print("tab\there")         # tab    here
```

Escape sequences: `\n` `\t` `\\` `\{` `\"`.

Single-quoted strings are raw (no interpolation, no escapes):

```portascript
print('hello {name}')      # hello {name}
```

Triple-quoted for multiline (leading whitespace stripped by closing indent):

```portascript
let msg = """
    line one
    line two
    """
```

`'''` for raw multiline (no interpolation).

### Commands

`run` invokes uutils builtins (cross-platform, in-process via subprocess):

```portascript
run echo "hello world"
run sort data.txt
run mkdir -p /tmp/work
run cp source.txt dest.txt
```

`exec` spawns external (system) commands:

```portascript
exec git status
exec cargo build --release
```

Arguments support interpolation and list spread:

```portascript
let name = "world"
run echo "hello {name}"       # interpolated string arg
run echo {name}               # expression arg (coerced to string)

let flags = ["--verbose", "--color"]
exec cargo build {flags...}   # list spread into individual args
```

Command modifier bracket `[...]` for per-command env vars and stdin:

```portascript
exec [RUST_LOG="debug"] cargo build
exec [stdin=data] podman build -f -
```

### Pipelines

Pipelines connect stdout to stdin:

```portascript
run echo "banana\napple" | run sort
exec git log --oneline | run head -n 5
```

Any stage failing fails the whole pipeline (always-on pipefail).

### Capture

`$()` captures stdout as a trimmed string:

```portascript
let branch = $(exec git branch --show-current)
let count = $(run cat data.txt | run wc -l)
```

`try` captures everything as a result map (never aborts):

```portascript
let r = try exec git push
if r.ok {
    print("pushed")
} else {
    eprintln("failed ({r.code}): {r.stderr}")
}
```

Result map fields: `.ok` (bool), `.code` (int), `.stdout` (str), `.stderr` (str).

### Error Handling

Commands fail the script on nonzero exit by default (`set -e` equivalent, always on).

`?` suppresses failure:

```portascript
run rm tempfile.txt ?          # don't care if it doesn't exist
```

`try` captures failure without aborting (see [Capture](#capture)).

### Operators

**Arithmetic:** `+` `-` `*` `/` `%` (int/float, standard precedence)

**String:** `+` concatenation

**Comparison:** `==` `!=` `<` `>` `<=` `>=`

**Logical:** `and` `or` `not` (keywords, not symbols)

**Coalesce:** `??` (returns left if non-empty string, else right)

```portascript
let val = env.FOO ?? "default"
```

### Control Flow

```portascript
# if / elif / else
if count > 0 {
    run echo "positive"
} elif count == 0 {
    run echo "zero"
} else {
    run echo "negative"
}

# for / in
for f in ["a.txt", "b.txt"] {
    run cat {f}
}
for i in range(1, 10) {
    run echo {i}
}
for line in lines($(run cat data.txt)) {
    run echo "line: {line}"
}

# while
let mut i = 0
while i < 10 {
    i = i + 1
}

# match
match ext {
    "rs" => run echo "rust"
    "js" | "ts" => run echo "javascript"
    _ => run echo "unknown"
}

# break / continue work in for and while
```

Braces required. No parentheses around conditions. No fallthrough in match.

### Functions

```portascript
fn greet(name: str) {
    run echo "hello {name}"
}

fn add(a: int, b: int) -> int {
    return a + b
}

greet("world")
let sum = add(3, 4)
```

Declared before use (one-pass constraint). Parameters are typed.
Functions can access and modify outer `mut` variables.

### Builtin Functions

**Type conversion:**
`int(val)` `float(val)` `str(val)` `typeof(val)`

**String:**
`len(s)` `trim(s)` `upper(s)` `lower(s)` `split(s, delim)` `join(list, delim)`
`lines(s)` `contains(s, sub)` `starts_with(s, prefix)` `ends_with(s, suffix)`
`replace(s, old, new)`

**List:**
`len(list)` `append(list, val)` `range(start, end)` `range(end)`
Index: `list[i]` Slice: `list[1..]` `list[1..3]`

**Map:**
`len(map)` `keys(map)` `has_key(map, key)`
Access: `map["key"]` `map.field` Mutate: `map["key"] = val`

**Path:**
`path.join(parts...)` `path.abs(p)` `path.parent(p)` `path.ext(p)` `path.stem(p)`
`path.exists(p)` `path.is_file(p)` `path.is_dir(p)` `path.is_socket(p)`

**Filesystem:**
`read(path)` `write(path, content)` `append_file(path, content)`
`tempfile()`

**Process:**
`pid()` `command_exists(name)`

**I/O:**
`print(val)` `eprintln(val)` `eprint(val)`

**Control:**
`exit()` `exit(code)` `error(msg)`

### Environment Variables

```portascript
let home = env.HOME
let missing = env.FOO ?? "default"
```

### Script Arguments

```portascript
let script_path = args[0]
let first_arg = args[1]
let rest = args[1..]
```

## Bash Comparison

| Bash | Portascript | Notes |
|------|-------------|-------|
| `echo "hello"` | `run echo "hello"` | `run` keyword required |
| `git status` | `exec git status` | `exec` for external commands |
| `VAR="val"` | `let var = "val"` | `let` required, block-scoped |
| `VAR=val cmd` | `exec [VAR="val"] cmd` | Scoped env via modifier bracket |
| `$VAR` / `${VAR}` | `{var}` | Interpolation in double-quoted strings |
| `$(cmd)` | `$(exec cmd)` / `$(run cmd)` | `run`/`exec` required inside |
| `"${arr[@]}"` | `{list...}` | List spread in command args |
| `set -e` | default | Always on, nonzero aborts |
| `set -o pipefail` | default | Always on |
| `cmd \|\| true` | `cmd ?` | `?` suppresses failure |
| `if cmd; then` | `let r = try cmd; if r.ok` | `try` captures result map |
| `$?` | `r.code` | Via `try` result map |
| `[[ -f file ]]` | `path.is_file(file)` | Builtin function |
| `[[ -d dir ]]` | `path.is_dir(dir)` | Builtin function |
| `[[ -n "$VAR" ]]` | `var != ""` | Direct comparison |
| `${VAR:-default}` | `env.VAR ?? "default"` | `??` coalesce operator |
| `local var=val` | `let var = val` | Block-scoped by default |
| `readonly VAR=val` | `let var = val` | Immutable by default |
| `declare -A map` | `let mut m = {}` | Map literal |
| `for ((i=0; i<n; i++))` | `for i in range(0, n)` | Range-based |
| `case $x in ... esac` | `match x { ... }` | No fallthrough |
| `function f() { ... }` | `fn f() { ... }` | Typed parameters |
| `2>/dev/null` | `try cmd` (captures stderr) | No general redirection |
| `cmd1 && cmd2` | Sequential statements | Default fail-on-error |
| `cmd1 \|\| cmd2` | `cmd1 ?; if ... ` | Use `try` + conditional |
| `trap ... EXIT` | Not supported | Use `tempfile()` for auto-cleanup |
| `source file.sh` | Not yet implemented | Planned as `use` |

## Bash Conversion Guide

### Variables and quoting

```bash
# Bash
NAME="world"
echo "hello $NAME"
echo 'literal $NAME'
```

```portascript
# Portascript
let name = "world"
run echo "hello {name}"
run echo 'literal $NAME'
```

No word splitting, no glob expansion on variables. Quoting is simpler --
double quotes interpolate `{expr}`, single quotes are raw. No need for
`"$VAR"` defensive quoting.

### Command substitution

```bash
BRANCH=$(git branch --show-current)
COUNT=$(cat data | wc -l)
```

```portascript
let branch = $(exec git branch --show-current)
let count = $(run cat data | run wc -l)
```

### Conditionals on command success

```bash
if git push 2>/dev/null; then
    echo "pushed"
else
    echo "failed"
fi
```

```portascript
let r = try exec git push
if r.ok {
    run echo "pushed"
} else {
    run echo "failed"
}
```

### Array building and spreading

```bash
ARGS=()
[[ -n "$VERBOSE" ]] && ARGS+=("--verbose")
ARGS+=("--output" "$DIR")
cmd "${ARGS[@]}"
```

```portascript
let mut cmd_args = []
if env.VERBOSE != "" {
    cmd_args = append(cmd_args, "--verbose")
}
cmd_args = append(cmd_args, "--output")
cmd_args = append(cmd_args, dir)
exec cmd {cmd_args...}
```

### Parsing KEY=VALUE files

```bash
declare -A config
while IFS= read -r line; do
    [[ -z "$line" || "$line" == \#* ]] && continue
    key="${line%%=*}"
    value="${line#*=}"
    config["$key"]="$value"
done < config.env
```

```portascript
fn parse_env_file(filepath: str) -> map {
    let mut result = {}
    for line in lines(read(filepath)) {
        let line = trim(line)
        if line == "" or starts_with(line, "#") {
            continue
        }
        let parts = split(line, "=")
        let key = parts[0]
        let value = join(parts[1..], "=")
        result[key] = value
    }
    return result
}

let config = parse_env_file("config.env")
```

### Error suppression

```bash
rm -f tempfile 2>/dev/null || true
```

```portascript
run rm -f tempfile ?
```

### Heredocs / multiline strings

```bash
cat <<'EOF'
no interpolation
${literal}
EOF
```

```portascript
run echo '''
    no interpolation
    ${literal}
    '''
```

## Supported Builtins

24 uutils coreutils, invoked with `run`:

| Command | Description |
|---------|-------------|
| `echo` | Display text |
| `cat` | Concatenate and display files |
| `ls` | List directory contents |
| `cp` | Copy files |
| `mv` | Move/rename files |
| `rm` | Remove files |
| `mkdir` | Create directories |
| `touch` | Create files / update timestamps |
| `chmod` | Change file permissions |
| `head` | Display first lines |
| `tail` | Display last lines |
| `sort` | Sort lines |
| `uniq` | Filter duplicate adjacent lines |
| `wc` | Count lines/words/bytes |
| `tr` | Translate characters |
| `cut` | Extract columns |
| `tee` | Copy stdin to file and stdout |
| `seq` | Generate number sequences |
| `yes` | Repeat a string |
| `basename` | Strip directory from path |
| `dirname` | Strip filename from path |
| `printf` | Formatted output |
| `true` | Exit 0 |
| `false` | Exit 1 |

All other commands use `exec` (spawns from system PATH).
