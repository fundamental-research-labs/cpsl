# Sorting algorithms implemented in pure Python

def bubble_sort(arr):
    n = len(arr)
    for i in range(n):
        for j in range(0, n - i - 1):
            if arr[j] > arr[j + 1]:
                arr[j], arr[j + 1] = arr[j + 1], arr[j]
    return arr

def insertion_sort(arr):
    for i in range(1, len(arr)):
        key = arr[i]
        j = i - 1
        while j >= 0 and arr[j] > key:
            arr[j + 1] = arr[j]
            j -= 1
        arr[j + 1] = key
    return arr

def selection_sort(arr):
    n = len(arr)
    for i in range(n):
        min_idx = i
        for j in range(i + 1, n):
            if arr[j] < arr[min_idx]:
                min_idx = j
        arr[i], arr[min_idx] = arr[min_idx], arr[i]
    return arr

# Test data
data = [64, 34, 25, 12, 22, 11, 90, 1, 55, 43, 77, 8, 99, 3, 67]

# Copy helper
def copy_list(lst):
    return [x for x in lst]

# Bubble sort
r1 = bubble_sort(copy_list(data))
print(f"bubble:    {r1}")

# Insertion sort
r2 = insertion_sort(copy_list(data))
print(f"insertion: {r2}")

# Selection sort
r3 = selection_sort(copy_list(data))
print(f"selection: {r3}")

# Verify all match
expected = sorted(data)
assert r1 == expected, f"bubble sort failed: {r1}"
assert r2 == expected, f"insertion sort failed: {r2}"
assert r3 == expected, f"selection sort failed: {r3}"
print(f"expected:  {expected}")
print("All sorting algorithms match!")
