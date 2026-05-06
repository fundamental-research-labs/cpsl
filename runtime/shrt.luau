-- shrt.luau — Shell runtime for transpiled shell→Luau code
-- Every shell command maps to a sh.* function call.
-- Piped data flows as strings (newline-separated lines), just like real shell.

local sh = {}

-- ============================================================
-- State
-- ============================================================
sh.last_exit_code = 0
local cwd = "/"
sh.cwd = cwd
local pipe_depth = 0  -- >0 means we're inside a pipe; suppress printing

-- Environment variables table — mirrors shell env
sh.env = {
    HOME = "/",
    PWD = "/",
    PATH = "/usr/bin:/bin",
    USER = "sandbox",
    SHELL = "/bin/sh",
    LANG = "en_US.UTF-8",
}

-- Positional parameters ($0, $1, ...)
local positional_args = {}

function sh.set(name, value)
    sh.env[name] = tostring(value or "")
end

function sh.get(name)
    return sh.env[name] or ""
end

-- Expand $VAR and ${VAR} references in a string at runtime
function sh.expand(str)
    if str == nil then return "" end
    -- ${VAR} form
    local result = string.gsub(str, "%${([^}]+)}", function(name)
        return sh.env[name] or ""
    end)
    -- $VAR form (word chars only, must not be preceded by \)
    result = string.gsub(result, "%$([%w_]+)", function(name)
        return sh.env[name] or ""
    end)
    -- Special: $? → last exit code
    result = string.gsub(result, "%$%?", tostring(sh.last_exit_code))
    -- Special: $# → positional arg count
    result = string.gsub(result, "%$#", tostring(#positional_args))
    return result
end

-- Set positional arguments (for scripts/functions)
function sh.set_args(...)
    positional_args = {...}
end

function sh.arg(n)
    return positional_args[n] or ""
end

-- ============================================================
-- Helpers
-- ============================================================

local function split_lines(s)
    if s == nil or s == "" then return {} end
    local lines = {}
    local start = 1
    while true do
        local i = string.find(s, "\n", start, true)
        if not i then
            local last = string.sub(s, start)
            if last ~= "" then
                table.insert(lines, last)
            end
            break
        end
        table.insert(lines, string.sub(s, start, i - 1))
        start = i + 1
    end
    return lines
end

local function join_lines(lines)
    return table.concat(lines, "\n")
end

local function resolve_path(path)
    if path == nil then return cwd end
    if string.sub(path, 1, 1) == "/" then return path end
    if cwd == "/" then return "/" .. path end
    return cwd .. "/" .. path
end

local function tostring_val(v)
    if v == nil then return "" end
    return tostring(v)
end

-- Forward-declare emit so helpers above its definition close over this local.
-- Luau's `local function emit` would not be visible to earlier function bodies.
local emit

-- Wrap a filesystem operation with error handling (shell semantics: print error, set $?=1, continue).
-- Returns (true, result) on success, (false, error_message) on failure.
local function try_io(fn, ...)
    local results = table.pack(pcall(fn, ...))
    if not results[1] then
        local err_msg = tostring(results[2])
        -- Strip Luau stack trace noise — keep just the error message
        local clean = string.match(err_msg, ":%d+:%s*(.+)") or err_msg
        -- Reformat read-only errors: "/workspace/foo.txt: Read-only file system" → "/workspace is read-only"
        if string.find(clean, "Read%-only file system") then
            local path = string.match(clean, "^(.-): Read%-only") or ""
            local parent = string.match(path, "^(/[^/]+)") or path
            if parent ~= "" then
                clean = parent .. " is read-only"
            end
        end
        emit(clean)
        sh.last_exit_code = 1
        return false
    end
    sh.last_exit_code = 0
    return true, table.unpack(results, 2, results.n)
end

-- Check if a path is writable, emitting a clear error if not.
-- Returns true if writable, false if read-only (with error emitted + exit code set).
-- Uses fs.writable() when available (requires Rust rebuild), otherwise skips pre-check
-- and lets try_io catch the error from the actual write operation.
local function check_writable(resolved)
    if type(fs.writable) == "function" and not fs.writable(resolved) then
        local parent = string.match(resolved, "(.+)/") or "/"
        emit(parent .. " is read-only")
        sh.last_exit_code = 1
        return false
    end
    return true
end

-- Print only when not inside a pipe (pipe_depth == 0)
-- Strips one trailing newline since print() adds its own.
emit = function(s)
    if pipe_depth == 0 and s ~= nil and s ~= "" then
        if string.sub(s, -1) == "\n" then
            s = string.sub(s, 1, -2)
        end
        if s ~= "" then
            print(s)
        end
    end
end

-- ============================================================
-- Core commands
-- ============================================================

function sh.echo(...)
    local n_args = select("#", ...)
    local start = 1
    local no_newline = false

    -- Check for -n flag (suppress trailing newline)
    if n_args >= 1 then
        local first = select(1, ...)
        if first == "-n" then
            no_newline = true
            start = 2
        end
    end

    local parts = {}
    for i = start, n_args do
        local v = select(i, ...)
        if type(v) == "table" then
            -- Glob result: expand table entries
            for _, item in ipairs(v) do
                table.insert(parts, tostring_val(item))
            end
        else
            table.insert(parts, tostring_val(v))
        end
    end
    local result = table.concat(parts, " ")
    if no_newline then
        if pipe_depth == 0 and result ~= "" then
            __write(result)
        end
    else
        emit(result)
    end
    return result
end

function sh.printf(fmt, ...)
    local result = fmt or ""
    local args = {...}
    local i = 0
    -- Handle format specifiers: %s (string), %d (integer), %f (float), %% (literal %)
    result = string.gsub(result, "%%([sdfl%%])", function(spec)
        if spec == "%" then return "%" end
        i = i + 1
        local val = args[i]
        if spec == "d" then
            return tostring(math.floor(tonumber(tostring_val(val)) or 0))
        elseif spec == "f" then
            return tostring(tonumber(tostring_val(val)) or 0)
        else
            return tostring_val(val)
        end
    end)
    -- Handle \n and \t in format string
    result = string.gsub(result, "\\n", "\n")
    result = string.gsub(result, "\\t", "\t")
    emit(result)
    return result
end

function sh.ls(opts)
    -- Accept: sh.ls(), sh.ls("/path"), sh.ls({l=true, a=true, [1]="/path"})
    if type(opts) == "string" then opts = {[1] = opts} end
    opts = opts or {}

    -- Resolve short → long
    local path = opts.path or opts[1]
    local long = opts.long or opts.l
    local all = opts.all or opts.a
    local human = opts.human_readable or opts.h

    local resolved = resolve_path(path)

    -- Human-readable size formatting (B, K, M, G)
    local function format_size(bytes)
        if bytes < 1024 then
            return string.format("%dB", bytes)
        elseif bytes < 1024 * 1024 then
            return string.format("%.1fK", bytes / 1024)
        elseif bytes < 1024 * 1024 * 1024 then
            return string.format("%.1fM", bytes / (1024 * 1024))
        else
            return string.format("%.1fG", bytes / (1024 * 1024 * 1024))
        end
    end

    -- If path points to a file (not a directory), just display it
    if fs.exists(resolved) and not fs.isdir(resolved) then
        local name = string.match(resolved, "[^/]+$") or resolved
        if not long then
            emit(name)
            return name
        end
        local ok, size = try_io(fs.size, resolved)
        if not ok then return "" end
        local size_str = human and format_size(size) or tostring(size)
        local result = string.format("%-4s %8s  %s", "file", size_str, name)
        emit(result)
        return result
    end

    local ok, raw_entries = try_io(fs.list, resolved)
    if not ok then return "" end

    -- Filter hidden files unless -a is set
    local entries = {}
    for _, name in ipairs(raw_entries) do
        if all or string.sub(name, 1, 1) ~= "." then
            table.insert(entries, name)
        end
    end

    if not long then
        local result = join_lines(entries)
        emit(result)
        return result
    end

    -- Long format: type + size + name
    local lines = {}
    for _, name in ipairs(entries) do
        local full = resolved == "/" and ("/" .. name) or (resolved .. "/" .. name)
        local kind = "file"
        if fs.isdir(full) then kind = "dir" end
        if kind == "file" then
            local ok_s, size = try_io(fs.size, full)
            local size_str = ok_s and (human and format_size(size) or tostring(size)) or "?"
            table.insert(lines, string.format("%-4s %8s  %s", kind, size_str, name))
        else
            table.insert(lines, string.format("%-4s %8s  %s", kind, "-", name))
        end
    end
    local result = join_lines(lines)
    emit(result)
    return result
end

function sh.cat(...)
    local args = {...}
    if #args == 0 then return "" end
    local parts = {}
    for _, arg in ipairs(args) do
        -- If it looks like a path (starts with / or has no newlines and isn't multiline),
        -- try to read it as a file
        if type(arg) == "string" and string.find(arg, "\n", 1, true) == nil and string.sub(arg, 1, 1) == "/" then
            local ok, content = try_io(fs.read, arg)
            if ok then table.insert(parts, content) end
        elseif type(arg) == "string" and string.find(arg, "\n", 1, true) == nil and fs.exists(resolve_path(arg)) then
            local ok, content = try_io(fs.read, resolve_path(arg))
            if ok then table.insert(parts, content) end
        else
            table.insert(parts, tostring_val(arg))
        end
    end
    local result = table.concat(parts, "")
    emit(result)
    return result
end

function sh.pwd()
    emit(cwd)
    return cwd
end

function sh.cd(path)
    local target
    if path == nil then
        target = "/"
    else
        target = resolve_path(path)
    end
    if not fs.isdir(target) then
        print("cd: " .. (path or "") .. ": No such file or directory")
        sh.last_exit_code = 1
        return
    end
    cwd = target
    sh.cwd = cwd
    sh.env.PWD = cwd
    sh.last_exit_code = 0
end

-- ============================================================
-- Text processing
-- ============================================================

function sh.head(opts, n_arg)
    -- Accept: sh.head("text", 5), sh.head({n=5, [1]=content, input=piped})
    local input, n
    if type(opts) == "table" then
        n = opts.n or opts.lines or 10
        input = opts.input or opts[1] or ""
    else
        input = opts
        n = n_arg or 10
    end
    if type(n) == "string" then n = tonumber(n) or 10 end
    local lines = split_lines(tostring_val(input))
    local result_lines = {}
    for i = 1, math.min(n, #lines) do
        table.insert(result_lines, lines[i])
    end
    local result = join_lines(result_lines)
    emit(result)
    return result
end

function sh.tail(opts, n_arg)
    -- Accept: sh.tail("text", 5), sh.tail({n=5, [1]=content, input=piped})
    local input, n
    if type(opts) == "table" then
        n = opts.n or opts.lines or 10
        input = opts.input or opts[1] or ""
    else
        input = opts
        n = n_arg or 10
    end
    if type(n) == "string" then n = tonumber(n) or 10 end
    local lines = split_lines(tostring_val(input))
    local start = math.max(1, #lines - n + 1)
    local result_lines = {}
    for i = start, #lines do
        table.insert(result_lines, lines[i])
    end
    local result = join_lines(result_lines)
    emit(result)
    return result
end

function sh.grep(opts, pattern_arg)
    -- Accept: sh.grep("text", "pat"), sh.grep({i=true, [1]="pat", [2]=content, input=piped})
    local input, pattern, ignore_case, invert, count_only, show_line_nums
    if type(opts) == "table" then
        pattern = opts[1] or ""
        input = opts.input or opts[2] or ""
        ignore_case = opts.ignore_case or opts.i
        invert = opts.invert or opts.v
        count_only = opts.count or opts.c
        show_line_nums = opts.line_number or opts.n
    else
        input = opts
        pattern = pattern_arg or ""
    end
    if input == nil then return "" end
    local lines = split_lines(tostring_val(input))
    local matched = {}
    for line_num, line in ipairs(lines) do
        local search_line = line
        local search_pat = pattern
        if ignore_case then
            search_line = string.lower(line)
            search_pat = string.lower(pattern)
        end
        local found = string.find(search_line, search_pat, 1, true) ~= nil
        if invert then found = not found end
        if found then
            if show_line_nums then
                table.insert(matched, tostring(line_num) .. ":" .. line)
            else
                table.insert(matched, line)
            end
        end
    end
    if count_only then
        local result = tostring(#matched)
        emit(result)
        sh.last_exit_code = (#matched > 0) and 0 or 1
        return result
    end
    local result = join_lines(matched)
    if result ~= "" then
        emit(result)
    end
    sh.last_exit_code = (#matched > 0) and 0 or 1
    return result
end

function sh.wc(opts, flag_arg)
    -- Accept: sh.wc("text", "-l"), sh.wc({l=true, [1]=content, input=piped})
    local input, count_lines, count_words, count_chars
    if type(opts) == "table" then
        input = opts.input or opts[1] or ""
        count_lines = opts.lines or opts.l
        count_words = opts.words or opts.w
        count_chars = opts.chars or opts.c or opts.bytes
    else
        input = opts
        if flag_arg == "-l" then count_lines = true
        elseif flag_arg == "-w" then count_words = true
        elseif flag_arg == "-c" then count_chars = true end
    end
    local text = tostring_val(input)
    local result
    if count_lines then
        local lines = split_lines(text)
        result = tostring(#lines)
    elseif count_words then
        local count = 0
        for _ in string.gmatch(text, "%S+") do count = count + 1 end
        result = tostring(count)
    elseif count_chars then
        result = tostring(#text)
    else
        local lines = split_lines(text)
        local words = 0
        for _ in string.gmatch(text, "%S+") do words = words + 1 end
        result = string.format("%d %d %d", #lines, words, #text)
    end
    emit(result)
    return result
end

function sh.sort(opts, flag_arg)
    -- Accept: sh.sort("text", "-r"), sh.sort({r=true, n=true, [1]=content, input=piped})
    local input, reverse, numeric, unique
    if type(opts) == "table" then
        input = opts.input or opts[1] or ""
        reverse = opts.reverse or opts.r
        numeric = opts.numeric_sort or opts.n
        unique = opts.unique or opts.u
    else
        input = opts
        if flag_arg == "-n" then numeric = true
        elseif flag_arg == "-r" then reverse = true end
    end
    local lines = split_lines(tostring_val(input))
    if numeric and reverse then
        table.sort(lines, function(a, b)
            return (tonumber(a) or 0) > (tonumber(b) or 0)
        end)
    elseif numeric then
        table.sort(lines, function(a, b)
            return (tonumber(a) or 0) < (tonumber(b) or 0)
        end)
    elseif reverse then
        table.sort(lines, function(a, b) return a > b end)
    else
        table.sort(lines)
    end
    if unique then
        local deduped = {}
        local prev = nil
        for _, line in ipairs(lines) do
            if line ~= prev then
                table.insert(deduped, line)
                prev = line
            end
        end
        lines = deduped
    end
    local result = join_lines(lines)
    emit(result)
    return result
end

function sh.uniq(opts)
    -- Accept: sh.uniq("text"), sh.uniq({[1]=content, input=piped})
    local input
    if type(opts) == "table" then
        input = opts.input or opts[1] or ""
    else
        input = opts
    end
    local lines = split_lines(tostring_val(input))
    local result_lines = {}
    local prev = nil
    for _, line in ipairs(lines) do
        if line ~= prev then
            table.insert(result_lines, line)
            prev = line
        end
    end
    local result = join_lines(result_lines)
    emit(result)
    return result
end

-- Expand character ranges like 'a-z' → 'abcdefghijklmnopqrstuvwxyz'
local function expand_char_range(s)
    local result = {}
    local i = 1
    while i <= #s do
        if i + 2 <= #s and string.sub(s, i + 1, i + 1) == "-" then
            local from = string.byte(s, i)
            local to = string.byte(s, i + 2)
            if from <= to then
                for c = from, to do table.insert(result, string.char(c)) end
            else
                for c = from, to, -1 do table.insert(result, string.char(c)) end
            end
            i = i + 3
        else
            table.insert(result, string.sub(s, i, i))
            i = i + 1
        end
    end
    return table.concat(result)
end

function sh.tr(input, set1_or_flag, set2_or_set1, set2_extra)
    local text = tostring_val(input)
    local delete_mode = false
    local squeeze_mode = false
    local set1, set2

    -- Parse: tr -d 'chars', tr -s 'chars', tr 'set1' 'set2'
    if set1_or_flag == "-d" then
        delete_mode = true
        set1 = set2_or_set1
    elseif set1_or_flag == "-s" then
        squeeze_mode = true
        set1 = set2_or_set1
    else
        set1 = set1_or_flag
        set2 = set2_or_set1
    end

    if set1 then set1 = expand_char_range(set1) end
    if set2 then set2 = expand_char_range(set2) end

    if delete_mode and set1 then
        local del_set = {}
        for i = 1, #set1 do del_set[string.sub(set1, i, i)] = true end
        local result = string.gsub(text, ".", function(c)
            if del_set[c] then return "" end
            return c
        end)
        emit(result)
        return result
    elseif squeeze_mode and set1 then
        local sq_set = {}
        for i = 1, #set1 do sq_set[string.sub(set1, i, i)] = true end
        local result = {}
        local prev = nil
        for i = 1, #text do
            local c = string.sub(text, i, i)
            if sq_set[c] and c == prev then
                -- squeeze: skip consecutive duplicates
            else
                table.insert(result, c)
            end
            prev = c
        end
        local out = table.concat(result)
        emit(out)
        return out
    elseif set1 and set2 then
        local map = {}
        for i = 1, #set1 do
            local to_char = (i <= #set2) and string.sub(set2, i, i) or string.sub(set2, #set2, #set2)
            map[string.sub(set1, i, i)] = to_char
        end
        local result = string.gsub(text, ".", function(c) return map[c] or c end)
        emit(result)
        return result
    end
    emit(text)
    return text
end

function sh.cut(input, ...)
    local text = tostring_val(input)
    local lines = split_lines(text)
    local result_lines = {}
    local delim = "\t"
    local fields = nil

    -- Parse variadic flag-value pairs: "-d", ",", "-f", "2"
    local extra = {...}
    local i = 1
    while i <= #extra do
        if extra[i] == "-d" and i + 1 <= #extra then
            delim = extra[i + 1]
            i = i + 2
        elseif extra[i] == "-f" and i + 1 <= #extra then
            fields = tonumber(extra[i + 1])
            i = i + 2
        else
            i = i + 1
        end
    end

    for _, line in ipairs(lines) do
        if fields then
            local parts = {}
            local start = 1
            while true do
                local j = string.find(line, delim, start, true)
                if not j then
                    table.insert(parts, string.sub(line, start))
                    break
                end
                table.insert(parts, string.sub(line, start, j - 1))
                start = j + #delim
            end
            table.insert(result_lines, parts[fields] or "")
        else
            table.insert(result_lines, line)
        end
    end
    local result = join_lines(result_lines)
    emit(result)
    return result
end

-- ============================================================
-- File operations
-- ============================================================

function sh.mkdir(opts)
    -- Accept: sh.mkdir("/path"), sh.mkdir({p=true, [1]="/path"})
    local path
    if type(opts) == "table" then
        path = opts.path or opts[1]
    else
        path = opts
    end
    local resolved = resolve_path(path)
    if not check_writable(resolved) then return end
    try_io(fs.mkdir, resolved)
end

function sh.touch(path)
    local resolved = resolve_path(path)
    if not fs.exists(resolved) then
        if not check_writable(resolved) then return end
        try_io(fs.write, resolved, "")
    else
        sh.last_exit_code = 0
    end
end

function sh.cp(opts, dst_arg)
    -- Accept: sh.cp("/src", "/dst"), sh.cp({r=true, [1]="/src", [2]="/dst"})
    local src, dst
    if type(opts) == "table" then
        src = opts.source or opts[1]
        dst = opts.dest or opts[2]
    else
        src = opts
        dst = dst_arg
    end
    local resolved_src = resolve_path(src)
    local resolved_dst = resolve_path(dst)
    if not check_writable(resolved_dst) then return end
    try_io(fs.copy, resolved_src, resolved_dst)
end

function sh.mv(src, dst)
    local resolved_src = resolve_path(src)
    local resolved_dst = resolve_path(dst)
    if not check_writable(resolved_dst) then return end
    try_io(fs.rename, resolved_src, resolved_dst)
end

function sh.rm(opts, ...)
    -- Accept: sh.rm("/path"), sh.rm(glob_result), sh.rm({r=true, f=true, [1]="/path"})
    local force = false
    if type(opts) == "table" and opts[1] ~= nil and type(opts[1]) == "string" then
        -- Could be a flag table {r=true, [1]="/path"} or a glob result {"/a", "/b"}
        -- If it has boolean values, it's a flag table
        local is_flag_table = false
        for k, v in pairs(opts) do
            if type(k) == "string" and type(v) == "boolean" then
                is_flag_table = true
                if k == "f" then force = true end
            end
        end
        if is_flag_table then
            -- Extract positional paths from table
            local i = 1
            while opts[i] ~= nil do
                local val = opts[i]
                if type(val) == "table" then
                    for _, path in ipairs(val) do
                        if not force or fs.exists(resolve_path(path)) then
                            try_io(fs.remove, resolve_path(path))
                        end
                    end
                else
                    if not force or fs.exists(resolve_path(val)) then
                        try_io(fs.remove, resolve_path(val))
                    end
                end
                i = i + 1
            end
        else
            -- Glob result: iterate table of paths
            for _, path in ipairs(opts) do
                try_io(fs.remove, resolve_path(path))
            end
        end
    elseif type(opts) == "string" then
        try_io(fs.remove, resolve_path(opts))
    end
    -- Handle varargs for backward compat
    local extra = {...}
    for _, arg in ipairs(extra) do
        if type(arg) == "table" then
            for _, path in ipairs(arg) do
                try_io(fs.remove, resolve_path(path))
            end
        else
            try_io(fs.remove, resolve_path(arg))
        end
    end
end

function sh.find(opts, ...)
    -- Accept: sh.find("/path", "-name", "*.csv"), sh.find({[1]="/path", name="*.csv", type="f"})
    local path, name_pattern, type_filter, max_depth

    if type(opts) == "table" then
        path = opts.path or opts[1] or "."
        name_pattern = opts.name
        type_filter = opts.type
        max_depth = opts.maxdepth and tonumber(opts.maxdepth)
    else
        path = opts or "."
        local args = {...}
        local i = 1
        while i <= #args do
            if args[i] == "-name" and i + 1 <= #args then
                name_pattern = args[i + 1]
                i = i + 2
            elseif args[i] == "-type" and i + 1 <= #args then
                type_filter = args[i + 1]
                i = i + 2
            elseif args[i] == "-maxdepth" and i + 1 <= #args then
                max_depth = tonumber(args[i + 1])
                i = i + 2
            else
                i = i + 1
            end
        end
    end

    local resolved = resolve_path(path)

    -- Convert shell glob name pattern to Lua pattern
    local function name_matches(name)
        if name_pattern == nil then return true end
        local lua_pat = "^" .. string.gsub(string.gsub(string.gsub(name_pattern, "%.", "%%."), "%*", ".*"), "%?", ".") .. "$"
        return string.find(name, lua_pat) ~= nil
    end

    local results = {}
    local function walk(dir, depth)
        if max_depth and depth > max_depth then return end
        local ok, entries = pcall(fs.list, dir)
        if not ok then return end
        for _, entry in ipairs(entries) do
            local full = dir == "/" and ("/" .. entry) or (dir .. "/" .. entry)
            local is_dir = fs.isdir(full)
            local is_file = fs.isfile(full)
            local type_ok = true
            if type_filter == "f" then type_ok = is_file
            elseif type_filter == "d" then type_ok = is_dir end
            if type_ok and name_matches(entry) then
                table.insert(results, full)
            end
            if is_dir then
                walk(full, depth + 1)
            end
        end
    end

    walk(resolved, 1)
    local result = join_lines(results)
    emit(result)
    return result
end

function sh.tree(opts, ...)
    -- Accept: sh.tree("/path"), sh.tree({[1]="/path", d=true, L=3})
    local path, depth, dirs_only

    if type(opts) == "table" then
        path = opts.path or opts[1]
        dirs_only = opts.d == true
        depth = opts.L and tonumber(opts.L)
    else
        path = opts
        local args = {...}
        local i = 1
        while i <= #args do
            if args[i] == "-d" then
                dirs_only = true
                i = i + 1
            elseif args[i] == "-L" and i + 1 <= #args then
                depth = tonumber(args[i + 1])
                i = i + 2
            else
                i = i + 1
            end
        end
    end

    -- resolve_path(".") produces "/." when cwd is "/" which breaks virtual dir lookup.
    -- Use cwd directly when no path or "." is given; resolve otherwise.
    if path == nil or path == "." then
        path = cwd
    else
        path = resolve_path(path)
    end

    local tree_opts: {[string]: any} = {path = path}
    if depth then tree_opts.depth = depth end
    if dirs_only then tree_opts.dirs_only = true end
    local result = fs.tree(tree_opts)
    emit(result)
    return result
end

function sh.tee(input, file)
    local text = tostring_val(input)
    if file then
        try_io(fs.write, resolve_path(file), text)
    end
    emit(text)
    return text
end

-- ============================================================
-- Output redirections (> and >>)
-- ============================================================

-- Write redirect: echo "test" > file
-- Suppresses stdout (captures output) and writes to file, creating it if needed.
-- Adds trailing newline for non-empty output (matching print()/emit() behavior).
function sh.redirect_write(path, fn)
    pipe_depth = pipe_depth + 1
    local result = fn()
    pipe_depth = pipe_depth - 1
    if path == "/dev/null" then return end
    local resolved = resolve_path(path)
    if not check_writable(resolved) then return end
    local text = tostring_val(result)
    if text ~= "" then text = text .. "\n" end
    try_io(fs.write, resolved, text)
end

-- Append redirect: echo "test" >> file
-- Suppresses stdout (captures output) and appends to file, creating it if needed.
-- Adds trailing newline for non-empty output (matching print()/emit() behavior).
function sh.redirect_append(path, fn)
    pipe_depth = pipe_depth + 1
    local result = fn()
    pipe_depth = pipe_depth - 1
    if path == "/dev/null" then return end
    local resolved = resolve_path(path)
    if not check_writable(resolved) then return end
    local text = tostring_val(result)
    if text ~= "" then text = text .. "\n" end
    local existing = ""
    if fs.exists(resolved) then
        local ok, content = try_io(fs.read, resolved)
        if not ok then return end
        existing = content
    end
    try_io(fs.write, resolved, existing .. text)
end

-- ============================================================
-- Tests
-- ============================================================

function sh.test(...)
    local args = {...}
    if #args == 0 then
        sh.last_exit_code = 1
        return false
    end

    -- Negation prefix: ! expr
    if args[1] == "!" then
        local rest = {}
        for i = 2, #args do table.insert(rest, args[i]) end
        local result = not sh.test(unpack(rest))
        sh.last_exit_code = result and 0 or 1
        return result
    end

    local result = false

    -- Unary tests: -f, -d, -e, -r, -w, -s, -z, -n
    if #args == 2 then
        local flag = args[1]
        local val = args[2]
        if flag == "-f" then result = fs.isfile(resolve_path(val))
        elseif flag == "-d" then result = fs.isdir(resolve_path(val))
        elseif flag == "-e" then result = fs.exists(resolve_path(val))
        elseif flag == "-r" then result = fs.exists(resolve_path(val))
        elseif flag == "-w" then result = fs.exists(resolve_path(val))
        elseif flag == "-s" then
            local p = resolve_path(val)
            if fs.isfile(p) then
                local ok, content = try_io(fs.read, p)
                result = ok and #content > 0
            end
        elseif flag == "-z" then result = (val == nil or val == "")
        elseif flag == "-n" then result = (val ~= nil and val ~= "")
        end
    elseif #args == 3 then
        -- Binary tests: =, !=, -eq, -ne, -lt, -le, -gt, -ge
        local a, op, b = args[1], args[2], args[3]
        if op == "=" or op == "==" then result = (a == b)
        elseif op == "!=" then result = (a ~= b)
        elseif op == "-eq" then result = (tonumber(a) == tonumber(b))
        elseif op == "-ne" then result = (tonumber(a) ~= tonumber(b))
        elseif op == "-lt" then result = ((tonumber(a) or 0) < (tonumber(b) or 0))
        elseif op == "-le" then result = ((tonumber(a) or 0) <= (tonumber(b) or 0))
        elseif op == "-gt" then result = ((tonumber(a) or 0) > (tonumber(b) or 0))
        elseif op == "-ge" then result = ((tonumber(a) or 0) >= (tonumber(b) or 0))
        end
    elseif #args == 1 then
        -- Single arg: true if non-empty string
        result = (args[1] ~= nil and args[1] ~= "")
    end

    sh.last_exit_code = result and 0 or 1
    return result
end

-- ============================================================
-- Glob / pattern matching
-- ============================================================

-- Convert a shell glob pattern to a Lua pattern
local function glob_to_lua_pattern(pat)
    local result = "^"
    local i = 1
    while i <= #pat do
        local c = string.sub(pat, i, i)
        if c == "*" then
            -- ** is recursive (match anything including /)
            if string.sub(pat, i + 1, i + 1) == "*" then
                result = result .. ".*"
                i = i + 2
                -- Skip trailing / after **
                if string.sub(pat, i, i) == "/" then i = i + 1 end
            else
                result = result .. "[^/]*"
            end
        elseif c == "?" then
            result = result .. "[^/]"
        elseif c == "[" then
            -- Character class: pass through to Lua pattern
            local j = string.find(pat, "]", i + 1, true)
            if j then
                result = result .. string.sub(pat, i, j)
                i = j + 1
            else
                result = result .. "%["
                i = i + 1
            end
        elseif string.find(c, "[%.%+%-%^%$%(%)%%]", 1) then
            result = result .. "%" .. c
        else
            result = result .. c
        end
        if c ~= "*" and c ~= "[" then
            i = i + 1
        end
    end
    return result .. "$"
end

-- Recursive listing helper for ** globs
local function list_recursive(dir)
    local results = {}
    local ok, entries = pcall(fs.list, dir)
    if not ok then return results end
    for _, name in ipairs(entries) do
        local full = dir == "/" and ("/" .. name) or (dir .. "/" .. name)
        table.insert(results, full)
        -- Recurse into directories
        if fs.isdir(full) then
            local sub = list_recursive(full)
            for _, s in ipairs(sub) do
                table.insert(results, s)
            end
        end
    end
    return results
end

function sh.glob(pattern)
    local has_doublestar = string.find(pattern, "**", 1, true) ~= nil

    if has_doublestar then
        -- Recursive glob: find base dir before **, list recursively, filter
        local base = string.match(pattern, "^(.-)/.*%*%*") or "."
        local resolved = resolve_path(base)
        local all = list_recursive(resolved)
        local lua_pat = glob_to_lua_pattern(pattern)
        local matched = {}
        for _, path in ipairs(all) do
            if string.find(path, lua_pat) then
                table.insert(matched, path)
            end
        end
        return matched
    end

    -- Single-level glob
    local dir = string.match(pattern, "^(.*)/") or "."
    local pat = string.match(pattern, "([^/]+)$") or "*"
    local resolved = resolve_path(dir)
    local ok, entries = pcall(fs.list, resolved)
    if not ok then return {} end

    local lua_pat = glob_to_lua_pattern(pat)
    local matched = {}
    for _, name in ipairs(entries) do
        if string.find(name, lua_pat) then
            if dir == "." then
                table.insert(matched, name)
            else
                table.insert(matched, dir .. "/" .. name)
            end
        end
    end
    return matched
end

function sh.match(value, pattern)
    -- Shell-style pattern matching (for case statements)
    local lua_pat = "^" .. string.gsub(string.gsub(string.gsub(pattern, "%.", "%%."), "%*", ".*"), "%?", ".") .. "$"
    return string.find(tostring_val(value), lua_pat) ~= nil
end

-- ============================================================
-- Pipeline helper
-- ============================================================

-- Capture command output without printing (for command substitution)
function sh.capture(fn)
    pipe_depth = pipe_depth + 1
    local result = fn()
    pipe_depth = pipe_depth - 1
    return result
end

function sh.pipe(...)
    -- All args are functions (thunks/lambdas).
    -- First: function() return sh.cmd(...) end (no stdin)
    -- Rest: function(_in) return sh.cmd(_in, ...) end (receives previous output)
    -- Suppress printing inside the pipeline; only emit the final result.
    pipe_depth = pipe_depth + 1
    local result = nil
    for i = 1, select("#", ...) do
        local fn_or_val = select(i, ...)
        if type(fn_or_val) == "function" then
            result = fn_or_val(result)
        else
            result = fn_or_val
        end
    end
    pipe_depth = pipe_depth - 1
    emit(result)
    return result
end

-- ============================================================
-- Misc
-- ============================================================

function sh.true_cmd()
    sh.last_exit_code = 0
    return true
end

function sh.false_cmd()
    sh.last_exit_code = 1
    return false
end

function sh.exit(code)
    error("exit:" .. tostring(code or 0), 0)
end

-- ============================================================
-- Informational commands
-- ============================================================

function sh.whoami()
    local result = "sandbox"
    emit(result)
    return result
end

function sh.hostname_cmd()
    local info = _G.__sysinfo or {}
    local result = info.hostname or "sandbox"
    emit(result)
    return result
end

function sh.id()
    local result = "uid=1000(sandbox) gid=1000(sandbox)"
    emit(result)
    return result
end

function sh.uname(opts)
    local flag = nil
    if type(opts) == "table" then
        flag = opts[1]
    elseif type(opts) == "string" then
        flag = opts
    end

    local info = _G.__sysinfo or {}
    local sysname = info.sysname or "Sandbox"
    local release = info.release or "1.0"
    local machine = info.machine or "unknown"
    local os_name = info.os or "unknown"
    local os_ver = info.os_version or ""
    local full = sysname .. " " .. release .. " " .. machine .. " " .. os_name .. " " .. os_ver

    local result
    if flag == "-a" then
        result = full
    elseif flag == "-s" then
        result = sysname
    elseif flag == "-r" then
        result = release
    elseif flag == "-m" then
        result = machine
    else
        result = sysname
    end
    emit(result)
    return result
end

function sh.date(opts)
    local fmt = nil
    if type(opts) == "table" then
        fmt = opts[1]
    elseif type(opts) == "string" then
        fmt = opts
    end

    local result
    if fmt then
        -- Strip leading + if present (date +%Y-%m-%d → %Y-%m-%d)
        if string.sub(fmt, 1, 1) == "+" then
            fmt = string.sub(fmt, 2)
        end
        if fmt == "%s" then
            result = tostring(os.time())
        else
            result = os.date(fmt)
        end
    else
        result = os.date()
    end
    emit(result)
    return result
end

function sh.env_cmd()
    local lines = {}
    local keys = {}
    for k in pairs(sh.env) do
        table.insert(keys, k)
    end
    table.sort(keys)
    for _, k in ipairs(keys) do
        table.insert(lines, k .. "=" .. tostring(sh.env[k]))
    end
    local result = table.concat(lines, "\n")
    emit(result)
    return result
end

function sh.export(...)
    -- Parse VAR=value pairs
    for i = 1, select("#", ...) do
        local arg = select(i, ...)
        local name, value = string.match(tostring(arg), "^([%w_]+)=(.*)")
        if name then
            sh.env[name] = value
        end
    end
    sh.last_exit_code = 0
end

function sh.which(name)
    if name == nil then
        print("which: missing argument")
        sh.last_exit_code = 1
        return ""
    end
    -- Check known commands
    local known = {
        echo=true, printf=true, ls=true, cat=true, pwd=true, cd=true,
        head=true, tail=true, grep=true, wc=true, sort=true, uniq=true,
        tr=true, cut=true, mkdir=true, touch=true, cp=true, mv=true,
        rm=true, find=true, tee=true, test=true, whoami=true, hostname=true,
        id=true, uname=true, date=true, env=true, export=true, which=true,
        type_cmd=true, ps=true, sleep=true,
    }
    if known[name] then
        local result = name .. ": sandbox builtin"
        emit(result)
        sh.last_exit_code = 0
        return result
    end
    -- Check if it's a module
    local mod = _G[name]
    if type(mod) == "table" and rawget(mod, "help") ~= nil then
        local result = name .. ": sandbox module"
        emit(result)
        sh.last_exit_code = 0
        return result
    end
    print(name .. " not found")
    sh.last_exit_code = 1
    return ""
end

function sh.type_cmd(name)
    if name == nil then
        print("type: missing argument")
        sh.last_exit_code = 1
        return ""
    end
    local known = {
        echo=true, printf=true, ls=true, cat=true, pwd=true, cd=true,
        head=true, tail=true, grep=true, wc=true, sort=true, uniq=true,
        tr=true, cut=true, mkdir=true, touch=true, cp=true, mv=true,
        rm=true, find=true, tee=true, test=true, whoami=true, hostname=true,
        id=true, uname=true, date=true, env=true, export=true, which=true,
        type_cmd=true, ps=true, sleep=true,
    }
    if known[name] then
        local result = name .. " is a sandbox builtin"
        emit(result)
        sh.last_exit_code = 0
        return result
    end
    -- Check if it's a module
    local mod = _G[name]
    if type(mod) == "table" and rawget(mod, "help") ~= nil then
        local result = name .. " is a sandbox module"
        emit(result)
        sh.last_exit_code = 0
        return result
    end
    print("type: " .. name .. ": not found")
    sh.last_exit_code = 1
    return ""
end

-- Graceful stubs for commands that can't work in the sandbox
function sh.ps()
    local result = "PID TTY TIME CMD\n  1 ?   00:00 sandbox (no process management)"
    emit(result)
    return result
end

function sh.stub_no_process(name)
    print(name .. ": not supported in sandbox (Luau is single-threaded, no process management)")
    sh.last_exit_code = 1
end

function sh.stub_no_network(name)
    local has_http = type(_G["http"]) == "table"
    if has_http then
        print(name .. ": not available in sandbox. Use the http module instead (try: http help)")
    else
        print(name .. ": not available in sandbox (no network module loaded)")
    end
    sh.last_exit_code = 1
end

function sh.stub_no_pkg(name)
    print(name .. ": no package management in sandbox")
    sh.last_exit_code = 1
end

function sh.sleep_cmd()
    print("sleep: not supported in sandbox (Luau is single-threaded)")
    sh.last_exit_code = 1
end

function sh.sudo()
    print("sudo: no privilege escalation in sandbox")
    sh.last_exit_code = 1
end

--- Top-level help for bash mode. Shows commands grouped by category
--- and available modules with shell-style usage hints.
function sh.shell_help()
    local text = "Sandbox Shell\n"
        .. "\n"
        .. "File I/O:       cat, head, tail, tee\n"
        .. "Search:         grep, find\n"
        .. "Text:           sort, uniq, tr, cut, wc, base64\n"
        .. "File ops:       ls, mkdir, touch, cp, mv, rm\n"
        .. "Navigation:     pwd, cd\n"
        .. "Output:         echo, printf\n"
        .. "Logic:          test, true, false, exit\n"
        .. "Info:           whoami, hostname, id, uname, date, env, export, which, type, ps\n"
        .. "Variables:      export VAR=value, $VAR\n"

    -- Dynamically list loaded modules
    local known = {"fs","compress","json","csv","doc","plot","yaml","xml","numx","http","fuzzy","phone","email","country","currency","datetime","image","random","base64","fin","yfinance","edgar","ocr","crypto","regex","html","url","qr","sfae"}
    local lines = {}
    for _, name in ipairs(known) do
        local m = rawget(_G, name)
        if type(m) == "table" then
            local summary = rawget(m, "__summary") or ""
            local pad = string.rep(" ", math.max(1, 13 - #name))
            table.insert(lines, "  " .. name .. pad .. summary)
        end
    end
    if #lines > 0 then
        text = text .. "\nAll modules are available as globals (no import needed).\n"
            .. "Run '<name> help' for detailed usage.\n"
            .. "\nModules:\n"
            .. table.concat(lines, "\n") .. "\n"
    end

    print(text)
end

--- List available shell commands and loaded modules for error hints.
local function available_commands_hint()
    local builtins = {"echo", "printf", "ls", "cat", "pwd", "cd", "head", "tail",
        "grep", "wc", "sort", "uniq", "tr", "cut", "base64",
        "mkdir", "touch", "cp", "mv",
        "rm", "find", "tee", "test", "true", "false", "exit", "help",
        "whoami", "hostname", "id", "uname", "date", "env", "export",
        "which", "type", "ps", "kill", "top", "bg", "fg", "jobs",
        "sleep", "ssh", "curl", "wget", "sudo", "apt", "brew", "pip"}
    local modules = {}
    for k, v in pairs(_G) do
        if type(v) == "table" and k ~= "_G" and k ~= "sh" and k ~= "_VERSION"
            and rawget(v, "help") ~= nil then
            table.insert(modules, k)
        end
    end
    table.sort(modules)
    local hint = "available commands: " .. table.concat(builtins, ", ")
    if #modules > 0 then
        hint = hint .. "\navailable modules:  " .. table.concat(modules, ", ")
    end
    return hint
end

--- Runtime module dispatch: checks _G[name] at runtime.
--- If the global is a table (module), dispatches to module.method(args).
--- If method is nil, calls module.help() (bare module name).
--- If the global is a callable function, calls it directly (e.g. help).
--- Auto-serializes table return values to JSON for shell display.
function sh.run(name, method, args)
    local mod = _G[name]
    if mod == nil then
        error("command not found: " .. tostring(name) .. "\n" .. available_commands_hint(), 0)
    end
    -- Callable global function (e.g. help) — call it directly
    if type(mod) == "function" then
        local result = mod()
        if result ~= nil then emit(tostring(result)) end
        return result
    end
    if type(mod) ~= "table" then
        error("command not found: " .. tostring(name) .. "\n" .. available_commands_hint(), 0)
    end

    -- Bare module name (no method), "help", or any flag (starts with "-") → show help.
    -- Flags like --help, -h, --version, etc. are not valid method names, so show help.
    -- help() accepts an optional mode arg: help("shell") for flag syntax.
    -- Use rawget to bypass __index metatable (which errors on unknown keys).
    local is_flag = type(method) == "string" and string.sub(method, 1, 1) == "-"
    if method == nil or method == "help" or is_flag then
        local help = rawget(mod, "help")
        if type(help) == "function" then
            local result = help("shell")
            if result ~= nil then emit(tostring(result)) end
            return result
        end
        if method == nil then
            error(tostring(name) .. ": no method specified (try '" .. tostring(name) .. " help')", 0)
        end
    end

    local fn = mod[method]
    if fn == nil then
        -- Method not found — suggest help
        local hint = ""
        if type(mod.help) == "function" then
            hint = " (try '" .. tostring(name) .. "' for available methods)"
        end
        error(tostring(name) .. "." .. tostring(method) .. ": unknown method" .. hint, 0)
    end

    -- Unpack named args into positional args using __params metadata.
    -- Shell transpiles `numx add --a 1 --b 2` → sh.run("numx", "add", {a="1", b="2"})
    -- but Rust functions expect positional args: numx.add("1", "2").
    -- __params maps method → ordered param names, so we can unpack.
    local call_args = args
    if type(args) == "table" then
        local params = rawget(mod, "__params")
        if params then
            local param_order = params[method]
            if param_order and #param_order > 0 then
                -- Check if table has named string keys (not just positional [1],[2],...)
                local has_named = false
                for _, pname in ipairs(param_order) do
                    if args[pname] ~= nil then
                        has_named = true
                        break
                    end
                end
                if has_named then
                    -- Coerce string numbers to actual numbers for numeric-looking values
                    local unpacked = {}
                    for _, pname in ipairs(param_order) do
                        local v = args[pname]
                        if type(v) == "string" then
                            local n = tonumber(v)
                            if n then v = n end
                        end
                        table.insert(unpacked, v)
                    end
                    call_args = unpacked
                end
            end
        end
    end

    local ok, result
    if type(call_args) == "table" and call_args[1] ~= nil and rawget(mod, "__params") then
        -- Unpacked positional args
        ok, result = pcall(fn, table.unpack(call_args))
    else
        ok, result = pcall(fn, call_args)
    end
    if not ok then
        local err = tostring(result)
        -- Strip stack traceback if present
        local trace = string.find(err, "\nstack traceback:")
        if trace then err = string.sub(err, 1, trace - 1) end

        -- Detect argument errors and reformat for shell context
        if string.find(err, "missing required argument") then
            -- Reformat: 'name' (type) → --name <type>
            local shell_err = string.gsub(err, "'([%w_]+)' %((%w+)%)", "--%1 <%2>")
            -- Replace "missing required argument" → "missing"
            shell_err = string.gsub(shell_err, "missing required arguments? ", "missing ")
            -- Replace "module.method:" prefix → "module method:"
            shell_err = string.gsub(shell_err, "^(%w+)%.(%w+):", "%1 %2:")

            -- Try to get shell-format help for this specific function
            local fn_help = ""
            local help_fn = rawget(mod, "help")
            if type(help_fn) == "function" then
                -- Capture the shell help output to extract the relevant line
                pipe_depth = pipe_depth + 1
                local help_text = help_fn("shell") or ""
                pipe_depth = pipe_depth - 1
                -- Find the line for this specific method
                for line in string.gmatch(help_text, "[^\n]+") do
                    if string.find(line, name .. " " .. method, 1, true) then
                        fn_help = "\n  usage: " .. line
                        break
                    end
                end
            end

            error(shell_err .. fn_help, 0)
        elseif string.find(err, "expected string, got")
            or string.find(err, "expected table, got")
            or string.find(err, "expected number, got")
            or string.find(err, "expected boolean, got")
            or string.find(err, "bad argument")
        then
            -- Type mismatch errors — reformat module.method → module method
            local shell_err = string.gsub(err, "^(%w+)%.(%w+):", "%1 %2:")
            error(shell_err .. "\n  hint: run '" .. tostring(name) .. "' for usage", 0)
        else
            -- Runtime errors pass through as-is
            error(err, 0)
        end
    end

    -- Auto-serialize: if result is a table, emit as pretty JSON; strings/numbers as-is;
    -- userdata (e.g. numx arrays) via tostring which calls __tostring metamethod.
    if result ~= nil then
        if type(result) == "table" then
            local ok_enc, encoded = pcall(json.encode, result, {pretty = true})
            if ok_enc then
                emit(encoded)
            else
                emit(tostring(result))
            end
        elseif type(result) == "string" or type(result) == "number" then
            emit(tostring(result))
        elseif type(result) == "userdata" then
            emit(tostring(result))
        end
    end

    return result
end

function sh.exec(cmd, ...)
    -- Legacy fallback — error in sandbox (no real exec)
    error("command not found: " .. tostring(cmd) .. "\n" .. available_commands_hint(), 0)
end

function sh.home()
    return "/"
end

function sh.lines(text)
    -- Iterator over lines of text
    local lines = split_lines(tostring_val(text))
    local i = 0
    return function()
        i = i + 1
        if i <= #lines then return lines[i] end
        return nil
    end
end

return sh
