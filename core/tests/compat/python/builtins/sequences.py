# Sequence built-ins
# len() on various types
print(len("hello"))
print(len(""))
print(len([1, 2, 3]))
print(len([]))
print(len({"a": 1, "b": 2}))
print(len({}))
print(len((1, 2, 3, 4)))

# sorted() basic
print(sorted([3, 1, 4, 1, 5, 9]))
print(sorted([]))
print(sorted(["banana", "apple", "cherry"]))

# sorted() with key
print(sorted(["banana", "apple", "cherry", "date"], key=len))

# sorted() with reverse
print(sorted([3, 1, 4, 1, 5], reverse=True))

# sorted() with key and reverse
print(sorted(["bb", "a", "ccc"], key=len, reverse=True))

# reversed() into list
print(list(reversed([1, 2, 3, 4])))

# range() all forms
print(list(range(5)))
print(list(range(2, 7)))
print(list(range(0, 10, 2)))
print(list(range(10, 0, -2)))
print(list(range(5, 5)))

# enumerate() basic
for i, v in enumerate(["a", "b", "c"]):
    print(i, v)

# enumerate() with start
for i, v in enumerate(["x", "y"], 10):
    print(i, v)

# zip() equal length
for a, b in zip([1, 2, 3], ["a", "b", "c"]):
    print(a, b)

# zip() unequal length (truncates to shortest)
for a, b in zip([1, 2], ["a", "b", "c"]):
    print(a, b)
