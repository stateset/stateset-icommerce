/**
 * Web Chat Channel for StateSet iCommerce
 *
 * Serves a self-contained chat UI as an embedded HTML page, registered as
 * routes on the HTTP gateway.  The channel uses the shared agent pipeline
 * from base.js so users get the same multi-turn, tool-equipped experience
 * they would on Telegram, Discord, or any other channel.
 *
 * Routes:
 *   GET  /chat                   - Chat UI (HTML page)
 *   POST /chat/message           - Send a message  { text, sessionId? } => { response, sessionId }
 *   GET  /chat/history/:sessionId - Retrieve conversation history
 */

import { createSessionManager, processWithAgent, handleBotCommand } from './base.js';
import { runMiddleware } from './middleware.js';
import crypto from 'crypto';

// ============================================================================
// In-memory conversation history
// ============================================================================

/** @type {Map<string, Array<{ role: string, text: string, timestamp: string }>>} */
const conversationHistory = new Map();

function getHistory(sessionId) {
  if (!conversationHistory.has(sessionId)) {
    conversationHistory.set(sessionId, []);
  }
  return conversationHistory.get(sessionId);
}

function pushMessage(sessionId, role, text) {
  const history = getHistory(sessionId);
  history.push({ role, text, timestamp: new Date().toISOString() });
  // Cap at 500 messages per session to bound memory
  if (history.length > 500) {
    history.splice(0, history.length - 500);
  }
}

// ============================================================================
// HTML template
// ============================================================================

