# Dicts — comprehensive coverage
# Creation and print (insertion order preserved)
d = {"a": 1, "b": 2, "c": 3}
print(d)

# Empty dict
e = {}
print(e)

# Indexing
print(d["a"])
print(d["c"])

# Key assignment
d["d"] = 4
print(d["d"])

# Deletion
del d["d"]
print("d" in d)

# keys, values, items
print(list(d.keys()))
print(list(d.values()))
print(list(d.items()))

# get (with/without default)
print(d.get("a"))
print(d.get("z"))
print(d.get("z", 99))

# update
d.update({"b": 20, "e": 5})
print(d["b"])
print(d["e"])

# len
print(len(d))
print(len({}))

# in (checks keys)
print("a" in d)
print("z" in d)
print("a" not in d)

# Iteration (for k in d preserves insertion order)
result = []
for k in d:
    result.append(k)
print(result)

# Nested dict
nested = {"x": {"a": 1}, "y": {"b": 2}}
print(nested)

# Dict with various value types
mixed = {"int": 1, "str": "hello", "bool": True, "none": None, "list": [1, 2]}
print(mixed["int"])
print(mixed["str"])
print(mixed["bool"])
print(mixed["none"])
print(mixed["list"])
