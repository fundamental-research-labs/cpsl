# Fibonacci computation — recursive and iterative

def fib_recursive(n):
    if n <= 1:
        return n
    return fib_recursive(n - 1) + fib_recursive(n - 2)

def fib_iterative(n):
    a = 0
    b = 1
    for i in range(n):
        a, b = b, a + b
    return a

# Verify both give same results
for i in range(15):
    r = fib_recursive(i)
    it = fib_iterative(i)
    assert r == it, f"mismatch at {i}: {r} != {it}"
    print(f"fib({i}) = {r}")

# Compute larger value iteratively
print(f"fib(30) = {fib_iterative(30)}")
print(f"fib(50) = {fib_iterative(50)}")
