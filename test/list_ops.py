# List operations test

# Construction
nums = [10, 20, 30, 40, 50]
print(f"nums = {nums}")
print(f"len = {len(nums)}")

# Indexing
print(f"nums[0] = {nums[0]}")
print(f"nums[-1] = {nums[-1]}")
print(f"nums[2] = {nums[2]}")

# Slicing
print(f"nums[1:3] = {nums[1:3]}")
print(f"nums[:2] = {nums[0:2]}")
print(f"nums[3:] = {nums[3:5]}")
print(f"nums[::-1] = {nums[::-1]}")

# Append, extend, insert
nums.append(60)
print(f"after append(60): {nums}")
nums.extend([70, 80])
print(f"after extend([70,80]): {nums}")
nums.insert(0, 5)
print(f"after insert(0,5): {nums}")

# Pop
popped = nums.pop()
print(f"popped last: {popped}, nums = {nums}")
popped = nums.pop(0)
print(f"popped first: {popped}, nums = {nums}")

# Sort and reverse
data = [5, 3, 8, 1, 9, 2, 7, 4, 6]
data.sort()
print(f"sorted: {data}")
data.reverse()
print(f"reversed: {data}")

# List comprehension
evens = [x for x in range(20) if x % 2 == 0]
print(f"evens: {evens}")

squares = [x ** 2 for x in range(10)]
print(f"squares: {squares}")

# Nested comprehension
matrix = [[1, 2, 3], [4, 5, 6], [7, 8, 9]]
flat = [x for row in matrix for x in row]
print(f"flat: {flat}")

# Membership
print(f"5 in data: {5 in data}")
print(f"99 in data: {99 in data}")

# Concatenation and repetition
a = [1, 2, 3]
b = [4, 5, 6]
print(f"a + b = {a + b}")
print(f"a * 3 = {a * 3}")

# Enumerate and zip
names = ["alice", "bob", "charlie"]
scores = [85, 92, 78]
for i, name in enumerate(names):
    print(f"  {i}: {name}")
for name, score in zip(names, scores):
    print(f"  {name}: {score}")
