# Portascript Design

A single-binary, cross-platform scripting language in Rust.
Bash-like command-oriented semantics without the footguns.
Built on uutils for portable coreutils builtins.

## Principles

- Every line is a statement. No expression-oriented ambiguity.
- String-oriented pipelines, like bash. Not an object shell.
- Builtins (uutils) and external commands are syntactically distinct.
- Failure is loud. Commands fail the script unless explicitly handled.
- One-pass execution. No AST. Fast startup, low memory.
- Cross-platform by default. Same script runs on Linux, macOS, Windows.

## Types

Six types. No user-defined types.

| Type   | Literal              | Notes                           |
|--------|----------------------|---------------------------------|
| str    | `"hello"` `'hello'`  | UTF-8. Default type.            |
| int    | `42` `-1`            | i64                             |
| float  | `3.14`               | f64                             |
| bool   | `true` `false`       |                                 |
| list   | `["a", "b", "c"]`    | Heterogeneous                   |
| map    | `{key: "val"}`       | String keys, any values         |

No null. No option type. Empty string is the zero value for unset variables.

Automatic coercion: values coerce to string when used in command arguments
or interpolation. `int` and `float` coerce to each other in arithmetic.
`str` to `int`/`float` coercion is explicit via `int()` and `float()` builtins.
Bool coerces to `"true"`/`"false"` as string, `1`/`0` as int.

## Lexical Structure

### Comments

```portascript
# single line comment
```

No block comments.

### Strings

Double-quoted strings support interpolation. Single-quoted strings are raw.

```portascript
let name = "world"
echo "hello {name}"       # hello world
echo 'hello {name}'       # hello {name}
```

Interpolation uses `{}`. Braces are rare enough in shell arguments
that this won't collide often. Literal `{` in double-quoted strings: `\{`.

Multiline strings:

```portascript
let msg = """
    line one
    line two
    """
```

Leading whitespace is stripped based on closing `"""` indentation (like Rust raw strings / Swift).

Escape sequences in double-quoted strings: `\n` `\t` `\\` `\{` `\"`.

### Identifiers

`[a-zA-Z_][a-zA-Z0-9_]*`

Variable references in command arguments use `{var}` (interpolation).
Bare words in command position resolve as builtin commands, not variables.

### Line continuation

```portascript
cp "source" \
   "dest"
```

Backslash-newline joins lines. Lines ending in `|` also continue.

### Semicolons

Semicolons separate statements on one line.

```portascript
let x = 1; let y = 2
```

## Variables

```portascript
let name = "value"           # immutable
let mut count = 0            # mutable

count = count + 1            # reassignment (mutable only)
```

Variables are block-scoped. Inner blocks shadow outer.
All variables must be initialized at declaration.

### Environment variables

Read:

```portascript
let home = env.HOME            # read env var
let missing = env.FOO ?? ""    # with default (?? is "or if empty")
```

Write:

```portascript
env.MY_VAR = "value"           # set for this process + children
```

Scoped env for a single command:

```portascript
exec [RUST_LOG="debug"] cargo build
```

## Commands

### Builtins (uutils)

Bare command names invoke uutils builtins. These run in-process.

```portascript
echo "hello"
ls -la /tmp
cp source.txt dest.txt
sort data.txt
cat file1.txt file2.txt
head -n 10 log.txt
mkdir -p /tmp/work
rm -rf /tmp/work
mv old.txt new.txt
chmod 755 script.sh
wc -l file.txt
basename /foo/bar.txt
dirname /foo/bar.txt
touch newfile.txt
tr "a-z" "A-Z"
cut -d: -f1
tee output.log
uniq
seq 1 10
yes
true
false
```

The full set of builtins matches what uutils provides.
Arguments follow standard unix conventions (flags, operands).
Arguments are strings -- variables interpolate via `{}`.

### External commands

The `exec` keyword invokes external (system) commands. This makes it
explicit when you're leaving the portable sandbox.

```portascript
exec git status
exec cargo build --release
exec python3 script.py
```

`exec` spawns a child process on all platforms via Rust's `std::process::Command`.
No `fork()` dependency. Works uniformly on Windows.

### Why the distinction matters

- **Readability.** You can instantly see which commands are portable (builtins)
  and which depend on the system (exec).
