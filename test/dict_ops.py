# Dictionary operations test
# Note: Lua table iteration order != Python insertion order,
# so we avoid relying on key order in printed output.

# Construction and access
d = {"name": "Alice", "age": 30, "city": "NYC"}
print(f"d['name'] = {d['name']}")
print(f"d['age'] = {d['age']}")
print(f"d.get('age') = {d.get('age', 0)}")
print(f"d.get('missing', -1) = {d.get('missing', -1)}")

# Length
print(f"len(d) = {len(d)}")

# Membership
print(f"'name' in d: {'name' in d}")
print(f"'phone' in d: {'phone' in d}")

# Modification
d["email"] = "alice@example.com"
print(f"d['email'] = {d['email']}")
print(f"len after add = {len(d)}")

# Dict comprehension
squares = {x: x ** 2 for x in range(5)}
print(f"squares[0] = {squares[0]}")
print(f"squares[4] = {squares[4]}")

# Update
d2 = {"city": "SF", "phone": "555-1234"}
d.update(d2)
print(f"d['city'] after update = {d['city']}")
print(f"d['phone'] after update = {d['phone']}")

# Nested dict
nested = {"a": {"x": 1, "y": 2}, "b": {"x": 3, "y": 4}}
print(f"nested['a']['x'] = {nested['a']['x']}")
print(f"nested['b']['y'] = {nested['b']['y']}")

# Keys/values/items (test via len, not printing order)
keys = d.keys()
vals = d.values()
items = d.items()
print(f"len(keys) = {len(keys)}")
print(f"len(values) = {len(vals)}")
print(f"len(items) = {len(items)}")
