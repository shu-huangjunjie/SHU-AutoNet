// TypeScript Frontend Logic for SHU AutoNet
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

interface AppConfig {
  user_id: string;
  password: string;
  auth_server: string;
  auth_port: number | string;
  test_url: string;
  minimize_to_tray: boolean;
  close_to_tray: boolean;
  start_with_windows: boolean;
  launch_minimized: boolean;
  parse_start_token: string;
  parse_start_offset: number;
  parse_end_token: string;
  theme?: string;
}

// DOM Elements
const themeToggleBtn = document.getElementById("theme_toggle_btn") as HTMLButtonElement;
const userIdInput = document.getElementById("user_id") as HTMLInputElement;
const passwordInput = document.getElementById("password") as HTMLInputElement;
const togglePwdBtn = document.getElementById("toggle_pwd_btn") as HTMLButtonElement;

const authServerInput = document.getElementById("auth_server") as HTMLInputElement;
const authPortInput = document.getElementById("auth_port") as HTMLInputElement;
const testUrlInput = document.getElementById("test_url") as HTMLInputElement;

const advToggle = document.getElementById("adv_toggle") as HTMLElement;
const advArrow = document.getElementById("adv_arrow") as HTMLElement;
const advBody = document.getElementById("adv_body") as HTMLElement;
const parseStartTokenInput = document.getElementById("parse_start_token") as HTMLInputElement;
const parseStartOffsetInput = document.getElementById("parse_start_offset") as HTMLInputElement;
const parseEndTokenInput = document.getElementById("parse_end_token") as HTMLInputElement;

const minimizeToTrayCheck = document.getElementById("minimize_to_tray") as HTMLInputElement;
const closeToTrayCheck = document.getElementById("close_to_tray") as HTMLInputElement;
const startWithWindowsCheck = document.getElementById("start_with_windows") as HTMLInputElement;
const launchMinimizedCheck = document.getElementById("launch_minimized") as HTMLInputElement;

const saveBtn = document.getElementById("save_btn") as HTMLButtonElement;
const toggleMonitorBtn = document.getElementById("toggle_monitor_btn") as HTMLButtonElement;
const monitorBtnText = document.getElementById("monitor_btn_text") as HTMLElement;
const testConnBtn = document.getElementById("test_conn_btn") as HTMLButtonElement;
const clearLogBtn = document.getElementById("clear_log_btn") as HTMLButtonElement;
const logConsole = document.getElementById("log_console") as HTMLElement;

const statusIndicator = document.getElementById("status_indicator") as HTMLElement;
const statusText = document.getElementById("status_text") as HTMLElement;

// Check if running in Tauri environment
const isTauri = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

let isMonitoring = false;

// Theme Switcher Logic
function applyTheme(theme: "dark" | "light") {
  document.documentElement.setAttribute("data-theme", theme);
  localStorage.setItem("app_theme", theme);
}

// SVG Icons for password toggle
const EYE_OPEN_SVG = `
  <svg class="eye-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
    <path d="M1 12s4-8 11-8 11 8 11 8-4 8-11 8-11-8-11-8z"></path>
    <circle cx="12" cy="12" r="3"></circle>
  </svg>
`;

const EYE_CLOSED_SVG = `
  <svg class="eye-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
    <path d="M17.94 17.94A10.07 10.07 0 0 1 12 20c-7 0-11-8-11-8a18.45 18.45 0 0 1 5.06-5.94M9.9 4.24A9.12 9.12 0 0 1 12 4c7 0 11 8 11 8a18.5 18.5 0 0 1-2.16 3.19m-6.72-1.07a3 3 0 1 1-4.24-4.24"></path>
    <line x1="1" y1="1" x2="23" y2="23"></line>
  </svg>
`;

// Append log line with color tags
function appendLog(rawMsg: string) {
  const line = document.createElement("div");
  line.className = "log-line";

  const tagMatch = rawMsg.match(/^\[(.*?)\]\s*(.*)$/);

  if (tagMatch) {
    const rawTag = tagMatch[1].toUpperCase();
    const restText = tagMatch[2];

    const tagSpan = document.createElement("span");
    tagSpan.className = "log-tag";

    if (rawTag.includes("SUCCESS") || rawTag.includes("RESTORED") || rawTag.includes("ONLINE")) {
      tagSpan.classList.add("tag-success");
      tagSpan.textContent = rawTag;
    } else if (rawTag.includes("ERROR") || rawTag.includes("FAIL")) {
      tagSpan.classList.add("tag-error");
      tagSpan.textContent = rawTag;
    } else if (rawTag.includes("WARN")) {
      tagSpan.classList.add("tag-warn");
      tagSpan.textContent = rawTag;
    } else if (rawTag.includes("MONITOR")) {
      tagSpan.classList.add("tag-monitor");
      tagSpan.textContent = rawTag;
    } else if (rawTag.includes("SYSTEM")) {
      tagSpan.classList.add("tag-system");
      tagSpan.textContent = rawTag;
    } else {
      tagSpan.classList.add("tag-info");
      tagSpan.textContent = rawTag;
    }

    const textSpan = document.createElement("span");
    textSpan.className = "log-text";
    textSpan.textContent = restText;

    line.appendChild(tagSpan);
    line.appendChild(textSpan);
  } else {
    const textSpan = document.createElement("span");
    textSpan.className = "log-text";
    textSpan.textContent = rawMsg;
    line.appendChild(textSpan);
  }

  logConsole.appendChild(line);
  logConsole.scrollTop = logConsole.scrollHeight;
}

