// Reader behaviour. Deliberately small: the layout aligns code and prose in
// one grid row, so there is no scroll-syncing to do.

(function () {
  // ---- theme toggle -------------------------------------------------------
  var KEY = "slbl-theme";
  var root = document.documentElement;
  var saved = null;
  try { saved = localStorage.getItem(KEY); } catch (e) { /* private mode */ }
  if (saved === "light" || saved === "dark") root.setAttribute("data-theme", saved);

  var btn = document.getElementById("theme");
  if (btn) {
    btn.addEventListener("click", function () {
      var systemDark = window.matchMedia("(prefers-color-scheme: dark)").matches;
      var current = root.getAttribute("data-theme") || (systemDark ? "dark" : "light");
      var next = current === "dark" ? "light" : "dark";
      root.setAttribute("data-theme", next);
      try { localStorage.setItem(KEY, next); } catch (e) { /* ignore */ }
    });
  }

  // ---- example runner -----------------------------------------------------
  // Phase 1 stub. The playground module (decision D1, wasm-bindgen) is not
  // wired up yet; until it is, say so rather than pretending to run.
  document.querySelectorAll(".run").forEach(function (button) {
    button.addEventListener("click", function () {
      var out = button.closest(".examples").querySelector(".output");
      out.hidden = false;
      out.textContent =
        "The WASM playground is not wired up yet (phase 1).\n" +
        "Run it locally:  cargo test -p " + button.dataset.example;
    });
  });
})();
