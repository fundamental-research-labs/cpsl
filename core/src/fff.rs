//! FFF-backed content search module for the Luau sandbox.

use crate::grep_api::{self, FffGrepProvider, GrepProvider, GrepRequestOptions};
use crate::lua_util::register_help_functions;
use crate::sandbox::{
    wrap_module_with_help_hints, FieldDoc, FnDoc, ModuleDoc, Param, ParamType, ReturnType,
};
use crate::MountTable;
use mlua::{Lua, MultiValue};
use std::sync::Arc;

const FFF_GREP_OPTS_FIELDS: &[FieldDoc] = &[
    FieldDoc {
        name: "pattern",
        typ: "string",
        required: true,
        description: "Literal byte pattern to search for",
    },
    FieldDoc {
        name: "path",
        typ: "string",
        required: true,
        description: "File or directory to search",
    },
    FieldDoc {
        name: "max_count",
        typ: "number",
        required: false,
        description: "Maximum number of matching lines to return",
    },
];

pub(crate) static FFF_DOC: ModuleDoc = ModuleDoc {
    name: "fff",
    summary: "Fast literal content search powered by fff-grep",
    functions: &[FnDoc {
        name: "grep",
        description: "Search a file or directory for a literal pattern. Returns matching lines.",
        params: &[Param {
            name: "opts",
            short: None,
            typ: ParamType::Table,
            required: true,
            fields: Some(FFF_GREP_OPTS_FIELDS),
        }],
        returns: ReturnType::Table,
        example: Some(r#"fff.grep({pattern="TODO", path="/workspace", max_count=20})"#),
    }],
};

pub(crate) fn register_fff_globals(lua: &Lua, mounts: Arc<MountTable>) -> Result<(), mlua::Error> {
    let fff_table = lua.create_table()?;
    let provider = FffGrepProvider::byte_search(mounts);

    fff_table.set(
        "grep",
        lua.create_function(move |lua, args: MultiValue| {
            let request = grep_api::parse_grep_request(
                &args,
                FFF_DOC.params("grep"),
                "fff.grep",
                GrepRequestOptions::FffAlias,
            )?;
            let results = provider
                .search(&request)
                .map_err(|error| error.into_lua("fff.grep"))?;
            grep_api::grep_results_to_lua(lua, results, true)
        })?,
    )?;

    register_help_functions(lua, &fff_table, &FFF_DOC)?;
    lua.globals().set("fff", fff_table)?;
    wrap_module_with_help_hints(lua, "fff")?;

    Ok(())
}
