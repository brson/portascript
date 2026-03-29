let r = try exec sh -c "echo hello"
if r.ok {
    print(r.stdout)
}
