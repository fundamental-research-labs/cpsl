# Loops — comprehensive coverage
# for over range (1 arg)
for i in range(5):
    print(i)

# for over range (2 args)
for i in range(2, 6):
    print(i)

# for over range (3 args — step)
for i in range(0, 10, 3):
    print(i)

# for over list
for x in [10, 20, 30]:
    print(x)

# for over string
for c in "abc":
    print(c)

# while with break
i = 0
while True:
    if i >= 5:
        break
    print(i)
    i += 1

# while with continue
i = 0
while i < 10:
    i += 1
    if i % 2 == 0:
        continue
    print(i)

# Nested loops with break
for i in range(3):
    for j in range(3):
        if j == 2:
            break
        print(i, j)

# enumerate
for i, v in enumerate(["a", "b", "c"]):
    print(i, v)

# zip
for a, b in zip([1, 2, 3], ["x", "y", "z"]):
    print(a, b)

# reversed
for x in reversed([1, 2, 3]):
    print(x)

# Accumulator pattern
total = 0
for x in [10, 20, 30]:
    total += x
print(total)
