# Test fs read/write from Python
fs.write("/data/output.txt", "written from python!")
content = fs.read("/data/output.txt")
print(f"Read back: {content}")

# Test fs.exists and fs.mkdir
print(f"exists /data/output.txt: {fs.exists('/data/output.txt')}")
print(f"exists /data/nope.txt: {fs.exists('/data/nope.txt')}")

fs.mkdir("/data/subdir")
fs.write("/data/subdir/nested.txt", "nested content")
print(f"nested: {fs.read('/data/subdir/nested.txt')}")

# List
entries = fs.list("/data")
print("entries in /data:")
for e in entries:
    print(f"  {e}")
