-- pyrt.luau — Python semantics runtime for transpiled code
-- Every py.list, py.dict, py.tuple wraps a table with a type marker.

local py = {}

-- ============================================================
-- Sentinel for None (Lua nil loses dict keys)
-- ============================================================
py.None = setmetatable({}, { __tostring = function() return "None" end })

-- ============================================================
-- Type markers
-- ============================================================
local LIST_MT = {}
local DICT_MT = {}
local TUPLE_MT = {}

-- ============================================================
-- Float marker — tags a number as "Python float" for display.
-- Not used in arithmetic/comparison — just carries format info
-- through to py.str/py.repr.
-- ============================================================
local FLOAT_MT = {}
FLOAT_MT.__index = FLOAT_MT

function py.F(n)
    if type(n) == "table" and n.__py_float then return n end
    return setmetatable({ __py_float = true, v = n }, FLOAT_MT)
end

local function is_py_float(val)
    return type(val) == "table" and val.__py_float == true
end

local function unwrap_float(val)
    if is_py_float(val) then return val.v end
    return val
end

-- ============================================================
-- Construction
-- ============================================================

function py.list(t)
    local obj = { __py_type = "list", data = {}, length = 0 }
    if t then
        if type(t) == "function" then
            -- Drain iterator function into list
            local val = t()
            while val ~= nil do
                obj.length = obj.length + 1
                obj.data[obj.length] = val
                val = t()
            end
        elseif type(t) == "table" and t.__py_type then
            -- Copy from py.list/tuple/dict
            if t.__py_type == "list" or t.__py_type == "tuple" then
                for i = 1, t.length do
                    obj.data[i] = t.data[i]
                    obj.length = i
                end
            elseif t.__py_type == "dict" then
                for i = 1, #t.keys_order do
                    obj.data[i] = t.keys_order[i]
                    obj.length = i
                end
            end
        else
            for i = 1, #t do
                obj.data[i] = t[i]
                obj.length = i
            end
        end
    end
    return setmetatable(obj, LIST_MT)
end

-- Legacy py.dict(table) — order not guaranteed (used for empty dicts and internal)
function py.dict(t)
    local obj = { __py_type = "dict", data = {}, keys_order = {} }
    if t then
        for k, v in pairs(t) do
            obj.data[k] = v
            table.insert(obj.keys_order, k)
        end
    end
    return setmetatable(obj, DICT_MT)
end

-- Ordered dict construction from flat key-value array: py.dict_from({"a", 1, "b", 2})
-- Preserves insertion order (Python dict semantics).
function py.dict_from(kvs)
    local obj = { __py_type = "dict", data = {}, keys_order = {} }
    if kvs then
        local i = 1
        while i <= #kvs do
            local k = kvs[i]
            local v = kvs[i + 1]
            if v == nil then v = py.None end
            obj.data[k] = v
            table.insert(obj.keys_order, k)
            i = i + 2
        end
    end
    return setmetatable(obj, DICT_MT)
end

function py.tuple(t)
    local obj = { __py_type = "tuple", data = {}, length = 0 }
    if t then
        for i = 1, #t do
            obj.data[i] = t[i]
            obj.length = i
        end
    end
    return setmetatable(obj, TUPLE_MT)
end

-- ============================================================
-- Type checking helpers
-- ============================================================

local function is_py_type(obj, typename)
    return type(obj) == "table" and obj.__py_type == typename
end

function py.type(val)
    if val == nil or val == py.None then return "NoneType" end
    if type(val) == "boolean" then return "bool" end
    if type(val) == "number" then
        if val == math.floor(val) then return "int" end
        return "float"
    end
    if type(val) == "string" then return "str" end
    if is_py_type(val, "list") then return "list" end
    if is_py_type(val, "dict") then return "dict" end
    if is_py_type(val, "tuple") then return "tuple" end
    if type(val) == "function" then return "function" end
    return "object"
end

-- User-facing type() — returns <class 'typename'> format like Python
function py.type_display(val)
    return "<class '" .. py.type(val) .. "'>"
end

