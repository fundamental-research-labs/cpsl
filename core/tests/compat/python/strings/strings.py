# Strings — comprehensive coverage
# String creation
s1 = "hello"
s2 = 'world'
print(s1, s2)

# Concatenation
print("hello" + " " + "world")

# Repetition
print("ha" * 3)
print("ab" * 0)

# Length
print(len("hello"))
print(len(""))

# Indexing (positive and negative)
s = "abcdef"
print(s[0])
print(s[2])
print(s[-1])
print(s[-3])

# Slicing
print(s[1:4])
print(s[:3])
print(s[3:])
print(s[:])
print(s[::2])
print(s[1::2])
print(s[::-1])

# f-strings
name = "world"
x = 42
print(f"hello {name}")
print(f"x = {x}")
print(f"sum = {2 + 3}")
print(f"len = {len(name)}")

# Escape sequences
print("hello\tworld")
print("line1\nline2")
print("back\\slash")
print("say \"hi\"")

# String in boolean context
if "hello":
    print("non-empty is truthy")
if not "":
    print("empty is falsy")

# String equality
print("abc" == "abc")
print("abc" == "def")
print("abc" != "def")

# String containment
print("ell" in "hello")
print("xyz" in "hello")
print("xyz" not in "hello")
