let r = try exec echo "hello"
if r.ok {
    print(r.stdout)
}
