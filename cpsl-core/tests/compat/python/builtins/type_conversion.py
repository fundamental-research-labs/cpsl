# Type conversion and inspection
# str() on all types
print(str(42))
print(str(3.14))
print(str(True))
print(str(False))
print(str(None))
print(str([1, 2, 3]))
print(str({"a": 1}))
print(str((1, 2)))

# int() from string/float
print(int("123"))
print(int(3.99))
print(int(-3.99))

# float() from string/int
print(float("3.14"))
print(float(42))
print(float(0))

# type() output
print(type(42))
print(type(3.14))
print(type("hello"))
print(type(True))
print(type(None))
print(type([1, 2]))
print(type({}))
print(type((1,)))

# isinstance() checks
print(isinstance(42, int))
print(isinstance(3.14, float))
print(isinstance("hi", str))
print(isinstance([1], list))
print(isinstance({}, dict))
print(isinstance((1,), tuple))
print(isinstance(42, str))
print(isinstance("hi", int))

# bool() conversions
print(bool(0))
print(bool(1))
print(bool(-1))
print(bool(""))
print(bool("hello"))
print(bool([]))
print(bool([0]))
print(bool(None))
print(bool({}))
print(bool({"a": 1}))
