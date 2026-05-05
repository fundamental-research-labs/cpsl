# Numeric built-ins
# int()
print(int(3.7))
print(int(3.0))
print(int(-3.7))
print(int("42"))
print(int("-17"))
print(int(True))
print(int(False))

# float()
print(float(3))
print(float("3.14"))
print(float("-2.5"))
print(float(0))

# abs()
print(abs(5))
print(abs(-5))
print(abs(0))
print(abs(-3.14))
print(abs(3.14))

# min/max with args
print(min(3, 1, 4, 1, 5))
print(max(3, 1, 4, 1, 5))
print(min(1, 2))
print(max(1, 2))

# min/max with lists
print(min([3, 1, 4, 1, 5]))
print(max([3, 1, 4, 1, 5]))

# sum
print(sum([1, 2, 3, 4, 5]))
print(sum([]))
print(sum([10, -3, 7]))
print(sum([1, 2, 3], 10))

# pow via **
print(2 ** 10)
print(3 ** 0)
print((-2) ** 3)
print(2 ** -1)
