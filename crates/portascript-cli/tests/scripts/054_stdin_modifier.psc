let data = "hello from stdin"
let result = $(exec [stdin=data] sh -c "cat")
print(result)
