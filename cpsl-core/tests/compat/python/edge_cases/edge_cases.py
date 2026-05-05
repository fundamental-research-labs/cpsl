# Edge cases and regressions
# Empty collections
print(len([]))
print(len({}))
print(len(()))
print(len(""))
print(list(range(0)))
print(sorted([]))

# None comparisons
print(None == None)
print(None != None)
print(None == 0)
print(None == "")
print(None == False)
print(None == [])

# Deeply nested structures
nested = [[1, [2, 3]], [4, [5, 6]]]
print(nested[0][1][0])
print(nested[1][1][1])

nested_dict = {"a": {"b": {"c": 42}}}
print(nested_dict["a"]["b"]["c"])

# Large loop
total = 0
for i in range(1000):
    total += 1
print(total)

# String with special characters
print("hello\tworld")
print("line1\nline2")
print("back\\slash")
print("quote\"inside")

# Negative indexing edge cases
lst = [10, 20, 30, 40, 50]
print(lst[-1])
print(lst[-5])
s = "abcde"
print(s[-1])
print(s[-5])

# Slice with step=-1 (full reverse)
print([1, 2, 3, 4, 5][::-1])
print("abcde"[::-1])

# Zero division in try
try:
    x = 10 // 0
except:
    print("caught floor division by zero")

try:
    y = 10 % 0
except:
    print("caught modulo by zero")

# Mixed int/float arithmetic
print(1 + 2.0)
print(3 * 1.5)
print(10 - 0.5)

# Chained comparisons
x = 5
print(1 < x < 10)
print(1 < x < 3)
print(1 <= 1 < 2)