function py.isinstance(val, typename)
    -- Error type matching: "ValueError: msg" matches "ValueError"
    if type(val) == "string" and type(typename) == "string" then
        local colon_pos = string.find(val, ":", 1, true)
        if colon_pos then
            local error_type = string.sub(val, 1, colon_pos - 1)
            if error_type == typename then return true end
        end
    end
    return py.type(val) == typename
end

-- ============================================================
-- Truthiness (Python semantics)
-- ============================================================

function py.bool(val)
    if val == nil or val == false or val == py.None then return false end
    if val == 0 then return false end
    if val == "" then return false end
    if is_py_type(val, "list") and val.length == 0 then return false end
    if is_py_type(val, "dict") and #val.keys_order == 0 then return false end
    if is_py_type(val, "tuple") and val.length == 0 then return false end
    return true
end

-- ============================================================
-- Indexing (0-based Python → 1-based Lua)
-- ============================================================

function py.index(obj, key)
    if is_py_type(obj, "list") or is_py_type(obj, "tuple") then
        local idx = key
        if idx < 0 then idx = obj.length + idx end
        idx = idx + 1 -- 0-based → 1-based
        if idx < 1 or idx > obj.length then
            error("IndexError: list index out of range", 0)
        end
        return obj.data[idx]
    elseif is_py_type(obj, "dict") then
        local val = obj.data[key]
        if val == nil then
            error("KeyError: " .. tostring(key), 0)
        end
        if val == py.None then return py.None end
        return val
    elseif type(obj) == "string" then
        local idx = key
        if idx < 0 then idx = #obj + idx end
        idx = idx + 1
        if idx < 1 or idx > #obj then
            error("IndexError: string index out of range", 0)
        end
        return string.sub(obj, idx, idx)
    end
    error("TypeError: object is not subscriptable", 0)
end

function py.setindex(obj, key, val)
    if is_py_type(obj, "list") then
        local idx = key
        if idx < 0 then idx = obj.length + idx end
        idx = idx + 1
        if idx < 1 or idx > obj.length then
            error("IndexError: list assignment index out of range", 0)
        end
        obj.data[idx] = val
    elseif is_py_type(obj, "dict") then
        if val == nil then
            -- del dict[key]: remove from both data and keys_order
            obj.data[key] = nil
            for i = #obj.keys_order, 1, -1 do
                if obj.keys_order[i] == key then
                    table.remove(obj.keys_order, i)
                    break
                end
            end
        else
            if obj.data[key] == nil then
                table.insert(obj.keys_order, key)
            end
            obj.data[key] = val
        end
    else
        error("TypeError: object does not support item assignment", 0)
    end
end

function py.slice(obj, start, stop, step)
    step = step or 1
    local len
    if is_py_type(obj, "list") or is_py_type(obj, "tuple") then
        len = obj.length
    elseif type(obj) == "string" then
        len = #obj
    else
        error("TypeError: object is not subscriptable", 0)
    end

    -- Resolve None/nil defaults
    if start == nil then start = (step > 0) and 0 or (len - 1) end
    if stop == nil then stop = (step > 0) and len or (-len - 1) end

    -- Normalize negative indices
    if start < 0 then start = math.max(len + start, 0) end
    if stop < 0 then
        -- For negative step, stop=-1 means "include index 0" (before-the-beginning sentinel)
        local floor = (step < 0) and -1 or 0
        stop = math.max(len + stop, floor)
    end

    -- Clamp
    start = math.min(start, len)
    stop = math.min(stop, len)

    local result = {}
    local i = start
    while (step > 0 and i < stop) or (step < 0 and i > stop) do
        local idx = i + 1 -- 0→1 based
        if type(obj) == "string" then
            table.insert(result, string.sub(obj, idx, idx))
        else
            table.insert(result, obj.data[idx])
        end
        i = i + step
    end

    if type(obj) == "string" then
        return table.concat(result)
    end
    -- Preserve type: slicing a tuple returns a tuple, slicing a list returns a list
    if is_py_type(obj, "tuple") then
        return py.tuple(result)
    end
    return py.list(result)
end

-- ============================================================
-- Iteration
-- ============================================================

