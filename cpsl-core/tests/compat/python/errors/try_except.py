# Error Handling — comprehensive coverage
# Basic try/except
try:
    x = 1 / 0
except:
    print("caught division by zero")

# try/except with specific type
try:
    y = int("abc")
except ValueError:
    print("caught ValueError")

# try/except/else
try:
    z = int("42")
except ValueError:
    print("should not reach here")
else:
    print("conversion succeeded:", z)

# try/except/finally
try:
    a = 10
except:
    print("error")
finally:
    print("finally always runs")

# try/except with finally on error
try:
    b = 1 / 0
except:
    print("caught error in try/finally")
finally:
    print("finally after error")

# raise with message
try:
    raise ValueError("bad value")
except ValueError:
    print("caught raised ValueError")

# Nested try/except
try:
    try:
        x = 1 / 0
    except:
        print("inner caught")
        raise ValueError("re-raised")
except ValueError:
    print("outer caught ValueError")
