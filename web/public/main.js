const MODE_LABELS = new Map([
  [0, "shell"],
  [1, "python"],
  [2, "luau"],
]);

const PROMPTS = new Map([
  [0, "$"],
  [1, ">>>"],
  [2, ">"],
]);

const SHELL_COMMANDS = [
  "apt",
  "base64",
  "bg",
  "brew",
  "cat",
  "cd",
  "cp",
  "curl",
  "cut",
  "date",
  "echo",
  "env",
  "exit",
  "export",
  "false",
  "fg",
  "find",
  "grep",
  "head",
  "help",
  "hostname",
  "id",
  "jobs",
  "kill",
  "ls",
  "mkdir",
  "mv",
  "npm",
  "pip",
  "pip3",
  "printf",
  "ps",
  "pwd",
  "rm",
  "sleep",
  "sort",
  "ssh",
  "sudo",
  "tail",
  "tee",
  "test",
  "top",
  "touch",
  "tr",
  "true",
  "type",
  "uname",
  "uniq",
  "wc",
  "wget",
  "which",
  "yarn",
];

const MODULE_METHODS = {
  base64: ["encode", "decode", "b64encode", "b64decode", "help"],
  fin: ["npv", "irr", "mirr", "pmt", "pv", "fv", "nper", "rate", "help"],
  fs: [
    "read",
    "write",
    "list",
    "exists",
    "writable",
    "mkdir",
    "rename",
    "remove",
    "isdir",
    "isfile",
    "size",
    "copy",
    "tree",
    "help",
  ],
  http: ["get", "post", "put", "patch", "delete", "request", "help"],
  json: ["decode", "encode", "help"],
  random: ["seed", "random", "randint", "uniform", "randrange", "choice", "shuffle", "sample", "help"],
  regex: ["match", "find_all", "replace", "replace_all", "split", "is_match", "escape", "help"],
  url: ["parse", "build", "encode", "decode", "query_parse", "query_build", "join", "help"],
};

const EXAMPLE_PLACEHOLDERS = new Map([
  [0, ['echo hello from CPSL', 'ls /', 'http get "https://httpbin.org/get"']],
  [1, ["print(6 * 7)", 'print("hello from Python")']],
  [
    2,
    [
      'print(json.encode({hello="world"}))',
      'local r = http.get("https://httpbin.org/get"); print(r.status)',
    ],
  ],
]);

const MODULE_NAMES = Object.keys(MODULE_METHODS).sort();
const MODULE_MEMBER_COMPLETIONS = MODULE_NAMES.flatMap((moduleName) =>
  MODULE_METHODS[moduleName].map((method) => `${moduleName}.${method}`)
);

const PATH_COMPLETIONS = [
  "/dev/",
  "/dev/null",
  "/dev/stderr",
  "/dev/stdin",
  "/dev/stdout",
  "/dev/urandom",
  "/dev/zero",
  "/etc/",
  "/etc/hostname",
  "/etc/os-release",
  "/proc/",
  "/proc/cpuinfo",
  "/proc/meminfo",
  "/proc/version",
  "/tmp/",
];

const LUA_COMPLETIONS = [
  "and",
  "break",
  "do",
  "else",
  "elseif",
  "end",
  "false",
  "for",
  "function",
  "help",
  "if",
  "in",
  "local",
  "math",
  "nil",
  "not",
  "or",
  "print",
  "repeat",
  "require",
  "return",
  "string",
  "table",
  "then",
  "true",
  "until",
  "while",
  "utf8",
];

const PYTHON_COMPLETIONS = [
  "False",
  "None",
  "True",
  "and",
  "break",
  "continue",
  "def",
  "dict",
  "elif",
  "else",
  "except",
  "float",
  "for",
  "help",
  "if",
  "in",
  "int",
  "len",
  "list",
  "not",
  "or",
  "print",
  "range",
  "return",
  "str",
  "try",
  "while",
];