// Update status indicator and label
function updateStatus(status: "online" | "offline" | "monitoring") {
  if (statusIndicator) {
    statusIndicator.className = `status-orb ${status}`;
  }
  if (statusText) {
    if (status === "online") {
      statusText.textContent = "Online";
    } else if (status === "monitoring") {
      statusText.textContent = "Monitoring Active";
    } else {
      statusText.textContent = "Offline";
    }
  }
}

// Read current form into config object
function readFormConfig(): AppConfig {
  const currentTheme = document.documentElement.getAttribute("data-theme") || "dark";
  return {
    user_id: userIdInput.value.trim(),
    password: passwordInput.value,
    auth_server: authServerInput.value.trim() || "10.10.9.9",
    auth_port: authPortInput.value.trim() || "8080",
    test_url: testUrlInput.value.trim() || "http://123.123.123.123/",
    minimize_to_tray: minimizeToTrayCheck.checked,
    close_to_tray: closeToTrayCheck.checked,
    start_with_windows: startWithWindowsCheck.checked,
    launch_minimized: launchMinimizedCheck.checked,
    parse_start_token: parseStartTokenInput.value || "index.jsp?",
    parse_start_offset: parseInt(parseStartOffsetInput.value, 10) || 10,
    parse_end_token: parseEndTokenInput.value || "'</script>",
    theme: currentTheme,
  };
}

// Populate form from config object
function populateForm(config: AppConfig) {
  userIdInput.value = config.user_id || "";
  passwordInput.value = config.password || "";
  authServerInput.value = config.auth_server || "10.10.9.9";
  authPortInput.value = String(config.auth_port || "8080");
  testUrlInput.value = config.test_url || "http://123.123.123.123/";

  parseStartTokenInput.value = config.parse_start_token || "index.jsp?";
  parseStartOffsetInput.value = String(config.parse_start_offset ?? 10);
  parseEndTokenInput.value = config.parse_end_token || "'</script>";

  minimizeToTrayCheck.checked = config.minimize_to_tray ?? true;
  closeToTrayCheck.checked = config.close_to_tray ?? true;
  startWithWindowsCheck.checked = config.start_with_windows ?? false;
  launchMinimizedCheck.checked = config.launch_minimized ?? false;

  if (config.theme === "light" || config.theme === "dark") {
    applyTheme(config.theme);
  }
}

// Update monitor button and status UI
function setMonitoringUI(running: boolean) {
  isMonitoring = running;
  if (running) {
    if (monitorBtnText) {
      monitorBtnText.textContent = "STOP MONITORING";
    }
    toggleMonitorBtn.classList.add("btn-stop");
    updateStatus("monitoring");
  } else {
    if (monitorBtnText) {
      monitorBtnText.textContent = "START MONITORING";
    }
    toggleMonitorBtn.classList.remove("btn-stop");
    updateStatus("offline");
  }
}