function py.iter(obj)
    -- Pass through functions (already iterators, e.g. from reversed/range)
    if type(obj) == "function" then return obj end
    if is_py_type(obj, "list") or is_py_type(obj, "tuple") then
        local i = 0
        return function()
            i = i + 1
            if i <= obj.length then return obj.data[i] end
            return nil
        end
    elseif is_py_type(obj, "dict") then
        local i = 0
        return function()
            i = i + 1
            if i <= #obj.keys_order then return obj.keys_order[i] end
            return nil
        end
    elseif type(obj) == "string" then
        local i = 0
        return function()
            i = i + 1
            if i <= #obj then return string.sub(obj, i, i) end
            return nil
        end
    elseif type(obj) == "table" then
        -- Plain Lua table (e.g. from fs.list) — iterate array part
        local i = 0
        return function()
            i = i + 1
            if obj[i] ~= nil then return obj[i] end
            return nil
        end
    end
    error("TypeError: object is not iterable", 0)
end

function py.range(a, b, step)
    local start, stop
    if b == nil then
        start, stop, step = 0, a, 1
    else
        start, stop = a, b
        step = step or 1
    end
    if step == 0 then error("ValueError: range() arg 3 must not be zero", 0) end
    local i = start - step
    return function()
        i = i + step
        if (step > 0 and i < stop) or (step < 0 and i > stop) then
            return i
        end
        return nil
    end
end

function py.enumerate(obj, start)
    start = start or 0
    local it = py.iter(obj)
    local i = start - 1
    return function()
        local val = it()
        if val == nil then return nil end
        i = i + 1
        return i, val
    end
end

function py.zip(...)
    local iters = {}
    for i = 1, select("#", ...) do
        local arg = select(i, ...)
        table.insert(iters, py.iter(arg))
    end
    return function()
        local vals = {}
        for _, it in ipairs(iters) do
            local v = it()
            if v == nil then return nil end
            table.insert(vals, v)
        end
        return table.unpack(vals)
    end
end

-- ============================================================
-- Operators
-- ============================================================

function py.add(a, b)
    -- Booleans → integers (Python: True=1, False=0)
    if type(a) == "boolean" then a = a and 1 or 0 end
    if type(b) == "boolean" then b = b and 1 or 0 end
    local ta = type(a)
    if ta == "number" then
        if type(b) == "number" then return a + b end
    elseif ta == "string" then
        if type(b) == "string" then return a .. b end
    elseif is_py_type(a, "list") and is_py_type(b, "list") then
        local result = py.list({})
        for i = 1, a.length do py.append(result, a.data[i]) end
        for i = 1, b.length do py.append(result, b.data[i]) end
        return result
    elseif is_py_type(a, "tuple") and is_py_type(b, "tuple") then
        local items = {}
        for i = 1, a.length do table.insert(items, a.data[i]) end
        for i = 1, b.length do table.insert(items, b.data[i]) end
        return py.tuple(items)
    end
    error("TypeError: unsupported operand type(s) for +: '" .. py.type(a) .. "' and '" .. py.type(b) .. "'", 0)
end

function py.mul(a, b)
    -- Booleans → integers (Python: True=1, False=0)
    if type(a) == "boolean" then a = a and 1 or 0 end
    if type(b) == "boolean" then b = b and 1 or 0 end
    local ta = type(a)
    if ta == "number" then
        if type(b) == "number" then return a * b end
        if type(b) == "string" then return string.rep(b, a) end
    elseif ta == "string" then
        if type(b) == "number" then return string.rep(a, b) end
    elseif is_py_type(a, "list") and type(b) == "number" then
        local result = py.list({})
        for _ = 1, b do
            for i = 1, a.length do py.append(result, a.data[i]) end
        end
        return result
    elseif is_py_type(a, "tuple") and type(b) == "number" then
        local items = {}
        for _ = 1, b do
            for i = 1, a.length do table.insert(items, a.data[i]) end
        end
        return py.tuple(items)
    end
    error("TypeError: unsupported operand type(s) for *", 0)
end

function py.sub(a, b)
    -- Booleans → integers (Python: True=1, False=0)
    if type(a) == "boolean" then a = a and 1 or 0 end
    if type(b) == "boolean" then b = b and 1 or 0 end
    return a - b
