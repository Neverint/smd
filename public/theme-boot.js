// Pick the theme before first paint to avoid a white flash in dark mode.
// A separate file (not inline) so the app's Content Security Policy allows it.
(function () {
  try {
    var s = JSON.parse(localStorage.getItem("smd.settings") || "{}").state || {};
    var dark =
      s.theme === "dark" ||
      ((!s.theme || s.theme === "system") && matchMedia("(prefers-color-scheme: dark)").matches);
    document.documentElement.dataset.theme = dark ? "dark" : "light";
  } catch (e) {
    document.documentElement.dataset.theme = "light";
  }
})();