- **Performance.** Builtins run in-process. No spawn overhead.
- **Portability auditing.** `grep -c "^exec " script.psc` tells you
  how platform-dependent a script is.

## Pipelines

Pipelines connect stdout of one command to stdin of the next.

```portascript
cat data.txt | sort | uniq | wc -l
```

Builtin-to-builtin pipes are in-process (no OS pipe).
When an `exec` command participates, OS pipes connect the processes.

```portascript
exec git log --oneline | head -n 5
cat urls.txt | exec xargs curl -s | sort
```

Pipeline failure: if any stage fails, the entire pipeline fails
(bash `pipefail` semantics, but always on).

### Capture

Capture command output into a variable with `$()`:

```portascript
let files = $(ls /src)              # stdout as string, trimmed
let count = $(cat data | wc -l)     # pipelines work inside $()
```

`$()` captures stdout as a trimmed string. Stderr passes through to the script's stderr.

## Operators

### Arithmetic

`+` `-` `*` `/` `%` on int/float. Standard precedence.

```portascript
let x = 10 + 3 * 2    # 16
let y = 10 / 3         # 3 (int division)
let z = 10.0 / 3.0     # 3.333...
```

### Comparison

`==` `!=` `<` `>` `<=` `>=`

String comparison is lexicographic. Numeric comparison on int/float.
Comparing different types is a runtime error (no implicit coercion in comparisons).

### Logical

`and` `or` `not` -- keywords, not symbols. Avoids `&&`/`||` confusion with
pipeline/command chaining.

```portascript
if x > 0 and x < 100 {
    echo "in range"
}
```

### String

`+` concatenates strings.

```portascript
let full = first + " " + last
```

### Coalesce

`??` returns left side if non-empty string, otherwise right side.

```portascript
let val = env.FOO ?? "default"
```

## Control Flow

### if / elif / else

```portascript
if count > 0 {
    echo "positive"
} elif count == 0 {
    echo "zero"
} else {
    echo "negative"
}
```

Braces required. No parentheses around condition.

### for

Iterate over lists, glob results, or lines:

```portascript
# over a list
for f in ["a.txt", "b.txt", "c.txt"] {
    cat {f}
}

# over glob results
for f in glob("src/**/*.rs") {
    wc -l {f}
}

# over lines of a string
for line in lines($(cat data.txt)) {
    echo "line: {line}"
}

# over a range
for i in range(1, 10) {
    echo {i}
}
```

### while

```portascript
let mut i = 0
while i < 10 {
    echo {i}
    i = i + 1
}
```

### break / continue

Work in `for` and `while`. No labels.

### match

```portascript
match ext {
    "rs" => echo "rust"
    "py" => echo "python"
    "js" | "ts" => echo "javascript"
    _ => echo "unknown"
}
```

No fallthrough.

## Error Handling

Commands (both builtins and exec) that return nonzero exit codes
cause the script to abort with an error message. This is the default --
no `set -e` required.

### try

Capture failure without aborting:

```portascript
let result = try exec git push
if result.ok {
    echo "pushed"
} else {
    echo "push failed with code {result.code}"
    echo "stderr: {result.stderr}"
}
```

`try` wraps a command and returns a result map with fields:
- `.ok` -- bool, true if exit code 0
- `.code` -- int, exit code
- `.stdout` -- str, captured stdout
- `.stderr` -- str, captured stderr

### The ? operator

Suffix `?` on a command to suppress failure (ignore nonzero exit).
The command runs, errors are silently discarded, execution continues.

```portascript
rm tempfile.txt ?    # don't care if it doesn't exist
```

This is intentionally the opposite of Rust's `?` -- in a shell context,
`?` meaning "don't care about this error" is more natural.

## Functions

```portascript
fn greet(name: str) {
    echo "hello {name}"
}

fn add(a: int, b: int) -> int {
    return a + b
}

# default arguments
fn deploy(env: str, verbose: bool = false) {
    if verbose {
        echo "deploying to {env}"
    }
    exec rsync -az ./build/ "{env}:/app/"
}
```