end

function py.eq(a, b)
    if a == b then return true end
    -- Fast path: primitives (number, string, bool, nil) can't be deep-equal
    if type(a) ~= "table" or type(b) ~= "table" then return false end
    -- Deep equality for py types
    if is_py_type(a, "list") and is_py_type(b, "list") then
        if a.length ~= b.length then return false end
        for i = 1, a.length do
            if not py.eq(a.data[i], b.data[i]) then return false end
        end
        return true
    end
    if is_py_type(a, "dict") and is_py_type(b, "dict") then
        if #a.keys_order ~= #b.keys_order then return false end
        for _, k in ipairs(a.keys_order) do
            if not py.eq(a.data[k], b.data[k]) then return false end
        end
        return true
    end
    if is_py_type(a, "tuple") and is_py_type(b, "tuple") then
        if a.length ~= b.length then return false end
        for i = 1, a.length do
            if not py.eq(a.data[i], b.data[i]) then return false end
        end
        return true
    end
    return false
end

function py.contains(container, val)
    if is_py_type(container, "list") or is_py_type(container, "tuple") then
        for i = 1, container.length do
            if py.eq(container.data[i], val) then return true end
        end
        return false
    elseif is_py_type(container, "dict") then
        return container.data[val] ~= nil
    elseif type(container) == "string" then
        if type(val) ~= "string" then
            error("TypeError: 'in <string>' requires string as left operand", 0)
        end
        return string.find(container, val, 1, true) ~= nil
    end
    error("TypeError: argument of type '" .. py.type(container) .. "' is not iterable", 0)
end

-- ============================================================
-- List methods
-- ============================================================

function py.append(list, val)
    list.length = list.length + 1
    list.data[list.length] = val
end

function py.extend(list, other)
    if is_py_type(other, "list") or is_py_type(other, "tuple") then
        for i = 1, other.length do
            py.append(list, other.data[i])
        end
    else
        for val in py.iter(other) do
            py.append(list, val)
        end
    end
end

function py.pop(list, idx)
    if list.length == 0 then error("IndexError: pop from empty list", 0) end
    if idx == nil then idx = list.length - 1 end -- default: last (0-based)
    if idx < 0 then idx = list.length + idx end
    local actual = idx + 1 -- 0→1 based
    if actual < 1 or actual > list.length then
        error("IndexError: pop index out of range", 0)
    end
    local val = list.data[actual]
    table.remove(list.data, actual)
    list.length = list.length - 1
    return val
end

function py.insert(list, idx, val)
    if idx < 0 then idx = math.max(list.length + idx + 1, 0) end
    local actual = idx + 1
    table.insert(list.data, actual, val)
    list.length = list.length + 1
end

function py.sort(list, key, reverse)
    local data = list.data
    -- Stable sort: decorate with original index, sort, undecorate.
    -- This preserves insertion order for equal keys (Python guarantee).
    local decorated = {}
    for i = 1, list.length do
        decorated[i] = { val = data[i], idx = i }
    end
    table.sort(decorated, function(a, b)
        local va = key and key(a.val) or a.val
        local vb = key and key(b.val) or b.val
        if va == vb then
            -- Tiebreak by original index for stability
            if reverse then return a.idx > b.idx end
            return a.idx < b.idx
        end
        if reverse then return va > vb end
        return va < vb
    end)
    for i = 1, list.length do
        data[i] = decorated[i].val
    end
end

function py.reverse(list)
    local d = list.data
    local n = list.length
    for i = 1, math.floor(n / 2) do
        d[i], d[n - i + 1] = d[n - i + 1], d[i]
    end
end

-- ============================================================
-- Dict methods
-- ============================================================

function py.keys(dict)
    local result = py.list({})
    for _, k in ipairs(dict.keys_order) do
        py.append(result, k)
    end
    return result
end

function py.values(dict)
    local result = py.list({})
    for _, k in ipairs(dict.keys_order) do
        py.append(result, dict.data[k])
    end
    return result
end

function py.items(dict)
    local result = py.list({})
    for _, k in ipairs(dict.keys_order) do
        py.append(result, py.tuple({ k, dict.data[k] }))
    end
    return result
