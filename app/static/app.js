// Reader behaviour. Deliberately small: the layout aligns code and prose in
// one grid row, so there is no scroll-syncing to do.

(function () {
  var root = document.body.dataset.root || "./";

  // ---- theme toggle -------------------------------------------------------
  var KEY = "slbl-theme";
  var html = document.documentElement;
  try {
    var saved = localStorage.getItem(KEY);
    if (saved === "light" || saved === "dark") html.setAttribute("data-theme", saved);
  } catch (e) { /* private mode */ }

  var toggle = document.getElementById("theme");
  if (toggle) {
    toggle.addEventListener("click", function () {
      var systemDark = window.matchMedia("(prefers-color-scheme: dark)").matches;
      var current = html.getAttribute("data-theme") || (systemDark ? "dark" : "light");
      var next = current === "dark" ? "light" : "dark";
      html.setAttribute("data-theme", next);
      try { localStorage.setItem(KEY, next); } catch (e) { /* ignore */ }
    });
  }

  // ---- example playground (decision D1) -----------------------------------
  // Loaded lazily on first use, so a reader who never runs an example never
  // downloads the module.
  var pending = null;
  function playground() {
    if (!pending) {
      pending = import(root + "static/wasm/playground.js").then(function (mod) {
        return mod.default().then(function () { return mod; });
      });
    }
    return pending;
  }

  function runExample(name, out) {
    out.hidden = false;
    out.textContent = "running…";
    playground().then(
      function (mod) {
        try {
          out.textContent = mod.run(name);
        } catch (err) {
          out.textContent = "error: " + err;
        }
      },
      function () {
        // The site builds without a wasm toolchain, so an absent module is a
        // normal state, not a crash. Say what to do about it.
        out.textContent =
          "The playground module is not in this build.\n\n" +
          "Build it with:  cargo xtask wasm && cargo site\n" +
          "Or run locally: cargo test -p " + name;
      }
    );
  }

  document.querySelectorAll(".run").forEach(function (button) {
    button.addEventListener("click", function () {
      runExample(button.dataset.example, button.closest(".examples").querySelector(".output"));
    });
  });

  // Deep link: ?run=<example> runs it on load. Also how the build verifies the
  // module actually executes in a browser.
  var wanted = new URLSearchParams(location.search).get("run");
  if (wanted) {
    var button = document.querySelector('.run[data-example="' + wanted + '"]');
    if (button) {
      button.scrollIntoView({ block: "nearest" });
      runExample(wanted, button.closest(".examples").querySelector(".output"));
    }
  }
})();
