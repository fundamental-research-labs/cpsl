# String Methods — comprehensive coverage
# upper / lower
print("hello".upper())
print("HELLO".lower())
print("Hello World".upper())
print("Hello World".lower())

# split — default (whitespace)
print("hello world foo".split())
print("  hello  world  ".split())

# split — with separator
print("a,b,c".split(","))
print("one::two::three".split("::"))

# join
print(" ".join(["hello", "world"]))
print(",".join(["a", "b", "c"]))
print("--".join(["x", "y", "z"]))

# strip / lstrip / rstrip (wrap in brackets to show whitespace)
print("[" + "  hello  ".strip() + "]")
print("[" + "  hello  ".lstrip() + "]")
print("[" + "  hello  ".rstrip() + "]")
print("xxhelloxx".strip("x"))

# replace
print("hello world".replace("world", "python"))
print("aabbcc".replace("b", "x"))
print("aaaa".replace("a", "b", 2))

# find
print("hello world".find("world"))
print("hello world".find("xyz"))
print("hello".find("l"))

# count
print("hello".count("l"))
print("banana".count("an"))
print("aaa".count("a"))

# startswith / endswith
print("hello".startswith("hel"))
print("hello".startswith("xyz"))
print("hello".endswith("llo"))
print("hello".endswith("xyz"))

# Chained methods
s = "  Hello World  "
print(s.strip().lower())
print(s.strip().upper())
print(s.strip().replace("World", "Python").lower())
