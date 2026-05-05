# Functional patterns: lambdas, map-like comprehensions, closures

# Lambda
double = lambda x: x * 2
print(f"double(5) = {double(5)}")

# Higher-order functions
def apply(fn, lst):
    return [fn(x) for x in lst]

def compose(f, g):
    return lambda x: f(g(x))

nums = [1, 2, 3, 4, 5]
doubled = apply(lambda x: x * 2, nums)
squared = apply(lambda x: x ** 2, nums)
print(f"doubled: {doubled}")
print(f"squared: {squared}")

add1_then_double = compose(lambda x: x * 2, lambda x: x + 1)
result = apply(add1_then_double, nums)
print(f"(x+1)*2: {result}")

# Closure / accumulator
def make_counter(start):
    count = [start]
    def increment():
        count[0] = count[0] + 1
        return count[0]
    return increment

c = make_counter(0)
for i in range(5):
    print(f"counter: {c()}")

# Reduce-like
def reduce(fn, lst, initial):
    acc = initial
    for x in lst:
        acc = fn(acc, x)
    return acc

total = reduce(lambda a, b: a + b, [1, 2, 3, 4, 5], 0)
print(f"reduce sum: {total}")

product = reduce(lambda a, b: a * b, [1, 2, 3, 4, 5], 1)
print(f"reduce product: {product}")

# Filter-like
def my_filter(fn, lst):
    return [x for x in lst if fn(x)]

data = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10]
evens = my_filter(lambda x: x % 2 == 0, data)
print(f"evens: {evens}")

big = my_filter(lambda x: x > 5, data)
print(f"big (>5): {big}")

# Zip + dict creation
keys = ["a", "b", "c", "d"]
vals = [1, 2, 3, 4]
d = {}
for k, v in zip(keys, vals):
    d[k] = v
print(f"zipped dict: {d}")