const root = document.documentElement;
const terminalShell = document.querySelector(".terminal-shell");
const terminal = document.querySelector(".terminal");
const terminalScreen = document.querySelector("#terminal-screen");
const output = document.querySelector("#terminal-output");
const form = document.querySelector("#terminal-form");
const input = document.querySelector("#terminal-input");
const placeholderEl = document.querySelector("#terminal-placeholder");
const promptEl = document.querySelector("#terminal-prompt");
const clearButton = document.querySelector("#clear-terminal");
const fullscreenButton = document.querySelector("#fullscreen-terminal");
const themeToggle = document.querySelector(".theme-toggle");
const modeButtons = [...document.querySelectorAll(".mode-tabs [data-mode]")];
const allowedDomainsEl = document.querySelector("#allowed-domains");
const copyrightYearEl = document.querySelector("#copyright-year");

let mode = 0;
let prompt = "$";
let ready = false;
let busy = false;
let history = [];
let historyIndex = 0;
let placeholderTimer = 0;
let placeholderIndex = 0;
let placeholderCharacter = 0;
let placeholderDeleting = false;
let commandSent = false;
let demoInViewport = true;
let shouldRestorePromptFocus = false;

const BUILD_ID = "__CPSL_BUILD_ID__";
const cacheSuffix = BUILD_ID.startsWith("__") ? "" : `?v=${encodeURIComponent(BUILD_ID)}`;

const worker = new Worker(`./cpsl.worker.js${cacheSuffix}`, { type: "module" });

function applyTheme(theme) {
  if (theme === "dark") {
    root.dataset.theme = "dark";
    themeToggle.setAttribute("aria-pressed", "true");
  } else {
    root.dataset.theme = "light";
    themeToggle.setAttribute("aria-pressed", "false");
  }
}

const savedTheme = localStorage.getItem("theme");
const systemDark = window.matchMedia("(prefers-color-scheme: dark)").matches;
applyTheme(savedTheme || (systemDark ? "dark" : "light"));

if (copyrightYearEl) {
  copyrightYearEl.textContent = new Date().getFullYear().toString();
}

themeToggle.addEventListener("click", () => {
  const next = root.dataset.theme === "dark" ? "light" : "dark";
  localStorage.setItem("theme", next);
  applyTheme(next);
});

function scrollTerminalToBottom() {
  requestAnimationFrame(() => {
    terminalScreen.scrollTop = terminalScreen.scrollHeight;
  });
}

function currentPlaceholderExamples() {
  return EXAMPLE_PLACEHOLDERS.get(mode) || EXAMPLE_PLACEHOLDERS.get(0) || [];
}

function setPromptPlaceholder(value) {
  if (commandSent || !demoInViewport || input.value) {
    if (placeholderEl) placeholderEl.textContent = "";
    return;
  }
  if (placeholderEl) placeholderEl.textContent = value;
}

function stopPlaceholderLoop() {
  window.clearTimeout(placeholderTimer);
  if (placeholderEl) placeholderEl.textContent = "";
}

function schedulePlaceholder(delay) {
  if (commandSent || !demoInViewport) {
    stopPlaceholderLoop();
    return;
  }

  window.clearTimeout(placeholderTimer);
  placeholderTimer = window.setTimeout(typePlaceholder, delay);
}

function typePlaceholder() {
  if (commandSent || !demoInViewport) {
    stopPlaceholderLoop();
    return;
  }

  const examples = currentPlaceholderExamples();
  if (!examples.length) return;

  if (input.value) {
    if (placeholderEl) placeholderEl.textContent = "";
    schedulePlaceholder(180);
    return;
  }

  const example = examples[placeholderIndex % examples.length];
  let delay = 54;

  if (placeholderDeleting) {
    placeholderCharacter = Math.max(0, placeholderCharacter - 1);
    delay = 24;
    if (placeholderCharacter === 0) {
      placeholderDeleting = false;
      placeholderIndex = (placeholderIndex + 1) % examples.length;
      delay = 260;
    }
  } else {
    placeholderCharacter = Math.min(example.length, placeholderCharacter + 1);
    if (placeholderCharacter === example.length) {
      placeholderDeleting = true;
      delay = 1450;
    }
  }

  setPromptPlaceholder(example.slice(0, placeholderCharacter));
  schedulePlaceholder(delay);
}

function restartPlaceholderLoop() {
  window.clearTimeout(placeholderTimer);
  placeholderCharacter = 0;
  placeholderDeleting = false;
  if (placeholderEl) placeholderEl.textContent = "";

  if (commandSent || !demoInViewport) return;

  schedulePlaceholder(180);
}

