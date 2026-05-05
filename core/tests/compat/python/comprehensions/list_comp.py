# List Comprehensions — comprehensive coverage
# Simple
squares = [x * x for x in range(6)]
print(squares)

# With condition
evens = [x for x in range(10) if x % 2 == 0]
print(evens)

# With expression
doubled = [x * 2 for x in [1, 2, 3, 4]]
print(doubled)

# With function call
lengths = [len(s) for s in ["hello", "hi", "world"]]
print(lengths)

# Nested list comprehension (flatten)
flat = [x for sublist in [[1, 2], [3, 4], [5]] for x in sublist]
print(flat)

# String processing
uppers = [s.upper() for s in ["hello", "world"]]
print(uppers)

# With complex condition
big_evens = [x for x in range(20) if x % 2 == 0 if x > 5]
print(big_evens)
