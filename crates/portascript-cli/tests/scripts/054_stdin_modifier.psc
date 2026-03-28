let data = "hello from stdin"
let result = $(exec [stdin=data] cat)
print(result)