function extractAllowedDomains(toml) {
  let inHttpSection = false;
  let assignment = "";
  let collectingAllowedDomains = false;

  for (const line of toml.split(/\r?\n/)) {
    const trimmed = line.trim();

    if (collectingAllowedDomains) {
      assignment += trimmed;
      if (trimmed.includes("]")) break;
      continue;
    }

    if (/^\[[^\]]+\]$/.test(trimmed)) {
      inHttpSection = trimmed === "[http]";
      continue;
    }
    if (!inHttpSection) continue;

    const match = trimmed.match(/^allowed_domains\s*=\s*(\[.*)$/);
    if (match) {
      assignment = match[1];
      if (assignment.includes("]")) break;
      collectingAllowedDomains = true;
    }
  }

  if (!assignment) return [];

  return [...assignment.matchAll(/"((?:[^"\\]|\\.)*)"/g)].map((match) => {
    try {
      return JSON.parse(`"${match[1]}"`);
    } catch {
      return match[1];
    }
  });
}

function renderAllowedDomains(domains) {
  if (!allowedDomainsEl) return;

  allowedDomainsEl.replaceChildren();
  const visibleDomains = domains.length ? domains : ["none"];

  for (const domain of visibleDomains) {
    const code = document.createElement("code");
    code.textContent = domain;
    allowedDomainsEl.append(code);
  }
}

async function loadAllowedDomains() {
  try {
    const response = await fetch(`./cpsl-web.toml${cacheSuffix}`);
    if (!response.ok) return;
    renderAllowedDomains(extractAllowedDomains(await response.text()));
  } catch {
    // Keep the HTML fallback when the static manifest is unavailable.
  }
}

function terminalOwnsFocus() {
  return terminalShell.contains(document.activeElement);
}

function restorePageScroll(scrollX, scrollY) {
  if (window.scrollX === scrollX && window.scrollY === scrollY) return;

  const previousBehavior = root.style.scrollBehavior;
  root.style.scrollBehavior = "auto";
  window.scrollTo(scrollX, scrollY);
  root.style.scrollBehavior = previousBehavior;
}

function focusPrompt() {
  const pageScrollX = window.scrollX;
  const pageScrollY = window.scrollY;

  try {
    input.focus({ preventScroll: true });
  } catch {
    input.focus();
  }

  restorePageScroll(pageScrollX, pageScrollY);
  requestAnimationFrame(() => restorePageScroll(pageScrollX, pageScrollY));
  scrollTerminalToBottom();
}

function clearConsole() {
  output.replaceChildren();
  commandSent = false;
  if (terminalOwnsFocus() && document.activeElement instanceof HTMLElement) {
    document.activeElement.blur();
  }
  restartPlaceholderLoop();
  scrollTerminalToBottom();
}

function setFullPage(expanded) {
  terminalShell.classList.toggle("is-full-page", expanded);
  document.body.classList.toggle("terminal-full-page", expanded);
  fullscreenButton.classList.toggle("is-expanded", expanded);
  fullscreenButton.setAttribute("aria-pressed", expanded ? "true" : "false");
  fullscreenButton.setAttribute(
    "aria-label",
    expanded ? "Exit full page terminal" : "Expand terminal to full page"
  );
  fullscreenButton.title = expanded ? "Exit full page" : "Full page";
  focusPrompt();
}

function appendLine(text, kind = "output", linePrompt = "") {
  const line = document.createElement("div");
  line.className = `terminal-line ${kind}`;

  if (linePrompt) {
    const promptNode = document.createElement("span");
    promptNode.className = "prompt";
    promptNode.textContent = linePrompt;
    const value = document.createElement("span");
    value.textContent = text;
    line.append(promptNode, value);
  } else {
    line.textContent = text;
  }

  output.append(line);
  scrollTerminalToBottom();
}

function appendBlock(text, kind = "output") {
  if (!text) return;
  for (const line of text.replace(/\r\n/g, "\n").split("\n")) {
    appendLine(line, kind);
  }
}

