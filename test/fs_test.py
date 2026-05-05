content = fs.read("/data/hello.txt")
print(f"File says: {content}")

entries = fs.list("/data")
for name in entries:
    print(name)
