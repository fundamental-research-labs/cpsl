# Test: comprehensive pattern coverage

# --- Ternary expression ---
x = 5
print("pos" if x > 0 else "neg")
print("zero" if x == 0 else "nonzero")

# --- del on dict keys and variables ---
d = {"a": 1, "b": 2, "c": 3}
del d["b"]
print(len(d))
print(d)
y = 42
del y

# --- raise + try/except ---
try:
    raise "test error"
except:
    print("caught error")

# try/finally
result = []
try:
    result.append(1)
finally:
    result.append(2)
print(result)

# --- Multiple return values ---
def swap(a, b):
    return b, a

p, q = swap(10, 20)
print(p, q)

# --- Chained comparisons ---
val = 5
print(1 < val < 10)
print(1 < val < 3)
print(0 <= val <= 5)

# --- is / is not with None ---
a = None
b = 42
print(a is None)
print(b is not None)
print(b is None)

# --- Unary operators ---
n = 7
print(-n)
print(not True)
print(not False)

# --- Bitwise NOT ---
print(~0)
print(~5)
print(~(-1))

# --- Bitwise operators ---
print(5 & 3)
print(5 | 3)
print(5 ^ 3)
print(1 << 4)
print(16 >> 2)

# --- reversed() ---
nums = [10, 20, 30]
for x in reversed(nums):
    print(x)

rev_list = list(reversed([1, 2, 3]))
print(rev_list)

# --- Generator expression in sum ---
total = sum(i * i for i in range(5))
print(total)

# --- Type conversions ---
print(int(3.9))
print(int("42"))
print(str(123))
print(bool(0))
print(bool(1))
print(bool(""))
print(bool("hi"))

# --- Multiple assignment targets ---
x = y = 5
print(x, y)

# --- pass statement ---
def empty_func():
    pass

empty_func()
print("pass works")

# --- String escapes ---
print("hello\tworld")
print("line1\nline2")

# --- Nested functions / closures ---
def outer(x):
    def inner(y):
        return x + y
    return inner(10)

print(outer(5))

# --- for/else ---
for i in range(5):
    if i == 10:
        break
else:
    print("for/else: no break")

for i in range(5):
    if i == 3:
        break
else:
    print("should not print")

# --- while/else ---
count = 3
while count > 0:
    count -= 1
else:
    print("while/else: completed")

# --- Nested tuple unpacking ---
a, (b, c) = 1, (2, 3)
print(a, b, c)

# --- List comprehension with condition ---
evens = [x for x in range(10) if x % 2 == 0]
print(evens)

# --- Dict comprehension ---
squares = {x: x * x for x in range(5)}
print(squares)

# --- Lambda ---
double = lambda x: x * 2
print(double(7))

# --- assert (should not error) ---
assert True
assert 1 + 1 == 2, "math is broken"
print("assertions passed")

# --- Augmented assignments with type tracking ---
counter = 0
counter += 5
counter *= 2
counter -= 1
counter //= 3
print(counter)

# --- Bitwise augmented assignments ---
flags = 0xFF
flags &= 0x0F
print(flags)
flags |= 0x30
print(flags)
flags ^= 0x05
print(flags)
