# Heavy computation: primes, GCD, and number theory

def is_prime(n):
    if n < 2:
        return False
    if n < 4:
        return True
    if n % 2 == 0 or n % 3 == 0:
        return False
    i = 5
    while i * i <= n:
        if n % i == 0 or n % (i + 2) == 0:
            return False
        i += 6
    return True

def sieve(limit):
    # Sieve of Eratosthenes — returns list of primes up to limit
    flags = [True for _ in range(limit + 1)]
    flags[0] = False
    flags[1] = False
    p = 2
    while p * p <= limit:
        if flags[p]:
            j = p * p
            while j <= limit:
                flags[j] = False
                j += p
        p += 1
    result = []
    for i in range(limit + 1):
        if flags[i]:
            result.append(i)
    return result

def gcd(a, b):
    while b != 0:
        a, b = b, a % b
    return a

def lcm(a, b):
    return abs(a * b) // gcd(a, b)

# Primes up to 200
primes = sieve(200)
print(f"primes up to 200 ({len(primes)} total): {primes}")

# Verify with trial division
for p in primes:
    assert is_prime(p), f"{p} is not prime!"
print("all sieve results verified by trial division")

# GCD / LCM
print(f"gcd(48, 18) = {gcd(48, 18)}")
print(f"gcd(100, 75) = {gcd(100, 75)}")
print(f"lcm(12, 18) = {lcm(12, 18)}")
print(f"lcm(7, 13) = {lcm(7, 13)}")

# Sum of primes
total = sum(primes)
print(f"sum of primes up to 200 = {total}")

# Goldbach check for even numbers 4..100
for n in range(4, 101, 2):
    found = False
    for p in primes:
        if p >= n:
            break
        if is_prime(n - p):
            found = True
            break
    assert found, f"Goldbach failed for {n}"
print("Goldbach conjecture holds for all even numbers 4..100")

# Collatz sequence length
def collatz_len(n):
    steps = 0
    while n != 1:
        if n % 2 == 0:
            n = n // 2
        else:
            n = 3 * n + 1
        steps += 1
    return steps

longest = 0
longest_n = 1
for n in range(1, 1001):
    c = collatz_len(n)
    if c > longest:
        longest = c
        longest_n = n

print(f"longest Collatz sequence under 1000: n={longest_n}, steps={longest}")
