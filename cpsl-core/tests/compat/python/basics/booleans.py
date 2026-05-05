# Booleans & Comparisons — comprehensive coverage
# Basic boolean values
print(True)
print(False)

# and / or / not
print(True and True)
print(True and False)
print(False or True)
print(False or False)
print(not True)
print(not False)

# Truthiness: falsy values
print(bool(0))
print(bool(""))
print(bool([]))
print(bool(None))
print(bool(False))

# Truthiness: truthy values
print(bool(1))
print(bool("hello"))
print(bool([1, 2]))
print(bool(True))
print(bool(-1))

# Comparison operators
print(1 == 1)
print(1 == 2)
print(1 != 2)
print(1 != 1)
print(3 < 5)
print(5 < 3)
print(3 > 1)
print(1 > 3)
print(3 <= 3)
print(3 <= 5)
print(5 >= 5)
print(3 >= 5)

# Chained comparisons
x = 5
print(1 < x < 10)
print(1 < x < 3)
print(0 < 1 < 2 < 3)

# in operator
print("ell" in "hello")
print("xyz" in "hello")
print(2 in [1, 2, 3])
print(5 in [1, 2, 3])
print("a" in {"a": 1, "b": 2})
print("c" in {"a": 1, "b": 2})

# not in
print("xyz" not in "hello")
print(5 not in [1, 2, 3])

# None comparisons
print(None == None)
print(None != None)
print(1 == None)
print(None == 1)

# Boolean in if
if True:
    print("true branch")
if not False:
    print("not false")
if 0:
    print("should not print")
else:
    print("zero is falsy")

# Short-circuit evaluation
x = 5
result = x > 0 and "positive"
print(result)
result = x < 0 and "negative"
print(result)
result = x < 0 or "fallback"
print(result)
