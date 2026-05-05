# Test: import patterns and passthrough modules

import math
print(math.floor(3.7))
print(math.ceil(3.2))
print(int(math.sqrt(16)))

from math import floor, ceil
print(floor(7.9))
print(ceil(2.1))

import math as m
print(m.floor(9.8))
print(m.ceil(0.1))

from math import floor as f
print(f(5.5))

# Multiple math operations
print(math.floor(math.sqrt(50)))
print(m.ceil(m.sqrt(10)))
