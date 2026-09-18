/* Vitro docs 本地预览交互层
 * 纯静态预览（file:// 直开），无网络请求：
 *   主题循环（自动/浅色/深色）· 侧栏与大纲开关 · 目录折叠记忆 ·
 *   标题与正文检索（正文索引惰性加载 assets/search-index.js）· 大纲滚动跟随 · 外链处理
 */
(function () {
  "use strict";

  var html = document.documentElement;
  var body = document.body;
  var STORE = {
    theme: "vitro-docs-theme",
    nav: "vitro-docs-nav",
    outline: "vitro-docs-outline",
    groups: "vitro-docs-groups"
  };

  function ls(key, value) {
    try {
      if (value === undefined) return localStorage.getItem(key);
      if (value === null) localStorage.removeItem(key);
      else localStorage.setItem(key, value);
    } catch (e) {
      /* file:// 下 localStorage 可能被禁用，忽略 */
    }
    return null;
  }

  /* ------------------------------------------------------------ 主题 */

  var media = window.matchMedia ? window.matchMedia("(prefers-color-scheme: dark)") : null;

  function applyTheme(mode) {
    var resolved = mode;
    if (mode === "auto") resolved = media && media.matches ? "dark" : "light";
    html.setAttribute("data-color-mode", resolved);
    html.setAttribute("data-color-mode-pref", mode);
  }

  var themeMode = ls(STORE.theme) || "auto";
  applyTheme(themeMode);
  if (media && media.addEventListener) {
    media.addEventListener("change", function () {
      if ((ls(STORE.theme) || "auto") === "auto") applyTheme("auto");
    });
  }
  var themeBtn = document.getElementById("theme-toggle");
  if (themeBtn) {
    themeBtn.addEventListener("click", function () {
      var order = ["auto", "light", "dark"];
      var next = order[(order.indexOf(themeMode) + 1) % order.length];
      themeMode = next;
      ls(STORE.theme, next);
      applyTheme(next);
      themeBtn.title = "主题：" + (next === "auto" ? "自动（跟随系统）" : next === "light" ? "浅色" : "深色");
    });
  }

  /* ---------------------------------------------------- 侧栏 / 大纲开关 */

  function toggleClass(key, cls, attr) {
    var stored = ls(key);
    if (stored === "off") body.classList.add(cls);
    var btn = document.getElementById(attr);
    if (!btn) return;
    btn.addEventListener("click", function () {
      body.classList.toggle(cls);
      ls(key, body.classList.contains(cls) ? "off" : "on");
    });
  }
  toggleClass(STORE.nav, "nav-off", "nav-toggle");
  toggleClass(STORE.outline, "outline-off", "outline-toggle");

  /* -------------------------------------------------------- 折叠状态记忆 */

  var groups = {};
  try {
    groups = JSON.parse(ls(STORE.groups) || "{}");
  } catch (e) {
    groups = {};
  }
  var detailsList = document.querySelectorAll(".sidebar details[data-group]");
  Array.prototype.forEach.call(detailsList, function (d) {
    var key = d.getAttribute("data-group");
    if (Object.prototype.hasOwnProperty.call(groups, key)) d.open = !!groups[key];
    d.addEventListener("toggle", function () {
      groups[key] = d.open;
      ls(STORE.groups, JSON.stringify(groups));
    });
  });

  /* ----------------------------------------------------------- 外链处理 */

  Array.prototype.forEach.call(document.querySelectorAll(".markdown-body a[href]"), function (a) {
    var href = a.getAttribute("href") || "";
    if (/^https?:\/\//i.test(href)) {
      a.setAttribute("target", "_blank");
      a.setAttribute("rel", "noopener noreferrer");
      if (/^https:\/\/github\.com\//i.test(href)) a.classList.add("repo-link");
    } else if (/\.(md|rs|go|c|cpp|h|py|json|yaml|toml|txt|sh)(#|$)/i.test(href)) {
      a.classList.add("repo-link");
    }
  });

  /* --------------------------------------------------------------- 检索 */

  var input = document.getElementById("search");
  var hits = document.getElementById("search-hits");
  var index = null;
  var indexLoading = false;
  var indexWaiters = [];

  function assetsBase() {
    var link = document.querySelector('link[rel="stylesheet"]');
    var href = link ? link.getAttribute("href") || "" : "assets/style.css";
    return href.replace(/style\.css.*$/, "");
  }

  function loadIndex(cb) {
    if (index) return cb(index);
    indexWaiters.push(cb);
    if (indexLoading) return;
    indexLoading = true;
    var s = document.createElement("script");
    s.src = assetsBase() + "search-index.js";
    s.onload = function () {
      index = window.__VITRO_DOCS_INDEX || [];
      indexWaiters.forEach(function (fn) {
        fn(index);
      });
      indexWaiters = [];
    };
    s.onerror = function () {
      index = [];
      indexWaiters.forEach(function (fn) {
        fn(index);
      });
      indexWaiters = [];
    };
    document.head.appendChild(s);
  }

  function esc(s) {
    return String(s).replace(/[&<>"]/g, function (c) {
      return { "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;" }[c];
    });
  }

  function mark(text, q) {
    var i = text.toLowerCase().indexOf(q);
    if (i < 0) return esc(text);
    return (
      esc(text.slice(0, i)) + "<mark>" + esc(text.slice(i, i + q.length)) + "</mark>" + esc(text.slice(i + q.length))
    );
  }

  function snippet(text, q) {
    var i = text.toLowerCase().indexOf(q);
    if (i < 0) return "";
    var start = Math.max(0, i - 36);
    var end = Math.min(text.length, i + q.length + 56);
    return (start > 0 ? "…" : "") + text.slice(start, end) + (end < text.length ? "…" : "");
  }

  function navFilter(q) {
    var count = 0;
    Array.prototype.forEach.call(document.querySelectorAll(".sidebar .nav-link"), function (a) {
      var hit = !q || a.textContent.toLowerCase().indexOf(q) >= 0;
      a.parentNode.style.display = hit ? "" : "none";
      if (hit) count++;
    });
    Array.prototype.forEach.call(document.querySelectorAll(".sidebar .nav-dir"), function (d) {
      var visible = d.querySelectorAll(".nav-link:not([style*='display: none'])").length;
      var ownHit = (d.querySelector("summary .nav-dir-a") || {}).textContent || "";
      var show = !q || visible > 0 || ownHit.toLowerCase().indexOf(q) >= 0;
      d.style.display = show ? "" : "none";
      if (q && show) d.querySelector("details").open = true;
    });
    return count;
  }

  function renderResults(q) {
    if (!q) {
      hits.hidden = true;
      hits.innerHTML = "";
      navFilter("");
      return;
    }
    var navCount = navFilter(q);
    loadIndex(function (idx) {
      var parts = [];
      var used = 0;
      idx.forEach(function (d) {
        if (used >= 40) return;
        var rows = [];
        (d.h || []).forEach(function (h) {
          if (used >= 40) return;
          if (h.t && h.t.toLowerCase().indexOf(q) >= 0) {
            rows.push(
              '<a href="' + esc(d.p) + "#" + encodeURIComponent(h.i) + '"><div>' + mark(h.t, q) +
              '</div><div class="hit-path">' + esc(d.s) + "</div></a>"
            );
            used++;
          }
        });
        if (used < 40 && d.x && d.x.toLowerCase().indexOf(q) >= 0) {
          rows.push(
            '<a href="' + esc(d.p) + "?q=" + encodeURIComponent(q) + '"><div class="hit-snip">' +
            esc(snippet(d.x, q)) + '</div><div class="hit-path">' + esc(d.s) + "</div></a>"
          );
          used++;
        }
        if (rows.length) parts.push('<div class="hit-group">' + esc(d.s) + "</div>" + rows.join(""));
      });
      var head = '<div class="hit-group">目录命中 ' + navCount + " 项 · 正文命中 " + used + " 项</div>";
      hits.innerHTML = head + (parts.length ? parts.join("") : '<div class="hit-empty">没有匹配</div>');
      hits.hidden = false;
    });
  }

  if (input && hits) {
    var timer = null;
    input.addEventListener("input", function () {
      var q = input.value.trim().toLowerCase();
      if (timer) clearTimeout(timer);
      timer = setTimeout(function () {
        renderResults(q);
      }, 120);
    });
    input.addEventListener("keydown", function (e) {
      if (e.key === "Escape") {
        input.value = "";
        renderResults("");
      }
    });
    document.addEventListener("keydown", function (e) {
      if (e.key === "/" && document.activeElement !== input) {
        e.preventDefault();
        input.focus();
        input.select();
      }
    });
  }

  /* --------------------------------------------- 页内命中高亮（?q= 参数） */

  function highlightQuery() {
    var q = "";
    try {
      q = new URLSearchParams(window.location.search).get("q") || "";
    } catch (e) {
      q = "";
    }
    q = q.trim();
    if (!q) return;
    var doc = document.getElementById("doc");
    if (!doc) return;
    var lower = q.toLowerCase();
    var walker = document.createTreeWalker(doc, NodeFilter.SHOW_TEXT, null);
    var nodes = [];
    while (walker.nextNode()) {
      var t = walker.currentNode;
      if (t.nodeValue && t.nodeValue.toLowerCase().indexOf(lower) >= 0) nodes.push(t);
    }
    var first = null;
    nodes.forEach(function (node) {
      var text = node.nodeValue;
      var frag = document.createDocumentFragment();
      var pos = 0;
      var idx;
      while ((idx = text.toLowerCase().indexOf(lower, pos)) >= 0) {
        if (idx > pos) frag.appendChild(document.createTextNode(text.slice(pos, idx)));
        var m = document.createElement("mark");
        m.className = "search-hit";
        m.textContent = text.slice(idx, idx + q.length);
        frag.appendChild(m);
        if (!first) first = m;
        pos = idx + q.length;
      }
      if (pos < text.length) frag.appendChild(document.createTextNode(text.slice(pos)));
      node.parentNode.replaceChild(frag, node);
    });
    if (first && first.scrollIntoView) {
      first.scrollIntoView({ block: "center" });
      if (history.replaceState) history.replaceState(null, "", window.location.pathname);
    }
  }
  highlightQuery();

  /* --------------------------------------------------------- 大纲滚动跟随 */

  var outlineLinks = Array.prototype.slice.call(document.querySelectorAll(".outline a"));
  if (outlineLinks.length && "IntersectionObserver" in window) {
    var byId = {};
    outlineLinks.forEach(function (a) {
      byId[a.getAttribute("href").slice(1)] = a;
    });
    var current = null;
    var io = new IntersectionObserver(
      function (entries) {
        entries.forEach(function (en) {
          if (!en.isIntersecting) return;
          var link = byId[en.target.id];
          if (!link || link === current) return;
          if (current) current.classList.remove("is-current");
          current = link;
          current.classList.add("is-current");
        });
      },
      { rootMargin: "-56px 0px -70% 0px", threshold: 0 }
    );
    Object.keys(byId).forEach(function (id) {
      var h = document.getElementById(id);
      if (h) io.observe(h);
    });
  }
})();
