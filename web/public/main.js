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

const root = document.documentElement;
const output = document.querySelector("#terminal-output");
const form = document.querySelector("#terminal-form");
const input = document.querySelector("#terminal-input");
const promptEl = document.querySelector("#terminal-prompt");
const resetButton = document.querySelector("#reset-terminal");
const themeToggle = document.querySelector(".theme-toggle");
const modeButtons = [...document.querySelectorAll("[data-mode]")];
const quickButtons = [...document.querySelectorAll(".quick-commands [data-command]")];

let mode = 0;
let prompt = "$";
let ready = false;
let busy = false;
let history = [];
let historyIndex = 0;

const worker = new Worker("./cpsl.worker.js", { type: "module" });

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

themeToggle.addEventListener("click", () => {
  const next = root.dataset.theme === "dark" ? "light" : "dark";
  localStorage.setItem("theme", next);
  applyTheme(next);
});

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
  output.scrollTop = output.scrollHeight;
}

function appendBlock(text, kind = "output") {
  if (!text) return;
  for (const line of text.replace(/\r\n/g, "\n").split("\n")) {
    appendLine(line, kind);
  }
}

function setMode(nextMode, announce = true) {
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

  input.focus();
}

function setBusy(nextBusy) {
  busy = nextBusy;
  input.disabled = nextBusy || !ready;
  resetButton.disabled = nextBusy || !ready;
}

function sendEval(command) {
  if (!ready || busy) return;

  appendLine(command, "command", prompt);
  if (command.trim()) {
    history.push(command);
    historyIndex = history.length;
  }

  setBusy(true);
  worker.postMessage({ type: "eval", mode, input: command });
}

form.addEventListener("submit", (event) => {
  event.preventDefault();
  const command = input.value;
  input.value = "";
  sendEval(command);
});

input.addEventListener("keydown", (event) => {
  if (event.key === "ArrowUp") {
    event.preventDefault();
    historyIndex = Math.max(0, historyIndex - 1);
    input.value = history[historyIndex] || "";
    queueMicrotask(() => input.setSelectionRange(input.value.length, input.value.length));
  }

  if (event.key === "ArrowDown") {
    event.preventDefault();
    historyIndex = Math.min(history.length, historyIndex + 1);
    input.value = history[historyIndex] || "";
    queueMicrotask(() => input.setSelectionRange(input.value.length, input.value.length));
  }

  if (event.key.toLowerCase() === "l" && event.ctrlKey) {
    event.preventDefault();
    output.replaceChildren();
  }
});

modeButtons.forEach((button) => {
  button.addEventListener("click", () => setMode(Number(button.dataset.mode)));
});

quickButtons.forEach((button) => {
  button.addEventListener("click", () => {
    const nextMode = Number(button.dataset.mode);
    if (nextMode !== mode) setMode(nextMode);
    input.value = button.dataset.command;
    input.focus();
  });
});

resetButton.addEventListener("click", () => {
  if (busy) return;
  setBusy(true);
  worker.postMessage({ type: "reset" });
});

worker.addEventListener("message", (event) => {
  const message = event.data;

  if (message.type === "ready") {
    ready = true;
    setBusy(false);
    appendLine("CPSL WASM runtime ready", "system");
    appendLine("try: echo hello from CPSL", "system");
    input.focus();
    return;
  }

  if (message.type === "reset") {
    ready = true;
    setBusy(false);
    prompt = PROMPTS.get(mode) || "$";
    promptEl.textContent = prompt;
    appendLine("session reset", "system");
    input.focus();
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
    input.focus();
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
worker.postMessage({ type: "init" });