function buildChatHTML() {
  return `<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="utf-8" />
<meta name="viewport" content="width=device-width, initial-scale=1" />
<title>StateSet iCommerce Chat</title>
<style>
/* ── Reset ── */
*, *::before, *::after { box-sizing: border-box; margin: 0; padding: 0; }

/* ── CSS Variables ── */
:root {
  --bg: #0f1117;
  --bg-surface: #1a1d27;
  --bg-bubble-user: #3b82f6;
  --bg-bubble-agent: #23262f;
  --text: #e4e4e7;
  --text-muted: #a1a1aa;
  --text-bubble-user: #ffffff;
  --text-bubble-agent: #e4e4e7;
  --border: #2e3140;
  --accent: #3b82f6;
  --accent-hover: #2563eb;
  --input-bg: #1a1d27;
  --scrollbar-track: #1a1d27;
  --scrollbar-thumb: #3f3f46;
  --code-bg: #0d0f14;
  --shadow: 0 -2px 16px rgba(0,0,0,0.25);
  --radius: 12px;
  --radius-sm: 8px;
}

:root.light {
  --bg: #f8f9fb;
  --bg-surface: #ffffff;
  --bg-bubble-user: #3b82f6;
  --bg-bubble-agent: #f0f0f3;
  --text: #18181b;
  --text-muted: #71717a;
  --text-bubble-user: #ffffff;
  --text-bubble-agent: #18181b;
  --border: #e4e4e7;
  --accent: #3b82f6;
  --accent-hover: #2563eb;
  --input-bg: #ffffff;
  --scrollbar-track: #f4f4f5;
  --scrollbar-thumb: #d4d4d8;
  --code-bg: #f4f4f5;
  --shadow: 0 -2px 16px rgba(0,0,0,0.06);
}

html, body {
  height: 100%;
  font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, "Helvetica Neue", Arial, sans-serif;
  background: var(--bg);
  color: var(--text);
}

/* ── Layout ── */
#app {
  display: flex;
  flex-direction: column;
  height: 100%;
  max-width: 720px;
  margin: 0 auto;
}

/* ── Header ── */
header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 14px 20px;
  border-bottom: 1px solid var(--border);
  background: var(--bg-surface);
  flex-shrink: 0;
}

header h1 {
  font-size: 16px;
  font-weight: 600;
  letter-spacing: -0.01em;
}

header .controls {
  display: flex;
  gap: 8px;
}

header button {
  background: transparent;
  border: 1px solid var(--border);
  color: var(--text-muted);
  border-radius: var(--radius-sm);
  padding: 5px 10px;
  font-size: 12px;
  cursor: pointer;
  transition: background 0.15s, color 0.15s;
}

header button:hover {
  background: var(--border);
  color: var(--text);
}

/* ── Messages area ── */
#messages {
  flex: 1;
  overflow-y: auto;
  padding: 20px 16px;
  display: flex;
  flex-direction: column;
  gap: 12px;
}

#messages::-webkit-scrollbar { width: 6px; }
#messages::-webkit-scrollbar-track { background: var(--scrollbar-track); }
#messages::-webkit-scrollbar-thumb { background: var(--scrollbar-thumb); border-radius: 3px; }

/* ── Bubbles ── */
.msg {
  max-width: 85%;
  padding: 10px 14px;
  border-radius: var(--radius);
  line-height: 1.55;
  font-size: 14px;
  word-wrap: break-word;
  overflow-wrap: break-word;
  white-space: pre-wrap;
}

.msg.user {
  align-self: flex-end;
  background: var(--bg-bubble-user);
  color: var(--text-bubble-user);
  border-bottom-right-radius: 4px;
}

.msg.agent {
  align-self: flex-start;
  background: var(--bg-bubble-agent);
  color: var(--text-bubble-agent);
  border-bottom-left-radius: 4px;
}

.msg .ts {
  display: block;
  font-size: 10px;
  margin-top: 6px;
  opacity: 0.55;
}

/* ── Markdown-ish rendering ── */
.msg strong { font-weight: 600; }
.msg em { font-style: italic; }
.msg code {
  font-family: "SFMono-Regular", Menlo, Consolas, monospace;
  font-size: 12.5px;
  background: var(--code-bg);
  padding: 1px 5px;
  border-radius: 4px;
}
.msg pre {
  background: var(--code-bg);
  padding: 10px 12px;
  border-radius: var(--radius-sm);
  overflow-x: auto;
  margin: 6px 0;
  font-size: 12.5px;
  line-height: 1.5;
}
.msg pre code {
  background: none;
  padding: 0;
}
.msg ul, .msg ol {
  padding-left: 20px;
  margin: 4px 0;
}
.msg li { margin: 2px 0; }

/* ── Loading indicator ── */
.loading {
  align-self: flex-start;
  display: flex;
  gap: 5px;
  padding: 14px 18px;
  background: var(--bg-bubble-agent);
  border-radius: var(--radius);
  border-bottom-left-radius: 4px;
}

.loading span {
  width: 8px;
  height: 8px;
  border-radius: 50%;
  background: var(--text-muted);
  animation: bounce 1.4s infinite both;
}

.loading span:nth-child(2) { animation-delay: 0.16s; }
.loading span:nth-child(3) { animation-delay: 0.32s; }

@keyframes bounce {
  0%, 80%, 100% { transform: scale(0.6); opacity: 0.4; }
  40% { transform: scale(1); opacity: 1; }
}

/* ── Input bar ── */
#input-bar {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 12px 16px;
  border-top: 1px solid var(--border);
  background: var(--bg-surface);
  box-shadow: var(--shadow);
  flex-shrink: 0;
}

#input-bar textarea {
  flex: 1;
  resize: none;
  border: 1px solid var(--border);
  background: var(--input-bg);
  color: var(--text);
  border-radius: var(--radius);
  padding: 10px 14px;
  font-size: 14px;
  font-family: inherit;
  line-height: 1.4;
  max-height: 120px;
  outline: none;
  transition: border-color 0.15s;
}

#input-bar textarea:focus {
  border-color: var(--accent);
}

#input-bar textarea::placeholder {
  color: var(--text-muted);
}

#send-btn {
  width: 40px;
  height: 40px;
  border: none;
  border-radius: 50%;
  background: var(--accent);
  color: #fff;
  cursor: pointer;
  display: flex;
  align-items: center;
  justify-content: center;
  flex-shrink: 0;
  transition: background 0.15s, transform 0.1s;
}

#send-btn:hover { background: var(--accent-hover); }
#send-btn:active { transform: scale(0.92); }
#send-btn:disabled { opacity: 0.45; cursor: not-allowed; }

#send-btn svg {
  width: 18px;
  height: 18px;
  fill: currentColor;
}

/* ── Welcome ── */
.welcome {
  text-align: center;
  color: var(--text-muted);
  padding: 48px 24px;
  line-height: 1.6;
}

.welcome h2 {
  font-size: 20px;
  color: var(--text);
  margin-bottom: 8px;
  font-weight: 600;
}

.welcome p { font-size: 14px; }

/* ── Responsive ── */
@media (max-width: 480px) {
  #app { max-width: 100%; }
  header { padding: 10px 14px; }
  #messages { padding: 14px 10px; }
  #input-bar { padding: 10px 12px; }
  .msg { max-width: 92%; font-size: 13.5px; }
}
</style>
</head>
<body>
<div id="app">
  <header>
    <h1>StateSet Commerce</h1>
    <div class="controls">
      <button id="theme-btn" title="Toggle theme">Light</button>
      <button id="reset-btn" title="New conversation">New Chat</button>
    </div>
  </header>
  <div id="messages">
    <div class="welcome">
      <h2>Welcome to StateSet</h2>
      <p>Ask me anything about your orders, products, inventory, returns, and more.</p>
    </div>
  </div>
  <div id="input-bar">
    <textarea id="input" rows="1" placeholder="Type a message..." autocomplete="off"></textarea>
    <button id="send-btn" title="Send">
      <svg viewBox="0 0 24 24"><path d="M2.01 21L23 12 2.01 3 2 10l15 2-15 2z"/></svg>
    </button>
  </div>
</div>

<script>
(function () {
  "use strict";

  // ── State ──
  let sessionId = localStorage.getItem("ss_chat_session") || "";
  let sending = false;

  const messagesEl = document.getElementById("messages");
  const inputEl    = document.getElementById("input");
  const sendBtn    = document.getElementById("send-btn");
  const themeBtn   = document.getElementById("theme-btn");
  const resetBtn   = document.getElementById("reset-btn");

  // ── Theme ──
  const savedTheme = localStorage.getItem("ss_chat_theme") || "dark";
  if (savedTheme === "light") {
    document.documentElement.classList.add("light");
    themeBtn.textContent = "Dark";
  }

  themeBtn.addEventListener("click", function () {
    const isLight = document.documentElement.classList.toggle("light");
    themeBtn.textContent = isLight ? "Dark" : "Light";
    localStorage.setItem("ss_chat_theme", isLight ? "light" : "dark");
  });

  // ── Reset ──
  resetBtn.addEventListener("click", function () {
    sessionId = "";
    localStorage.removeItem("ss_chat_session");
    messagesEl.innerHTML =
      '<div class="welcome"><h2>Welcome to StateSet</h2>' +
      "<p>Ask me anything about your orders, products, inventory, returns, and more.</p></div>";
  });

  // ── Markdown renderer (basic) ──
  function renderMarkdown(text) {
    // Escape HTML
    var s = text
      .replace(/&/g, "&amp;")
      .replace(/</g, "&lt;")
      .replace(/>/g, "&gt;");

    // Code blocks
    s = s.replace(/\x60\x60\x60(.*?)\x60\x60\x60/gs, function (_, code) {
      return "<pre><code>" + code.trim() + "</code></pre>";
    });

    // Inline code
    s = s.replace(/\x60([^\x60]+)\x60/g, "<code>$1</code>");

    // Bold **text** or __text__
    s = s.replace(/[*]{2}(.+?)[*]{2}/g, "<strong>$1</strong>");
    s = s.replace(/__(.+?)__/g, "<strong>$1</strong>");

    // Italic *text* or _text_  (but not inside words with underscores)
    s = s.replace(/(?<![A-Za-z0-9_])[*]([^*]+)[*](?![A-Za-z0-9_])/g, "<em>$1</em>");
    s = s.replace(/(?<![A-Za-z0-9_])_([^_]+)_(?![A-Za-z0-9_])/g, "<em>$1</em>");

    // Unordered lists: lines starting with - or *
    s = s.replace(/(^|\n)([-*]) (.+)/g, function (_, pre, bullet, content) {
      return (pre || "") + "<li>" + content + "</li>";
    });
    // Wrap consecutive <li> in <ul>
    s = s.replace(/((?:<li>.*<\\/li>\n?)+)/g, "<ul>$1</ul>");

    // Ordered lists: lines starting with 1. 2. etc
    s = s.replace(/(^|\n)([0-9]+)[.] (.+)/g, function (_, pre, num, content) {
      return (pre || "") + "<li>" + content + "</li>";
    });

    // Line breaks
    s = s.replace(/\n/g, "<br>");

    return s;
  }

  // ── Helpers ──
  function scrollToBottom() {
    messagesEl.scrollTop = messagesEl.scrollHeight;
  }

  function clearWelcome() {
    var w = messagesEl.querySelector(".welcome");
    if (w) w.remove();
  }

  function addBubble(role, text) {
    clearWelcome();
    var div = document.createElement("div");
    div.className = "msg " + role;
    var rendered = role === "agent" ? renderMarkdown(text) : escapeHtml(text);
    var now = new Date();
    var ts = now.toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" });
    div.innerHTML = rendered + '<span class="ts">' + escapeHtml(ts) + "</span>";
    messagesEl.appendChild(div);
    scrollToBottom();
    return div;
  }

  function escapeHtml(t) {
    return t.replace(/&/g,"&amp;").replace(/</g,"&lt;").replace(/>/g,"&gt;");
  }

  function showLoading() {
    var div = document.createElement("div");
    div.className = "loading";
    div.id = "loading-indicator";
    div.innerHTML = "<span></span><span></span><span></span>";
    messagesEl.appendChild(div);
    scrollToBottom();
  }

  function hideLoading() {
    var el = document.getElementById("loading-indicator");
    if (el) el.remove();
  }

  function autoResize() {
    inputEl.style.height = "auto";
    inputEl.style.height = Math.min(inputEl.scrollHeight, 120) + "px";
  }

  // ── Send ──
  async function send() {
    var text = inputEl.value.trim();
    if (!text || sending) return;

    sending = true;
    sendBtn.disabled = true;
    inputEl.value = "";
    autoResize();

    addBubble("user", text);
    showLoading();

    try {
      var body = { text: text };
      if (sessionId) body.sessionId = sessionId;

      var res = await fetch("/chat/message", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify(body),
      });

      var data = await res.json();
      hideLoading();

      if (data.sessionId) {
        sessionId = data.sessionId;
        localStorage.setItem("ss_chat_session", sessionId);
      }

      addBubble("agent", data.response || data.error || "No response.");
    } catch (err) {
      hideLoading();
      addBubble("agent", "Connection error. Please try again.");
      console.error(err);
    } finally {
      sending = false;
      sendBtn.disabled = false;
      inputEl.focus();
    }
  }

  // ── Load history on page load ──
  async function loadHistory() {
    if (!sessionId) return;
    try {
      var res = await fetch("/chat/history/" + encodeURIComponent(sessionId));
      if (!res.ok) return;
      var data = await res.json();
      if (data.messages && data.messages.length > 0) {
        clearWelcome();
        data.messages.forEach(function (m) {
          addBubble(m.role, m.text);
        });
      }
    } catch (e) {
      // Ignore — fresh session
    }
  }

  // ── Events ──
  sendBtn.addEventListener("click", send);

  inputEl.addEventListener("keydown", function (e) {
    if (e.key === "Enter" && !e.shiftKey) {
      e.preventDefault();
      send();
    }
  });

  inputEl.addEventListener("input", autoResize);

  // Boot
  loadHistory();
  inputEl.focus();
})();
</script>
</body>
</html>`;
}

