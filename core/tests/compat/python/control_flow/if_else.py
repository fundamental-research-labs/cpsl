# If/Elif/Else — comprehensive coverage
# Simple if
x = 10
if x > 5:
    print("big")

# if/else
if x > 20:
    print("huge")
else:
    print("not huge")

# if/elif/else chain
y = 15
if y < 0:
    print("negative")
elif y == 0:
    print("zero")
elif y < 10:
    print("small")
elif y < 20:
    print("medium")
else:
    print("large")

# Nested if
a = 5
b = 3
if a > 0:
    if b > 0:
        print("both positive")
    else:
        print("a positive, b not")
else:
    print("a not positive")

# Ternary expression
val = "even" if 10 % 2 == 0 else "odd"
print(val)
val = "even" if 7 % 2 == 0 else "odd"
print(val)

# Truthiness-based conditions
if [1, 2]:
    print("non-empty list is truthy")
if not []:
    print("empty list is falsy")
if "hello":
    print("non-empty string is truthy")
if not "":
    print("empty string is falsy")
if 42:
    print("non-zero is truthy")
if not 0:
    print("zero is falsy")
if not None:
    print("None is falsy")