end

function py.get(dict, key, default)
    local val = dict.data[key]
    if val == nil then return default end
    if val == py.None then return py.None end
    return val
end

function py.update(dict, other)
    if is_py_type(other, "dict") then
        for _, k in ipairs(other.keys_order) do
            py.setindex(dict, k, other.data[k])
        end
    end
end

-- ============================================================
-- String methods
-- ============================================================

function py.str(val)
    if val == nil or val == py.None then return "None" end
    if type(val) == "boolean" then return val and "True" or "False" end
    if type(val) == "string" then return val end
    if type(val) == "number" then return tostring(val) end
    if is_py_float(val) then
        local n = val.v
        if n ~= n then return "nan" end
        if n == math.huge then return "inf" end
        if n == -math.huge then return "-inf" end
        if math.floor(n) == n then return string.format("%.1f", n) end
        return tostring(n)
    end
    if is_py_type(val, "list") then
        local parts = {}
        for i = 1, val.length do table.insert(parts, py.repr(val.data[i])) end
        return "[" .. table.concat(parts, ", ") .. "]"
    end
    if is_py_type(val, "dict") then
        local parts = {}
        for _, k in ipairs(val.keys_order) do
            table.insert(parts, py.repr(k) .. ": " .. py.repr(val.data[k]))
        end
        return "{" .. table.concat(parts, ", ") .. "}"
    end
    if is_py_type(val, "tuple") then
        local parts = {}
        for i = 1, val.length do table.insert(parts, py.repr(val.data[i])) end
        if val.length == 1 then return "(" .. parts[1] .. ",)" end
        return "(" .. table.concat(parts, ", ") .. ")"
    end
    return tostring(val)
end

function py.repr(val)
    if type(val) == "string" then return "'" .. val .. "'" end
    return py.str(val)
end

function py.str_upper(s) return string.upper(s) end
function py.str_lower(s) return string.lower(s) end

function py.str_split(s, sep)
    local result = py.list({})
    if sep == nil or sep == "" then
        -- Split on whitespace
        for word in string.gmatch(s, "%S+") do
            py.append(result, word)
        end
    else
        local start = 1
        while true do
            local i, j = string.find(s, sep, start, true)
            if not i then
                py.append(result, string.sub(s, start))
                break
            end
            py.append(result, string.sub(s, start, i - 1))
            start = j + 1
        end
    end
    return result
end

function py.str_join(sep, list)
    local parts = {}
    if is_py_type(list, "list") or is_py_type(list, "tuple") then
        for i = 1, list.length do
            table.insert(parts, py.str(list.data[i]))
        end
    else
        for val in py.iter(list) do
            table.insert(parts, py.str(val))
        end
    end
    return table.concat(parts, sep)
end

