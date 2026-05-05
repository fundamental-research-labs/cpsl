# Tuples — comprehensive coverage
# Creation
t = (1, 2, 3)
print(t)

# Single element (trailing comma required)
s = (1,)
print(s)

# Empty tuple
e = ()
print(e)

# Indexing
print(t[0])
print(t[1])
print(t[-1])

# Slicing
print(t[1:])
print(t[:2])

# Unpacking
a, b, c = t
print(a, b, c)

# Nested unpacking
x, y = (10, 20)
print(x, y)

# len
print(len(t))
print(len(()))

# in operator
print(2 in t)
print(5 in t)

# Nested tuples
nested = ((1, 2), (3, 4))
print(nested)
print(nested[0])
print(nested[1][1])

# Tuple in print (repr format)
print((1, "hello", True, None))

# Tuple with mixed types
mixed = (1, "two", 3.14, True, None)
print(mixed)

# Tuple concatenation
print((1, 2) + (3, 4))

# Tuple repetition
print((0,) * 3)