// Initialize Application
async function init() {
  // Pre-load theme from localStorage for instantaneous switch
  const savedTheme = localStorage.getItem("app_theme") as "dark" | "light" | null;
  if (savedTheme === "light" || savedTheme === "dark") {
    applyTheme(savedTheme);
  }

  appendLog("[SYSTEM] SHU AutoNet initialized.");

  // Theme switcher toggle listener
  if (themeToggleBtn) {
    themeToggleBtn.addEventListener("click", () => {
      const curr = document.documentElement.getAttribute("data-theme") || "dark";
      const nextTheme = curr === "dark" ? "light" : "dark";
      applyTheme(nextTheme);

      // Persist in background config
      const cfg = readFormConfig();
      if (isTauri) {
        invoke("save_config", { config: cfg }).catch(() => {});
      }
      appendLog(`[SYSTEM] Switched to ${nextTheme.toUpperCase()} theme.`);
    });
  }

  // Password visibility toggle with SVG swap
  togglePwdBtn.addEventListener("click", () => {
    if (passwordInput.type === "password") {
      passwordInput.type = "text";
      togglePwdBtn.innerHTML = EYE_CLOSED_SVG;
    } else {
      passwordInput.type = "password";
      togglePwdBtn.innerHTML = EYE_OPEN_SVG;
    }
  });

  // Advanced parsing accordion toggle
  advToggle.addEventListener("click", () => {
    const isHidden = advBody.style.display === "none" || advBody.style.display === "";
    advBody.style.display = isHidden ? "block" : "none";
    advArrow.classList.toggle("expanded", isHidden);
  });

  // Clear log console
  clearLogBtn.addEventListener("click", () => {
    logConsole.innerHTML = "";
    appendLog("[SYSTEM] Console cleared.");
  });

  // Save config
  saveBtn.addEventListener("click", async () => {
    const cfg = readFormConfig();
    if (isTauri) {
      try {
        await invoke("save_config", { config: cfg });
        appendLog("[SUCCESS] Configuration saved locally (portable relative path).");
      } catch (err) {
        appendLog(`[ERROR] Failed to save configuration: ${err}`);
      }
    } else {
      localStorage.setItem("mock_config", JSON.stringify(cfg));
      appendLog("[SUCCESS] (Browser Dev) Configuration saved.");
    }
  });

  // Toggle monitoring daemon
  toggleMonitorBtn.addEventListener("click", async () => {
    const cfg = readFormConfig();
    if (!cfg.user_id || !cfg.password) {
      appendLog("[WARN] Credentials missing. Please enter Student ID and Password.");
      return;
    }

    if (isTauri) {
      try {
        await invoke("save_config", { config: cfg });
        const running = (await invoke("toggle_monitoring")) as boolean;
        setMonitoringUI(running);
      } catch (err) {
        appendLog(`[ERROR] Monitor toggle failed: ${err}`);
      }
    } else {
      setMonitoringUI(!isMonitoring);
      appendLog(`[SYSTEM] (Browser Dev) Monitoring state: ${isMonitoring}`);
    }
  });

  // Manual auth test
  testConnBtn.addEventListener("click", async () => {
    const cfg = readFormConfig();
    if (!cfg.user_id || !cfg.password) {
      appendLog("[WARN] User credentials missing.");
      return;
    }

    appendLog("[INFO] Initiating manual authentication probe...");
    if (isTauri) {
      try {
        await invoke("save_config", { config: cfg });
        const result = (await invoke("manual_connect")) as string;
        appendLog(`[AUTH RESULT] ${result}`);
        if (result.toLowerCase().includes("success") || result.toLowerCase().includes("online")) {
          updateStatus("online");
        }
      } catch (err) {
        appendLog(`[ERROR] Manual authentication failed: ${err}`);
      }
    } else {
      setTimeout(() => {
        appendLog("[SUCCESS] (Browser Dev) Simulated authentication passed.");
        updateStatus("online");
      }, 700);
    }
  });

  // Load config and listen to Tauri events
  if (isTauri) {
    try {
      const config = (await invoke("get_config")) as AppConfig;
      populateForm(config);

      // Listen for log events emitted from Rust backend
      await listen<string>("log_event", (event) => {
        appendLog(event.payload);
      });

      // Listen for monitor state changes
      await listen<boolean>("monitor_state", (event) => {
        setMonitoringUI(event.payload);
      });

      await listen<boolean>("network_state", (event) => {
        updateStatus(event.payload ? "online" : "offline");
      });

      // Check current monitoring state
      const running = (await invoke("is_monitoring")) as boolean;
      setMonitoringUI(running);

      // Auto start monitoring after 1s if credentials exist (same as Python original)
      setTimeout(() => {
        if (!isMonitoring && config.user_id && config.password) {
          appendLog("[SYSTEM] Auto-starting network monitoring daemon...");
          // start_monitoring is idempotent, so it cannot stop a monitor started meanwhile (e.g. from the tray)
          invoke("start_monitoring")
            .then((res) => setMonitoringUI(res as boolean))
            .catch((err) => appendLog(`[ERROR] Auto-start failed: ${err}`));
        }
      }, 1000);
    } catch (err) {
      appendLog(`[ERROR] Failed to connect to Rust backend: ${err}`);
    }
  } else {
    // Browser fallback
    const cached = localStorage.getItem("mock_config");
    if (cached) {
      try {
        populateForm(JSON.parse(cached));
      } catch {
        // ignore
      }
    }
    appendLog("[INFO] Browser preview mode active.");
  }
}

// Start after DOM loaded
document.addEventListener("DOMContentLoaded", init);
