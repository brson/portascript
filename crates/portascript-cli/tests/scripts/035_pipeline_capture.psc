let result = $(run echo "hello world" | exec sh -c "cat")
print(result)
