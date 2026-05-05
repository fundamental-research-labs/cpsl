# Variables & Assignment — comprehensive coverage
# Integer assignment
x = 1
y = 42
z = -7
print(x, y, z)

# Float assignment
a = 3.14
b = -0.5
c = 0.0
print(a, b, c)

# String assignment
s1 = "hello"
s2 = 'world'
print(s1, s2)

# Boolean assignment
t = True
f = False
print(t, f)

# None assignment
n = None
print(n)

# Multiple assignment on one line
p, q, r = 1, 2, 3
print(p, q, r)

# Swap via tuple unpacking
p, q = q, p
print(p, q)

# Chained reassignment
x = 100
x = x + 1
print(x)

# Augmented assignment — all numeric ops
val = 10
val += 5
print(val)
val -= 3
print(val)
val *= 2
print(val)
val /= 4
print(val)
val = 17
val //= 3
print(val)
val = 17
val %= 5
print(val)
val = 2
val **= 10
print(val)

# Mixed int/float
m = 5 + 2.0
print(m)
m = 10 / 3
print(m)

# Negative numbers and precedence
print(-3 + 1)
print(-(3 + 1))
print(2 + 3 * 4)
print((2 + 3) * 4)
print(2 ** 3 ** 2)
print(10 - 2 - 3)
print(100 // 7)
print(-17 % 5)
