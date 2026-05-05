//! Python `random` module for the Luau sandbox.
//!
//! Exposes `random.*` as a global table: `random()`, `randint()`, `choice()`,
//! `shuffle()`, `uniform()`, `randrange()`, `sample()`, `seed()`.
//!
//! Uses Luau's built-in `math.random` / `math.randomseed` for the RNG engine.
//! Functions that operate on py.list / py.dict types are implemented in Luau
//! (evaluated at registration time) because they need direct access to pyrt
//! metatables without FFI overhead.

use crate::sandbox::{wrap_module_with_help_hints, FnDoc, ModuleDoc, Param, ParamType, ReturnType};
use mlua::Lua;

pub(crate) static RANDOM_DOC: ModuleDoc = ModuleDoc {
    name: "random",
    summary: "Random number generation",
    functions: &[
        FnDoc {
            name: "seed",
            description:
                "Initialize the random number generator. Without arguments, uses a time-based seed.",
            params: &[Param {
                name: "n",
                short: Some('n'),
                typ: ParamType::Number,
                required: false,
                fields: None,
            }],
            returns: ReturnType::Void,
            example: None,
        },
        FnDoc {
            name: "random",
            description: "Return a random float in [0, 1).",
            params: &[],
            returns: ReturnType::Number,
            example: Some(r#"local r = random.random() -- e.g. 0.7312"#),
        },
        FnDoc {
            name: "randint",
            description: "Return a random integer N such that a <= N <= b (inclusive).",
            params: &[
                Param {
                    name: "a",
                    short: Some('a'),
                    typ: ParamType::Number,
                    required: true,
                    fields: None,
                },
                Param {
                    name: "b",
                    short: Some('b'),
                    typ: ParamType::Number,
                    required: true,
                    fields: None,
                },
            ],
            returns: ReturnType::Number,
            example: Some(r#"random.randint(1, 100)"#),
        },
        FnDoc {
            name: "uniform",
            description: "Return a random float N such that a <= N <= b.",
            params: &[
                Param {
                    name: "a",
                    short: Some('a'),
                    typ: ParamType::Number,
                    required: true,
                    fields: None,
                },
                Param {
                    name: "b",
                    short: Some('b'),
                    typ: ParamType::Number,
                    required: true,
                    fields: None,
                },
            ],
            returns: ReturnType::Number,
            example: None,
        },
        FnDoc {
            name: "randrange",
            description: "Return a random element from range(start, stop, step).",
            params: &[
                Param {
                    name: "start",
                    short: Some('s'),
                    typ: ParamType::Number,
                    required: true,
                    fields: None,
                },
                Param {
                    name: "stop",
                    short: None,
                    typ: ParamType::Number,
                    required: false,
                    fields: None,
                },
                Param {
                    name: "step",
                    short: None,
                    typ: ParamType::Number,
                    required: false,
                    fields: None,
                },
            ],
            returns: ReturnType::Number,
            example: Some("random.randrange(0, 100, 5)  -- random multiple of 5 in [0, 100)"),
        },
        FnDoc {
            name: "choice",
            description: "Return a random element from a non-empty sequence.",
            params: &[Param {
                name: "seq",
                short: Some('s'),
                typ: ParamType::Value,
                required: true,
                fields: None,
            }],
            returns: ReturnType::Value,
            example: Some(r#"random.choice({"red", "green", "blue"})"#),
        },
        FnDoc {
            name: "shuffle",
            description: "Shuffle a list in place.",
            params: &[Param {
                name: "list",
                short: Some('l'),
                typ: ParamType::Value,
                required: true,
                fields: None,
            }],
            returns: ReturnType::Void,
            example: None,
        },
        FnDoc {
            name: "sample",
            description: "Return a list of k unique elements chosen from the population.",
            params: &[
                Param {
                    name: "population",
                    short: Some('p'),
                    typ: ParamType::Value,
                    required: true,
                    fields: None,
                },
                Param {
                    name: "k",
                    short: Some('k'),
                    typ: ParamType::Number,
                    required: true,
                    fields: None,
                },
            ],
            returns: ReturnType::Value,
            example: Some(r#"random.sample({1,2,3,4,5}, 3) -- 3 unique items"#),
        },
    ],
};

/// Register `random.*` globals in the Lua VM.
///
/// Pure-numeric functions (seed, random, randint, uniform, randrange) are
/// implemented in Rust via mlua. Collection-aware functions (choice, shuffle,
/// sample) are implemented in a Luau chunk evaluated at registration time,
/// because they need direct access to pyrt metatables (`__py_type`, `.data`,
/// `.length`) without FFI overhead.
pub(crate) fn register_random_globals(lua: &Lua) -> Result<(), mlua::Error> {
    // Evaluate a Luau chunk that creates the full `random` table.
    // This gives us access to math.random/math.randomseed AND pyrt types.
    lua.load(
        r#"
        local random = {}

        function random.seed(n)
            if n ~= nil then
                math.randomseed(n)
            else
                math.randomseed(tick and tick() or 0)
            end
        end

        function random.random()
            return math.random()
        end

        function random.randint(a, b)
            return math.random(a, b)
        end

        function random.uniform(a, b)
            return a + (b - a) * math.random()
        end

        function random.randrange(start, stop, step)
            if stop == nil then
                return math.random(0, start - 1)
            end
            step = step or 1
            if step == 0 then
                error("ValueError: zero step for randrange()", 0)
            end
            local n = math.ceil((stop - start) / step)
            if n <= 0 then
                error("ValueError: empty range for randrange()", 0)
            end
            return start + step * math.random(0, n - 1)
        end

        function random.choice(seq)
            local py_type = type(seq) == "table" and seq.__py_type
            if py_type == "list" or py_type == "tuple" then
                if seq.length == 0 then
                    error("IndexError: Cannot choose from an empty sequence", 0)
                end
                return seq.data[math.random(1, seq.length)]
            elseif type(seq) == "string" then
                if #seq == 0 then
                    error("IndexError: Cannot choose from an empty sequence", 0)
                end
                local i = math.random(1, #seq)
                return string.sub(seq, i, i)
            elseif type(seq) == "table" then
                if #seq == 0 then
                    error("IndexError: Cannot choose from an empty sequence", 0)
                end
                return seq[math.random(1, #seq)]
            end
            error("TypeError: object is not indexable", 0)
        end

        function random.shuffle(list)
            local py_type = type(list) == "table" and list.__py_type
            if py_type == "list" then
                local d = list.data
                local n = list.length
                for i = n, 2, -1 do
                    local j = math.random(1, i)
                    d[i], d[j] = d[j], d[i]
                end
            elseif type(list) == "table" then
                local n = #list
                for i = n, 2, -1 do
                    local j = math.random(1, i)
                    list[i], list[j] = list[j], list[i]
                end
            else
                error("TypeError: object is not a mutable sequence", 0)
            end
        end

        function random.sample(population, k)
            local pool
            local n

            local py_type = type(population) == "table" and population.__py_type
            if py_type == "list" or py_type == "tuple" then
                n = population.length
                pool = {}
                for i = 1, n do pool[i] = population.data[i] end
            elseif type(population) == "table" then
                n = #population
                pool = {}
                for i = 1, n do pool[i] = population[i] end
            else
                error("TypeError: Population must be a sequence", 0)
            end

            if k > n then
                error("ValueError: Sample larger than population", 0)
            end

            -- Build result as a py.list if pyrt is available (via require)
            local result
            local ok, py_mod = pcall(require, "pyrt")
            if ok and type(py_mod) == "table" and type(py_mod.list) == "function" then
                result = py_mod.list({})
                for i = 1, k do
                    local j = math.random(i, n)
                    pool[i], pool[j] = pool[j], pool[i]
                    py_mod.append(result, pool[i])
                end
            else
                result = {}
                for i = 1, k do
                    local j = math.random(i, n)
                    pool[i], pool[j] = pool[j], pool[i]
                    table.insert(result, pool[i])
                end
            end
            return result
        end

        _G.random = random
        "#,
    )
    .exec()?;

    // Register help functions on the table
    let random_table: mlua::Table = lua.globals().get("random")?;
    crate::lua_util::register_help_functions(lua, &random_table, &RANDOM_DOC)?;
    wrap_module_with_help_hints(lua, "random")?;

    Ok(())
}