function setMode(nextMode, announce = true, { refocus = false } = {}) {
  mode = nextMode;
  prompt = PROMPTS.get(mode) || "$";
  promptEl.textContent = prompt;

  for (const button of modeButtons) {
    const selected = Number(button.dataset.mode) === mode;
    button.setAttribute("aria-selected", selected ? "true" : "false");
  }

  if (announce) {
    appendLine(`mode: ${MODE_LABELS.get(mode)}`, "system");
  }

  restartPlaceholderLoop();
  if (refocus && demoInViewport) {
    focusPrompt();
  } else {
    scrollTerminalToBottom();
  }
}

function setBusy(nextBusy) {
  busy = nextBusy;
  const commandReady = ready && !nextBusy;
  form.hidden = !commandReady;
  input.disabled = !commandReady;
}

function sendEval(command) {
  if (!ready || busy) return;

  shouldRestorePromptFocus = terminalOwnsFocus() && demoInViewport;
  commandSent = true;
  stopPlaceholderLoop();
  appendLine(command, "command", prompt);
  if (command.trim()) {
    history.push(command);
    historyIndex = history.length;
  }

  setBusy(true);
  worker.postMessage({ type: "eval", mode, input: command });
}

function uniqueSorted(items) {
  return [...new Set(items)].sort((a, b) => a.localeCompare(b));
}

function isClearShortcut(event) {
  return event.key.toLowerCase() === "k" && (event.metaKey || event.ctrlKey) && !event.altKey;
}

function completionRange(value, cursor) {
  const before = value.slice(0, cursor);
  const match = before.match(/[A-Za-z0-9_./-]*$/);
  const start = match ? cursor - match[0].length : cursor;
  return {
    start,
    end: input.selectionEnd ?? cursor,
    prefix: value.slice(start, cursor),
  };
}

function shellTokensBeforeCurrent(value, start) {
  const segment = value.slice(0, start).split(/[|;&]/).pop().trim();
  return segment ? segment.split(/\s+/) : [];
}

function candidatesForInput(value, range) {
  if (range.prefix.startsWith("/")) {
    return PATH_COMPLETIONS;
  }

  if (mode === 0) {
    const tokens = shellTokensBeforeCurrent(value, range.start);
    if (tokens.length === 1 && MODULE_METHODS[tokens[0]]) {
      return MODULE_METHODS[tokens[0]];
    }
    if (tokens.length === 0) {
      return uniqueSorted([...SHELL_COMMANDS, ...MODULE_NAMES]);
    }
    return [];
  }

  if (range.prefix.includes(".")) {
    return MODULE_MEMBER_COMPLETIONS;
  }

  const languageItems = mode === 1 ? PYTHON_COMPLETIONS : LUA_COMPLETIONS;
  return uniqueSorted([...languageItems, ...MODULE_NAMES, ...MODULE_MEMBER_COMPLETIONS]);
}

function commonPrefix(values) {
  if (!values.length) return "";

  let prefix = values[0];
  for (const value of values.slice(1)) {
    while (!value.startsWith(prefix) && prefix) {
      prefix = prefix.slice(0, -1);
    }
  }
  return prefix;
}

function replaceInputRange(start, end, value) {
  input.value = `${input.value.slice(0, start)}${value}${input.value.slice(end)}`;
  const nextCursor = start + value.length;
  input.setSelectionRange(nextCursor, nextCursor);
}

function showCompletionList(matches) {
  const visible = matches.slice(0, 24);
  const suffix = matches.length > visible.length ? "  ..." : "";
  appendLine(`${visible.join("  ")}${suffix}`, "system");
}

function completeInput() {
  const cursor = input.selectionStart ?? input.value.length;
  const range = completionRange(input.value, cursor);
  const candidates = candidatesForInput(input.value, range);
  const matches = candidates.filter((candidate) =>
    candidate.toLowerCase().startsWith(range.prefix.toLowerCase())
  );

  if (!matches.length) return;

  if (matches.length === 1) {
    replaceInputRange(range.start, range.end, matches[0]);
    return;
  }

  const shared = commonPrefix(matches);
  if (shared.length > range.prefix.length) {
    replaceInputRange(range.start, range.end, shared);
    return;
  }

  showCompletionList(matches);
}

