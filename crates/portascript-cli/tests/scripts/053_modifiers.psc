let result = $(exec [MY_VAR="hello"] sh -c "echo $MY_VAR")
print(result)
