# Lists — comprehensive coverage
# Creation
a = [1, 2, 3]
print(a)
b = []
print(b)
c = [1]
print(c)

# Indexing (positive and negative)
print(a[0])
print(a[1])
print(a[2])
print(a[-1])
print(a[-2])

# Slicing
nums = [0, 1, 2, 3, 4, 5]
print(nums[1:4])
print(nums[:3])
print(nums[3:])
print(nums[:])
print(nums[::2])
print(nums[1::2])
print(nums[::-1])
print(nums[4:1:-1])

# append
a.append(4)
print(a)

# extend
a.extend([5, 6])
print(a)

# pop
last = a.pop()
print(last, a)
second = a.pop(1)
print(second, a)

# insert
a.insert(0, 99)
print(a)
a.insert(2, 88)
print(a)

# sort and reverse
nums2 = [3, 1, 4, 1, 5, 9, 2]
nums2.sort()
print(nums2)
nums2.reverse()
print(nums2)

# sort with reverse
nums3 = [3, 1, 4, 1, 5]
nums3.sort(reverse=True)
print(nums3)

# len
print(len([1, 2, 3]))
print(len([]))

# in operator
print(2 in [1, 2, 3])
print(5 in [1, 2, 3])

# Concatenation
print([1, 2] + [3, 4])

# Repetition
print([0] * 3)
print([1, 2] * 2)

# Nested lists
nested = [[1, 2], [3, 4], [5]]
print(nested)
print(nested[0])
print(nested[1][1])

# sorted (returns new list)
orig = [3, 1, 2]
s = sorted(orig)
print(s)
print(orig)

# sorted with reverse
print(sorted([5, 3, 1, 4, 2], reverse=True))
