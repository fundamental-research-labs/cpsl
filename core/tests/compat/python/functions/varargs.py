# *args variadic functions
def f(*args):
    print(args)

f(1, 2, 3)
f()
f("hello")

def g(a, b, *rest):
    print(a, b, rest)

g(1, 2, 3, 4, 5)

def total(*nums):
    return sum(nums)

print(total(1, 2, 3))
print(total(10, 20))

def count_args(*args):
    print(len(args))

count_args(1, 2, 3, 4)
count_args()