function py.str_startswith(s, prefix) return string.sub(s, 1, #prefix) == prefix end
function py.str_endswith(s, suffix) return #suffix == 0 or string.sub(s, -#suffix) == suffix end
function py.str_strip(s, chars)
    if chars == nil then return string.match(s, "^%s*(.-)%s*$") end
    -- strip(chars): remove any chars in the set from both ends
    local i = 1
    while i <= #s and string.find(chars, string.sub(s, i, i), 1, true) do i = i + 1 end
    local j = #s
    while j >= i and string.find(chars, string.sub(s, j, j), 1, true) do j = j - 1 end
    return string.sub(s, i, j)
end
function py.str_lstrip(s, chars)
    if chars == nil then return string.match(s, "^%s*(.*)$") end
    local i = 1
    while i <= #s and string.find(chars, string.sub(s, i, i), 1, true) do i = i + 1 end
    return string.sub(s, i)
end
function py.str_rstrip(s, chars)
    if chars == nil then return string.match(s, "^(.-)%s*$") end
    local j = #s
    while j >= 1 and string.find(chars, string.sub(s, j, j), 1, true) do j = j - 1 end
    return string.sub(s, 1, j)
end

function py.str_replace(s, old, new, count)
    if count == nil then
        local result = {}
        local start = 1
        while true do
            local i, j = string.find(s, old, start, true)
            if not i then
                table.insert(result, string.sub(s, start))
                break
            end
            table.insert(result, string.sub(s, start, i - 1))
            table.insert(result, new)
            start = j + 1
        end
        return table.concat(result)
    else
        local result = {}
        local start = 1
        local n = 0
        while n < count do
            local i, j = string.find(s, old, start, true)
            if not i then break end
            table.insert(result, string.sub(s, start, i - 1))
            table.insert(result, new)
            start = j + 1
            n = n + 1
        end
        table.insert(result, string.sub(s, start))
        return table.concat(result)
    end
end

function py.str_find(s, sub)
    local i = string.find(s, sub, 1, true)
    if i then return i - 1 end -- 0-based
    return -1
end

function py.str_count(s, sub)
    local count = 0
    local start = 1
    while true do
        local i = string.find(s, sub, start, true)
        if not i then break end
        count = count + 1
        start = i + 1
    end
    return count
end

function py.str_title(s)
    -- Capitalize first letter of each word (after non-alpha), lowercase the rest
    local result = {}
    local capitalize_next = true
    for i = 1, #s do
        local c = string.sub(s, i, i)
        if string.match(c, "%a") then
            if capitalize_next then
                table.insert(result, string.upper(c))
                capitalize_next = false
            else
                table.insert(result, string.lower(c))
            end
        else
            table.insert(result, c)
            capitalize_next = true
        end
    end
    return table.concat(result)
end

function py.str_capitalize(s)
    if #s == 0 then return "" end
    return string.upper(string.sub(s, 1, 1)) .. string.lower(string.sub(s, 2))
end

function py.str_isdigit(s)
    if #s == 0 then return false end
    return string.match(s, "^%d+$") ~= nil
end

function py.str_isalpha(s)
    if #s == 0 then return false end
    return string.match(s, "^%a+$") ~= nil
end

function py.str_isupper(s)
    if #s == 0 then return false end
    local has_cased = false
    for i = 1, #s do
        local c = string.sub(s, i, i)
        if string.match(c, "%l") then return false end
        if string.match(c, "%u") then has_cased = true end
    end
    return has_cased
end

function py.str_islower(s)
    if #s == 0 then return false end
    local has_cased = false
    for i = 1, #s do
        local c = string.sub(s, i, i)
        if string.match(c, "%u") then return false end
        if string.match(c, "%l") then has_cased = true end
    end
    return has_cased
end

function py.str_zfill(s, width)
    if #s >= width then return s end
    local pad = width - #s
    if string.sub(s, 1, 1) == "+" or string.sub(s, 1, 1) == "-" then
        return string.sub(s, 1, 1) .. string.rep("0", pad) .. string.sub(s, 2)
    end
    return string.rep("0", pad) .. s
end

-- ============================================================
-- Builtins
-- ============================================================

function py.len(obj)
    if type(obj) == "string" then return #obj end
    if is_py_type(obj, "list") or is_py_type(obj, "tuple") then return obj.length end
    if is_py_type(obj, "dict") then return #obj.keys_order end
    -- Plain Lua tables (e.g., arrays returned by sandbox modules like fuzzy.extract)
    if type(obj) == "table" then return #obj end
    error("TypeError: object of type '" .. py.type(obj) .. "' has no len()", 0)
end

function py.int(val)
    if type(val) == "number" then
        -- Python int() truncates toward zero (not floor)
        if val >= 0 then return math.floor(val) else return math.ceil(val) end
    end
    if type(val) == "string" then
        local n = tonumber(val)
        if n then
            if n >= 0 then return math.floor(n) else return math.ceil(n) end
        end
        error("ValueError: invalid literal for int(): '" .. val .. "'", 0)
    end
    if type(val) == "boolean" then return val and 1 or 0 end
    error("TypeError: int() argument must be a string or a number", 0)
end

function py.float(val)
    if type(val) == "number" then return val + 0.0 end
    if type(val) == "string" then
        local n = tonumber(val)
        if n then return n + 0.0 end
        error("ValueError: could not convert string to float: '" .. val .. "'", 0)
    end
    error("TypeError: float() argument must be a string or a number", 0)
end

function py.abs(val) return math.abs(val) end

function py.min(...)
    local args = { ... }
    -- If single iterable argument
    if #args == 1 and (is_py_type(args[1], "list") or is_py_type(args[1], "tuple")) then
        local obj = args[1]
        if obj.length == 0 then error("ValueError: min() arg is an empty sequence", 0) end
        local result = obj.data[1]
        for i = 2, obj.length do
            if obj.data[i] < result then result = obj.data[i] end
        end
        return result
    end
    local result = args[1]
    for i = 2, #args do
        if args[i] < result then result = args[i] end
    end
    return result
end

function py.max(...)
    local args = { ... }
    if #args == 1 and (is_py_type(args[1], "list") or is_py_type(args[1], "tuple")) then
        local obj = args[1]
        if obj.length == 0 then error("ValueError: max() arg is an empty sequence", 0) end
        local result = obj.data[1]
        for i = 2, obj.length do
            if obj.data[i] > result then result = obj.data[i] end
        end
        return result
    end
    local result = args[1]
    for i = 2, #args do
        if args[i] > result then result = args[i] end
    end
    return result
end

function py.sum(iterable, start)
    start = start or 0
    local total = start
    if is_py_type(iterable, "list") or is_py_type(iterable, "tuple") then
        for i = 1, iterable.length do total = total + iterable.data[i] end
    else
        for val in py.iter(iterable) do total = total + val end
    end
    return total
end

function py.sorted(iterable, key, reverse)
    local result = py.list({})
    if is_py_type(iterable, "list") or is_py_type(iterable, "tuple") then
        for i = 1, iterable.length do py.append(result, iterable.data[i]) end
    else
        for val in py.iter(iterable) do py.append(result, val) end
    end
    py.sort(result, key, reverse)
    return result
end

function py.reversed(iterable)
    if is_py_type(iterable, "list") or is_py_type(iterable, "tuple") then
        local i = iterable.length + 1
        return function()
            i = i - 1
            if i >= 1 then return iterable.data[i] end
            return nil
        end
    end
    error("TypeError: argument to reversed() must be a sequence", 0)
end

function py.print(...)
    local args = { ... }
    local parts = {}
    for i = 1, select("#", ...) do
        table.insert(parts, py.str(args[i]))
    end
    print(table.concat(parts, " "))
end

-- ============================================================
-- Generic method call dispatch
-- ============================================================

local METHOD_MAP = {
    -- list
    append = function(obj, ...) return py.append(obj, ...) end,
    extend = function(obj, ...) return py.extend(obj, ...) end,
    pop = function(obj, ...) return py.pop(obj, ...) end,
    insert = function(obj, ...) return py.insert(obj, ...) end,
    sort = function(obj, ...) return py.sort(obj, ...) end,
    reverse = function(obj, ...) return py.reverse(obj, ...) end,
    -- dict
    keys = function(obj) return py.keys(obj) end,
    values = function(obj) return py.values(obj) end,
    items = function(obj) return py.items(obj) end,
    get = function(obj, ...) return py.get(obj, ...) end,
    update = function(obj, ...) return py.update(obj, ...) end,
    -- string
    upper = function(obj) return py.str_upper(obj) end,
    lower = function(obj) return py.str_lower(obj) end,
    split = function(obj, ...) return py.str_split(obj, ...) end,
    join = function(obj, ...) return py.str_join(obj, ...) end,
    startswith = function(obj, ...) return py.str_startswith(obj, ...) end,
    endswith = function(obj, ...) return py.str_endswith(obj, ...) end,
    strip = function(obj, ...) return py.str_strip(obj, ...) end,
    lstrip = function(obj, ...) return py.str_lstrip(obj, ...) end,
    rstrip = function(obj, ...) return py.str_rstrip(obj, ...) end,
    replace = function(obj, ...) return py.str_replace(obj, ...) end,
    find = function(obj, ...) return py.str_find(obj, ...) end,
    count = function(obj, ...) return py.str_count(obj, ...) end,
    title = function(obj) return py.str_title(obj) end,
    capitalize = function(obj) return py.str_capitalize(obj) end,
    isdigit = function(obj) return py.str_isdigit(obj) end,
    isalpha = function(obj) return py.str_isalpha(obj) end,
    isupper = function(obj) return py.str_isupper(obj) end,
    islower = function(obj) return py.str_islower(obj) end,
    zfill = function(obj, ...) return py.str_zfill(obj, ...) end,
}

function py.method_call(obj, method, ...)
    -- Check if object has the method directly (e.g. file objects with metatables)
    if type(obj) == "table" then
        local fn = obj[method]
        if type(fn) == "function" then
            return fn(obj, ...)
        end
    end
    local fn = METHOD_MAP[method]
    if fn then return fn(obj, ...) end
    error("AttributeError: '" .. py.type(obj) .. "' object has no attribute '" .. method .. "'", 0)
end

-- ============================================================
-- Error helpers
-- ============================================================

function py.error(typename, message)
    error({ __py_error = typename, message = message }, 0)
end

function py.error_str(err)
    if type(err) == "table" and err.__py_error then
        return err.__py_error .. ": " .. (err.message or "")
    end
    return tostring(err)
end

-- Exception constructors: return error strings matching the "TypeName: msg" pattern
-- used by py.isinstance for except handler matching.
function py.ValueError(msg)
    return "ValueError: " .. tostring(msg or "")
end

function py.TypeError(msg)
    return "TypeError: " .. tostring(msg or "")
end

function py.ZeroDivisionError(msg)
    return "ZeroDivisionError: " .. tostring(msg or "")
end

function py.IndexError(msg)
    return "IndexError: " .. tostring(msg or "")
end

function py.KeyError(msg)
    return "KeyError: " .. tostring(msg or "")
end

function py.RuntimeError(msg)
    return "RuntimeError: " .. tostring(msg or "")
end

function py.Exception(msg)
    return "Exception: " .. tostring(msg or "")
end

-- True division with zero check (Python /)
function py.truediv(a, b)
    if type(a) == "boolean" then a = a and 1 or 0 end
    if type(b) == "boolean" then b = b and 1 or 0 end
    if b == 0 then error("ZeroDivisionError: division by zero", 0) end
    return a / b
end

-- Floor division with zero check (Python //)
function py.floordiv(a, b)
    if type(a) == "boolean" then a = a and 1 or 0 end
    if type(b) == "boolean" then b = b and 1 or 0 end
    if b == 0 then error("ZeroDivisionError: integer division or modulo by zero", 0) end
    return math.floor(a / b)
end

-- Modulo with zero check and Python semantics
function py.mod(a, b)
    if type(a) == "boolean" then a = a and 1 or 0 end
    if type(b) == "boolean" then b = b and 1 or 0 end
    if b == 0 then error("ZeroDivisionError: integer division or modulo by zero", 0) end
    return a % b
end

-- Power
function py.pow(a, b)
    if type(a) == "boolean" then a = a and 1 or 0 end
    if type(b) == "boolean" then b = b and 1 or 0 end
    return a ^ b
end

-- ============================================================
-- File I/O — open()
-- ============================================================

local FILE_MT = {}
FILE_MT.__index = FILE_MT

function FILE_MT:write(content)
    local text = tostring(content or "")
    if self._mode == "a" then
        local existing = ""
        if fs.exists(self._path) then
            existing = fs.read(self._path)
        end
        fs.write(self._path, existing .. text)
    else
        -- "w" mode: accumulate buffer, write each time
        self._buffer = (self._buffer or "") .. text
        fs.write(self._path, self._buffer)
    end
    return #text
end

function FILE_MT:read()
    if fs.exists(self._path) then
        return fs.read(self._path)
    end
    return ""
end

function FILE_MT:close()
    -- No-op in sandbox
end

function py.open(path, mode)
    mode = mode or "r"
    -- Resolve relative paths (default cwd is /)
    local resolved = path
    if string.sub(path, 1, 1) ~= "/" then
        resolved = "/" .. path
    end

    if mode == "w" then
        fs.write(resolved, "")
    elseif mode == "a" then
        if not fs.exists(resolved) then
            fs.write(resolved, "")
        end
    end

    return setmetatable({
        __py_type = "file",
        _path = resolved,
        _mode = mode,
        _buffer = "",
    }, FILE_MT)
end

return py
