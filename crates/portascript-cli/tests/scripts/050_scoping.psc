let x = "outer"
if true {
    let x = "inner"
    print(x)
}
print(x)