form.addEventListener("submit", (event) => {
  event.preventDefault();
  const command = input.value;
  input.value = "";
  sendEval(command);
});

input.addEventListener("keydown", (event) => {
  if (event.key === "Tab") {
    event.preventDefault();
    completeInput();
    return;
  }

  if (event.key === "ArrowUp") {
    event.preventDefault();
    historyIndex = Math.max(0, historyIndex - 1);
    input.value = history[historyIndex] || "";
    queueMicrotask(() => input.setSelectionRange(input.value.length, input.value.length));
    return;
  }

  if (event.key === "ArrowDown") {
    event.preventDefault();
    historyIndex = Math.min(history.length, historyIndex + 1);
    input.value = history[historyIndex] || "";
    queueMicrotask(() => input.setSelectionRange(input.value.length, input.value.length));
    return;
  }

  if (event.key.toLowerCase() === "l" && event.ctrlKey) {
    event.preventDefault();
    clearConsole();
  }
});

input.addEventListener("input", () => {
  if (input.value) {
    if (placeholderEl) placeholderEl.textContent = "";
  }
});

terminal.addEventListener("click", (event) => {
  if (event.target !== input) {
    focusPrompt();
  }
});

if ("IntersectionObserver" in window) {
  const placeholderObserver = new IntersectionObserver(
    ([entry]) => {
      demoInViewport = entry.isIntersecting && entry.intersectionRatio > 0.25;
      if (demoInViewport) {
        restartPlaceholderLoop();
      } else {
        stopPlaceholderLoop();
        if (terminalOwnsFocus()) input.blur();
      }
    },
    { threshold: [0, 0.25, 0.5, 1] }
  );
  placeholderObserver.observe(terminalShell);
}

modeButtons.forEach((button) => {
  button.addEventListener("click", (event) =>
    setMode(Number(button.dataset.mode), true, { refocus: event.detail > 0 })
  );
});

clearButton.addEventListener("click", () => {
  clearConsole();
});

fullscreenButton.addEventListener("click", () => {
  setFullPage(!terminalShell.classList.contains("is-full-page"));
});

document.addEventListener("keydown", (event) => {
  if (isClearShortcut(event)) {
    if (!terminalOwnsFocus()) return;
    event.preventDefault();
    event.stopPropagation();
    clearConsole();
    return;
  }

  if (event.key === "Escape" && terminalShell.classList.contains("is-full-page")) {
    setFullPage(false);
  }
}, { capture: true });

worker.addEventListener("message", (event) => {
  const message = event.data;

  if (message.type === "ready") {
    ready = true;
    setBusy(false);
    appendLine("CPSL WASM runtime ready", "system");
    scrollTerminalToBottom();
    return;
  }

  if (message.type === "reset") {
    ready = true;
    setBusy(false);
    prompt = PROMPTS.get(mode) || "$";
    promptEl.textContent = prompt;
    appendLine("session reset", "system");
    if (shouldRestorePromptFocus && demoInViewport) {
      focusPrompt();
    } else {
      scrollTerminalToBottom();
    }
    shouldRestorePromptFocus = false;
    return;
  }

  if (message.type === "result") {
    setBusy(false);
    if (Array.isArray(message.warnings)) {
      message.warnings.forEach((warning) => appendBlock(warning, "warning"));
    }
    if (message.ok) {
      appendBlock(message.output, "output");
    } else {
      appendBlock(message.error || "CPSL execution failed", "error");
    }
    prompt = message.prompt || prompt;
    promptEl.textContent = prompt;
    if (shouldRestorePromptFocus && demoInViewport) {
      focusPrompt();
    } else {
      scrollTerminalToBottom();
    }
    shouldRestorePromptFocus = false;
    return;
  }

  if (message.type === "error") {
    ready = false;
    setBusy(false);
    input.disabled = true;
    appendBlock(message.message, "error");
  }
});

worker.addEventListener("error", (event) => {
  ready = false;
  setBusy(false);
  input.disabled = true;
  appendBlock(event.message || "CPSL worker failed to start", "error");
});

setBusy(true);
appendLine("loading CPSL WASM runtime...", "system");
loadAllowedDomains();
restartPlaceholderLoop();
worker.postMessage({ type: "init" });
