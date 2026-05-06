let moduleInstance = null;
let session = 0;
let evalLine = null;
let freeString = null;
let freeSession = null;
let lastError = null;

const BUILD_ID = "__CPSL_BUILD_ID__";
const cacheSuffix = BUILD_ID.startsWith("__") ? "" : `?v=${encodeURIComponent(BUILD_ID)}`;

async function loadModule() {
  if (moduleInstance) return moduleInstance;

  let createModule;
  try {
    ({ default: createModule } = await import(
      new URL(`./assets/wasm/cpsl.js${cacheSuffix}`, self.location.href).href
    ));
  } catch (error) {
    throw new Error(
      "CPSL WASM bundle is missing. Run ./web/build.sh with Emscripten installed, or let the GitHub Pages workflow build it."
    );
  }

  moduleInstance = await createModule({
    locateFile(path) {
      return new URL(`./assets/wasm/${path}${cacheSuffix}`, self.location.href).href;
    },
  });

  evalLine = moduleInstance.cwrap("cpsl_eval", "number", ["number", "number", "string"]);
  freeString = moduleInstance.cwrap("cpsl_string_free", null, ["number"]);
  freeSession = moduleInstance.cwrap("cpsl_session_free", null, ["number"]);
  lastError = moduleInstance.cwrap("cpsl_last_error", "number", []);
  return moduleInstance;
}

async function createSession() {
  const mod = await loadModule();
  const nextSession = mod.ccall("cpsl_session_new", "number", [], []);
  if (!nextSession) {
    const errorPtr = lastError ? lastError() : 0;
    const message = errorPtr ? mod.UTF8ToString(errorPtr) : "failed to initialize CPSL";
    throw new Error(message);
  }
  session = nextSession;
}

function disposeSession() {
  if (session && freeSession) {
    freeSession(session);
  }
  session = 0;
}

function parseResult(ptr) {
  if (!ptr) throw new Error("CPSL returned an empty response");

  try {
    const json = moduleInstance.UTF8ToString(ptr);
    return JSON.parse(json);
  } finally {
    freeString(ptr);
  }
}

self.addEventListener("message", async (event) => {
  const message = event.data;

  try {
    if (message.type === "init") {
      await createSession();
      self.postMessage({ type: "ready" });
      return;
    }

    if (message.type === "reset") {
      disposeSession();
      await createSession();
      self.postMessage({ type: "reset" });
      return;
    }

    if (message.type === "eval") {
      if (!session) await createSession();
      const ptr = evalLine(session, message.mode, message.input || "");
      self.postMessage({ type: "result", ...parseResult(ptr) });
    }
  } catch (error) {
    self.postMessage({
      type: "error",
      message: error instanceof Error ? error.message : String(error),
    });
  }
});