Functions are declared before use (one-pass constraint).
Parameters are typed. Return type is optional -- if omitted, the function
returns nothing (it's a procedure).

Functions can capture command output:

```portascript
fn file_count(dir: str) -> int {
    return int($(ls {dir} | wc -l))
}
```

No closures. No first-class functions. This is a scripting language.

## Builtin Functions

These are portascript-native functions, distinct from uutils builtins.

### Type conversion

- `int(val)` -- parse string to int, or truncate float
- `float(val)` -- parse string to float, or promote int
- `str(val)` -- convert anything to string
- `bool(val)` -- `""`, `"false"`, `0` -> false; everything else -> true

### String functions

- `len(s)` -- string length (or list length)
- `split(s, delim)` -- split string into list
- `join(list, delim)` -- join list into string
- `trim(s)` -- strip leading/trailing whitespace
- `starts_with(s, prefix)` -- bool
- `ends_with(s, suffix)` -- bool
- `contains(s, substr)` -- bool
- `replace(s, old, new)` -- string replacement
- `upper(s)` / `lower(s)` -- case conversion
- `lines(s)` -- split on newlines into list

### Path functions

- `path.join(parts...)` -- OS-aware path join
- `path.exists(p)` -- bool
- `path.is_file(p)` / `path.is_dir(p)` -- bool
- `path.ext(p)` -- file extension
- `path.stem(p)` -- filename without extension
- `path.parent(p)` -- parent directory
- `path.abs(p)` -- absolute path

### List functions

- `len(list)` -- length
- `append(list, val)` -- returns new list with val appended
- `list[i]` -- index access (0-based)
- `list[i..j]` -- slice

### Filesystem

- `glob(pattern)` -- returns list of matching paths
- `read(path)` -- read file contents as string
- `write(path, content)` -- write string to file

### Other

- `range(start, end)` / `range(end)` -- integer range for loops
- `typeof(val)` -- returns type name as string
- `error(msg)` -- abort script with message
- `print(val)` -- print without newline (echo adds newline)

## Process Model

All process spawning goes through Rust's `std::process::Command`.
No `fork()`. No `posix_spawn()` directly. This gives uniform behavior
across Linux, macOS, and Windows.

### Signals

Portascript takes a simple approach: signals indicate hard failure.

- **SIGINT / Ctrl-C**: kill all child processes, exit immediately with code 130.
- **SIGTERM**: same behavior, exit with 143.
- **SIGPIPE**: ignored (write errors surface as command failures).
- **On Windows**: Ctrl-C and process termination are handled via
  `SetConsoleCtrlHandler`. Same behavior: kill children, exit.

No signal trapping. No custom signal handlers. If you need that,
you need a different tool.

### Child process management

When portascript exits (normally or via signal), it kills all spawned
child processes. No orphans.

Portascript tracks all spawned PIDs (or HANDLEs on Windows) and sends
kill signals on exit. This uses Rust's `Child` type which abstracts
the platform differences.

## Pipelines -- Implementation

### Builtin-to-builtin

When all stages are uutils builtins, portascript uses in-process
threading. Each stage runs in a thread, connected by bounded
`crossbeam` channels or `std::sync::mpsc`. Data flows as byte chunks.

```
[cat thread] --channel--> [sort thread] --channel--> [wc thread]
```

No OS pipes. No process spawning. Fast.

### Mixed pipelines

When `exec` commands participate, portascript uses OS pipes to connect
to the child process stdio. Builtins in the same pipeline still run
as threads, with adapters between channels and OS pipes.

```
[cat thread] --os-pipe--> [exec grep process] --os-pipe--> [wc thread]
```

### Exec-to-exec

Standard OS pipe between two child processes.

## One-Pass Execution Model

### How it works

The interpreter processes the source file as a stream of tokens.
No separate parse phase that builds a full AST.

1. **Tokenizer** reads characters, emits tokens. Operates on demand.
2. **Executor** consumes tokens, executes immediately.

For straight-line code (commands, assignments, pipelines), tokens are
consumed and executed with zero buffering.

For block constructs (`if`, `for`, `while`, `fn`, `match`), the
executor buffers the token stream for the block body. This is a flat
`Vec<Token>`, not a tree. The buffered tokens are replayed during
execution (for loops replay multiple times).

Function bodies are stored as buffered token sequences and replayed
on each call.

### Why one-pass

- **Fast startup.** No parse phase. First command executes almost instantly.
- **Low memory.** Only block bodies are buffered. Straight-line scripts
  use O(1) memory regardless of length.
- **Simple implementation.** ~2000 lines for the core interpreter,
  estimated.

### Tradeoff

No ahead-of-time error checking. A syntax error on line 500 isn't
discovered until execution reaches line 500. This is acceptable --
bash works the same way, and for scripting this is fine. A `--check`
flag can do a dry-run parse without execution for linting.

## Token Types

```
Keyword:    let mut if elif else for in while break continue
            fn return match try exec and or not true false env
Ident:      [a-zA-Z_][a-zA-Z0-9_]*
Int:        [0-9]+
Float:      [0-9]+\.[0-9]+
String:     "..." or '...' or """..."""
Operator:   + - * / % == != < > <= >= = ??
Symbol:     { } [ ] ( ) | ; , . .. => ? \n
Bare:       anything else in command position (flags like -la, paths)
Comment:    # to end of line (discarded)
```

The key insight: after a command-position keyword (a builtin name,
`exec`, or start of line), subsequent tokens are parsed in **command mode**
where bare words and flags are valid. After `let`, `if`, `while`, `return`,
tokens are parsed in **expression mode** where operators and precedence apply.

## Grammar (Informal)

```
program     = stmt*
stmt        = let_stmt | assign_stmt | if_stmt | for_stmt | while_stmt
            | fn_stmt | match_stmt | return_stmt | break_stmt
            | continue_stmt | pipeline | cmd_stmt

let_stmt    = "let" "mut"? IDENT "=" expr
assign_stmt = IDENT "=" expr
if_stmt     = "if" expr block ("elif" expr block)* ("else" block)?
for_stmt    = "for" IDENT "in" expr block
while_stmt  = "while" expr block
fn_stmt     = "fn" IDENT "(" params? ")" ("->" type)? block
match_stmt  = "match" expr "{" match_arm* "}"
match_arm   = pattern ("," pattern)* "=>" (stmt | block)
return_stmt = "return" expr?
break_stmt  = "break"
cont_stmt   = "continue"

pipeline    = cmd_stmt ("|" cmd_stmt)+
cmd_stmt    = builtin_cmd | exec_cmd | try_expr
builtin_cmd = BUILTIN_NAME arg* "?"?
exec_cmd    = "exec" ("[" env_pairs "]")? arg* "?"?
try_expr    = "try" cmd_stmt

arg         = STRING | BARE | "{" expr "}" | flag
expr        = ... (standard precedence climbing)
block       = "{" stmt* "}"

type        = "str" | "int" | "float" | "bool" | "list" | "map"
```

## Crate Structure

```
portascript/
  Cargo.toml                  # workspace root
  crates/
    portascript/              # the binary crate
      src/
        main.rs               # entry point, arg parsing, file loading
    ps-core/                  # types, values, scopes
      src/
        lib.rs
        value.rs              # Value enum (Str, Int, Float, Bool, List, Map)
        scope.rs              # Scope stack
        error.rs              # PsError type
    ps-interp/                # tokenizer + one-pass executor
      src/
        lib.rs
        token.rs              # Token enum, tokenizer
        exec.rs               # Executor (the main loop)
        pipeline.rs           # Pipeline construction and execution
        builtins.rs           # Dispatch table for uutils builtins
        functions.rs          # User function storage and invocation
    ps-builtins/              # uutils integration layer
      src/
        lib.rs                # register() -> HashMap<&str, BuiltinFn>
        adapter.rs            # Adapter: uutils entry point -> portascript I/O
```

### Dependency graph

```
portascript (bin)
  -> ps-interp
       -> ps-core
       -> ps-builtins
            -> ps-core
            -> uu_echo, uu_ls, uu_cp, uu_sort, ... (uutils crates)
```

## uutils Integration Layer

Each uutils crate exposes a `uumain(args: impl Iterator<Item=OsString>) -> i32`
entry point. The adapter layer:

1. Accepts args as `Vec<String>` from the portascript executor.
2. Sets up I/O redirection: replaces stdin/stdout/stderr with portascript's
   pipeline channels using thread-local overrides or by invoking the
   uutils crate's stream-accepting API where available.
3. Calls the uutils entry point.
4. Translates the exit code to a portascript result.

For builtins where uutils doesn't expose a stream-accepting API,
the adapter uses OS pipes to capture output, with the builtin running
in a dedicated thread.

```rust
// Simplified adapter sketch
pub type BuiltinFn = fn(args: Vec<String>, stdin: PsStream, stdout: PsStream) -> PsResult;

pub fn run_builtin(
    name: &str,
    args: Vec<String>,
    stdin: PsStream,
    stdout: PsStream,
) -> PsResult {
    let builtin = BUILTINS.get(name).ok_or(PsError::UnknownBuiltin(name))?;
    builtin(args, stdin, stdout)
}
```

## Value Representation

```rust
#[derive(Clone, Debug)]
pub enum Value {
    Str(String),
    Int(i64),
    Float(f64),
    Bool(bool),
    List(Vec<Value>),
    Map(IndexMap<String, Value>),  // preserves insertion order
}

impl Value {
    pub fn to_str(&self) -> String { /* coerce anything to string */ }
    pub fn is_truthy(&self) -> bool { /* "" and 0 and false are falsy */ }
}
```

## Scope

```rust
pub struct Scope {
    frames: Vec<HashMap<String, (Value, bool)>>,  // (value, is_mutable)
}

impl Scope {
    pub fn push(&mut self) { self.frames.push(HashMap::new()); }
    pub fn pop(&mut self) { self.frames.pop(); }
    pub fn get(&self, name: &str) -> Option<&Value> { /* walk frames */ }
    pub fn set(&mut self, name: &str, val: Value) -> Result<()> { /* check mutability */ }
    pub fn declare(&mut self, name: &str, val: Value, mutable: bool) { /* insert in top frame */ }
}
```

## Example Scripts

### Build and deploy

```portascript
let project = "myapp"
let version = trim($(cat VERSION))

echo "building {project} v{version}"
exec cargo build --release

let binary = "target/release/{project}"
if not path.exists(binary) {
    error("build failed: {binary} not found")
}

let size = $(wc -c < {binary} | tr -d " ")
echo "binary size: {size} bytes"

let servers = ["web1.prod", "web2.prod"]
for server in servers {
    echo "deploying to {server}..."
    exec scp {binary} "{server}:/opt/{project}/bin/"
    exec ssh {server} "systemctl restart {project}"
}

echo "deployed v{version} to {len(servers)} servers"
```

### Log analysis

```portascript
fn analyze_log(logfile: str) {
    echo "=== {logfile} ==="
    let total = int($(wc -l < {logfile}))
    let errors = int($(cat {logfile} | grep -c "ERROR" ?))
    let warnings = int($(cat {logfile} | grep -c "WARN" ?))

    echo "  total lines: {total}"
    echo "  errors:      {errors}"
    echo "  warnings:    {warnings}"

    if errors > 0 {
        echo "  last 5 errors:"
        cat {logfile} | grep "ERROR" | tail -n 5
    }
}

for log in glob("/var/log/app/*.log") {
    analyze_log(log)
}
```

### File processing

```portascript
let src = env.1 ?? "."
let dest = env.2 ?? "./backup"

mkdir -p {dest}

let mut copied = 0
for f in glob("{src}/**/*.txt") {
    let rel = replace(f, src, "")
    let target = path.join(dest, rel)
    mkdir -p $(dirname {target})
    cp {f} {target}
    copied = copied + 1
}

echo "copied {copied} files to {dest}"
```

## CLI Interface

```
portascript script.psc [args...]
portascript --check script.psc     # parse check without execution
portascript --version
portascript --help
```

File extension: `.psc`

Script arguments available as `args` (a builtin list variable):

```portascript
let filename = args[1]   # first user argument (args[0] is script path)
```

## What This Is Not

- Not a shell. No interactive REPL. No job control. No prompt.
- Not a general-purpose language. No structs, traits, generics, async.
- Not POSIX-compatible. Won't run bash scripts. Intentional.
- Not a package manager. No imports from the internet. `use` loads local files only.

## Implementation Priority

1. **Tokenizer + executor core.** Let, assignment, if, for, while, functions.
   String interpolation. Scopes. Basic expressions.
2. **Command execution.** Builtin dispatch via uutils. `exec` for externals.
   Capture with `$()`.
3. **Pipelines.** In-process pipes for builtins. OS pipes for exec.
   Mixed pipelines.
4. **Error handling.** Default fail-on-error. `try` and `?` operator.
5. **Builtin functions.** String, path, list, type conversion functions.
6. **`--check` mode.** Dry-run parse for linting.
7. **Polish.** Error messages with line numbers. Edge cases. Windows testing.
