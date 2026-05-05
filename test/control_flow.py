# Control flow: if/elif/else, while, for, break, continue, try/except

# Nested if/elif/else
def classify(n):
    if n < 0:
        return "negative"
    elif n == 0:
        return "zero"
    elif n < 10:
        return "small"
    elif n < 100:
        return "medium"
    else:
        return "large"

for val in [-5, 0, 3, 42, 100]:
    print(f"classify({val}) = {classify(val)}")

# While with break
def find_first_prime_above(n):
    candidate = n + 1
    while True:
        is_p = True
        if candidate < 2:
            is_p = False
        else:
            i = 2
            while i * i <= candidate:
                if candidate % i == 0:
                    is_p = False
                    break
                i += 1
        if is_p:
            return candidate
        candidate += 1

print(f"first prime above 100: {find_first_prime_above(100)}")
print(f"first prime above 1000: {find_first_prime_above(1000)}")

# Continue
odd_sum = 0
for i in range(100):
    if i % 2 == 0:
        continue
    odd_sum += i
print(f"sum of odd numbers 0..99: {odd_sum}")

# Try/except (use index error — Luau doesn't error on 1/0)
try:
    items = [1, 2, 3]
    v = items[99]
except:
    print("caught index error")

try:
    d = {"a": 1}
    v = d["missing"]
except:
    print("caught key error")

# Nested loops
print("multiplication table (1-5):")
for i in range(1, 6):
    row = []
    for j in range(1, 6):
        row.append(i * j)
    print(f"  {row}")

# FizzBuzz
result = []
for i in range(1, 31):
    if i % 15 == 0:
        result.append("FizzBuzz")
    elif i % 3 == 0:
        result.append("Fizz")
    elif i % 5 == 0:
        result.append("Buzz")
    else:
        result.append(str(i))
print(f"FizzBuzz: {result}")
