# Dict Comprehensions — comprehensive coverage
# Simple dict comprehension
squares = {x: x * x for x in range(5)}
for k in sorted(squares.keys()):
    print(k, squares[k])

# With condition
even_squares = {x: x * x for x in range(10) if x % 2 == 0}
for k in sorted(even_squares.keys()):
    print(k, even_squares[k])

# From list
names = ["alice", "bob", "carol"]
name_lens = {n: len(n) for n in names}
for k in sorted(name_lens.keys()):
    print(k, name_lens[k])
