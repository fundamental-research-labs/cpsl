# Variables and arithmetic
x = 10
y = 3
print(f"x + y = {x + y}")
print(f"x - y = {x - y}")
print(f"x * y = {x * y}")
print(f"x / y = {x / y}")
print(f"x // y = {x // y}")
print(f"x % y = {x % y}")
print(f"x ** y = {x ** y}")

# Strings
name = "World"
greeting = f"Hello, {name}!"
print(greeting)
print(greeting.upper())
print(greeting.lower())
print("a,b,c".split(","))
print("-".join(["x", "y", "z"]))
print(greeting.startswith("Hello"))
print(len(greeting))

# Lists
items = [10, 20, 30, 40, 50]
items.append(60)
print(items)
print(items[0])
print(items[-1])
print(len(items))
items.pop()
print(items)

# Dicts
person = {"name": "Alice", "age": 30}
print(person["name"])
print(person.get("missing", "default"))
person["email"] = "alice@example.com"
for key in person:
    print(f"  {key}: {person[key]}")

# Control flow
for i in range(5):
    if i % 2 == 0:
        print(f"{i} is even")
    else:
        print(f"{i} is odd")

# Functions
def factorial(n):
    if n <= 1:
        return 1
    return n * factorial(n - 1)

print(f"5! = {factorial(5)}")

# Default arguments
def greet(name, greeting="hello"):
    return f"{greeting}, {name}"

print(greet("Bob"))
print(greet("Bob", "hi"))

# List comprehension
squares = [x**2 for x in range(10)]
print(squares)

evens = [x for x in range(20) if x % 2 == 0]
print(evens)

# Builtins
nums = [3, 1, 4, 1, 5, 9]
print(sorted(nums))
print(min(nums))
print(max(nums))
print(sum(nums))

# Boolean logic
print(True and False)
print(True or False)
print(not True)

# Membership
print(3 in [1, 2, 3])
print(4 in [1, 2, 3])
print("a" in "abc")

# Try/except
try:
    result = items[100]
except:
    print("Caught an error!")

print("Done!")
