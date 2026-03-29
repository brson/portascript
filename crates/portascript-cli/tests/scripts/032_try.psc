let r = try exec sh -c "exit 1"
print(r.ok)
print(r.code)
