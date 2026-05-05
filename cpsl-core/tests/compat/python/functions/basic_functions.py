# Functions — comprehensive coverage
# Basic function
def add(a, b):
    return a + b

print(add(3, 4))
print(add(10, -2))

# String function
def greet(name):
    return "hello " + name

print(greet("world"))

# Default args
def power(base, exp=2):
    return base ** exp

print(power(3))
print(power(2, 10))

# Multiple return values (tuple unpacking)
def divmod_fn(a, b):
    return a // b, a % b

q, r = divmod_fn(17, 5)
print(q, r)

# Recursive function
def factorial(n):
    if n <= 1:
        return 1
    return n * factorial(n - 1)

print(factorial(5))
print(factorial(0))

# Function as argument (sorted with key)
words = ["banana", "apple", "cherry", "date"]
result = sorted(words, key=len)
print(result)

# Lambda
double = lambda x: x * 2
print(double(5))
print(double(0))

# Lambda in sorted
nums = [-3, 1, -5, 2, 4]
print(sorted(nums, key=lambda x: abs(x)))

# Nested function
def outer(x):
    def inner(y):
        return x + y
    return inner(10)

print(outer(5))

# Function returning None implicitly
def no_return(x):
    y = x + 1

result = no_return(5)
print(result)
