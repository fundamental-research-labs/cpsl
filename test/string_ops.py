# String operations test

# Basic ops
s = "Hello, World!"
print(f"upper: {s.upper()}")
print(f"lower: {s.lower()}")
print(f"len: {len(s)}")
print(f"find 'World': {s.find('World')}")
print(f"find 'xyz': {s.find('xyz')}")
print(f"startswith 'Hello': {s.startswith('Hello')}")
print(f"endswith '!': {s.endswith('!')}")
print(f"replace: {s.replace('World', 'Luau')}")

# Split and join
csv = "apple,banana,cherry,date,elderberry"
parts = csv.split(",")
print(f"split: {parts}")
print(f"join: {' | '.join(parts)}")

# Strip
padded = "   hello   "
print(f"strip: '{padded.strip()}'")
print(f"lstrip: '{padded.lstrip()}'")
print(f"rstrip: '{padded.rstrip()}'")

# String multiplication and concatenation
line = "-" * 40
print(line)
greeting = "ha" * 5
print(f"ha*5: {greeting}")

# Indexing and slicing
word = "Python"
print(f"word[0]: {word[0]}")
print(f"word[-1]: {word[-1]}")
print(f"word[0:3]: {word[0:3]}")
print(f"word[3:]: {word[3:6]}")
print(f"word[::-1]: {word[::-1]}")

# String concatenation with +=
greeting = "hello"
greeting += " world"
print(f"greeting: {greeting}")

# Character iteration
vowels = ""
for ch in "transpiler":
    if ch in "aeiou":
        vowels += ch
print(f"vowels in 'transpiler': {vowels}")

# Count
text = "abracadabra"
print(f"count 'a': {text.count('a')}")
print(f"count 'abra': {text.count('abra')}")

# Membership
print(f"'World' in s: {'World' in s}")
print(f"'xyz' in s: {'xyz' in s}")
