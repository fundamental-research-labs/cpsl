import random

# Deterministic seed for reproducible output
random.seed(42)

# randint
print(random.randint(1, 10))
print(random.randint(1, 10))
print(random.randint(1, 10))

# choice from list
print(random.choice([10, 20, 30, 40, 50]))

# uniform
u = random.uniform(1.0, 2.0)
print(u >= 1.0 and u <= 2.0)

# randrange
print(random.randrange(10))
print(random.randrange(5, 10))

# shuffle (deterministic with seed)
random.seed(42)
lst = [1, 2, 3, 4, 5]
random.shuffle(lst)
print(lst)
