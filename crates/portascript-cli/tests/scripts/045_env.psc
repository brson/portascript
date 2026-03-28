let home = env.HOME
print(typeof(home))
let missing = env.PORTASCRIPT_NONEXISTENT_VAR ?? "default"
print(missing)