// ============================================================================
// Channel entry point
// ============================================================================

/**
 * Start the WebChat channel.
 *
 * Unlike socket-based channels (Telegram, Discord, etc.) the webchat channel
 * does not maintain a persistent connection.  Instead it returns a set of
 * routes that can be mounted on the HTTP gateway.
 *
 * @param {Object}  config
 * @param {import('./session-store.js').ChannelSessionStore} [config.sessionStore]
 * @param {import('./identity.js').CustomerIdentityStore}    [config.identityStore]
 * @param {Function[]}  [config.middleware]
 * @param {string}      [config.dbPath='./store.db']
 * @param {boolean}     [config.allowApply=false]
 * @param {string}      [config.model]
 * @param {number}      [config.maxTurns=10]
 * @param {string}      [config.agent]
 * @param {boolean}     [config.verbose=false]
 * @param {Object}      [config.autonomousEngine]
 * @returns {{ getRoutes: () => Array<{method:string,path:string,handler:Function}>, shutdown: () => void }}
 */
export function startWebChatChannel(config = {}) {
  const {
    sessionStore,
    identityStore,
    middleware = [],
    dbPath = './store.db',
    allowApply = false,
    model,
    maxTurns = 10,
    agent,
    verbose = false,
    autonomousEngine,
  } = config;

  // Session manager (re-uses shared base logic, including persistent store)
  const sessionManager = createSessionManager({ store: sessionStore, channel: 'webchat' });
  const cleanupHandle = sessionManager.startCleanup();

  console.log('[WebChat] Channel initialised.');

  // ── Cached HTML ──
  const chatHTML = buildChatHTML();

  // ── Route: GET /chat ──
  async function handleGetChat() {
    return {
      status: 200,
      body: null,
      _html: chatHTML,
    };
  }

  // ── Route: POST /chat/message ──
  async function handlePostMessage({ body }) {
    const { text, sessionId: incomingSessionId } = body || {};

    if (!text || typeof text !== 'string' || text.trim().length === 0) {
      return { status: 400, body: { error: 'Missing "text" field.' } };
    }

    // Resolve or create session id
    const sessionId = incomingSessionId || crypto.randomUUID();
    const session = sessionManager.getSession(sessionId);

    let trimmed = text.trim();

    // Middleware pipeline (mirrors createMessageHandler behavior for other gateways)
    if (middleware.length > 0) {
      try {
        const ctx = {
          text: trimmed,
          senderId: sessionId,
          targetId: sessionId,
          session,
          raw: body,
          channel: 'webchat',
          metadata: {},
          blocked: false,
          blockReason: null,
        };

        await runMiddleware(middleware, ctx);

        if (ctx.blocked) {
          const response = ctx.blockReason || 'Message blocked by middleware';
          pushMessage(sessionId, 'agent', response);
          sessionManager.persistSession(sessionId, session);
          return { status: 200, body: { response, sessionId, blocked: true } };
        }

        if (typeof ctx.text === 'string') {
          trimmed = ctx.text.trim();
        }
      } catch (err) {
        console.error('[WebChat] Middleware error:', err.message);
      }
    }

    // Record user message in history
    pushMessage(sessionId, 'user', trimmed);

    if (verbose) {
      console.log(`[WebChat] ${sessionId}: ${trimmed.slice(0, 120)}`);
    }

    // 1. Check bot commands first
    try {
      const cmd = await handleBotCommand(trimmed, session, allowApply, {
        identityStore,
        channel: 'webchat',
        senderId: sessionId,
        autonomousEngine,
      });

      if (cmd.handled) {
        const response = cmd.response || '';
        pushMessage(sessionId, 'agent', response);
        sessionManager.persistSession(sessionId, session);
        return {
          status: 200,
          body: { response, sessionId },
        };
      }
    } catch (err) {
      console.error('[WebChat] Bot command error:', err.message);
    }

    // 2. Agent processing
    try {
      const result = await processWithAgent(trimmed, session, {
        dbPath,
        allowApply,
        model,
        maxTurns,
        agent,
        channel: 'webchat',
        senderId: sessionId,
        verbose,
      });

      const response = result.response || 'I processed your request but have no text response.';
      pushMessage(sessionId, 'agent', response);
      sessionManager.persistSession(sessionId, session);

      if (verbose) {
        console.log(
          `[WebChat] Replied to ${sessionId} (${response.length} chars, agent: ${result.agent})`,
        );
      }

      return {
        status: 200,
        body: { response, sessionId },
      };
    } catch (err) {
      console.error('[WebChat] Agent error:', err.message);
      const errorMsg = 'Sorry, I encountered an error processing your request. Please try again.';
      pushMessage(sessionId, 'agent', errorMsg);
      return {
        status: 200,
        body: { response: errorMsg, sessionId },
      };
    }
  }

  // ── Route: GET /chat/history/:sessionId ──
  async function handleGetHistory({ params }) {
    const sid = params.sessionId;
    if (!sid) {
      return { status: 400, body: { error: 'Missing sessionId.' } };
    }

    const messages = conversationHistory.get(sid) || [];
    return {
      status: 200,
      body: { sessionId: sid, messages },
    };
  }

  // ── Build routes ──
  function getRoutes() {
    return [
      {
        method: 'GET',
        path: '/chat',
        handler: handleGetChat,
      },
      {
        method: 'POST',
        path: '/chat/message',
        handler: handlePostMessage,
      },
      {
        method: 'GET',
        path: '/chat/history/:sessionId',
        handler: handleGetHistory,
      },
    ];
  }

  // ── Shutdown ──
  function shutdown() {
    sessionManager.stopCleanup(cleanupHandle);
    conversationHistory.clear();
    console.log('[WebChat] Channel shut down.');
  }

  return { getRoutes, shutdown };
}
