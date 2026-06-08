// AION-MEDSAFE landing — scroll reveals + the "verify a sealed packet" moment.
(function () {
  "use strict";
  var reduce = window.matchMedia("(prefers-reduced-motion: reduce)").matches;

  // --- Scroll reveals -------------------------------------------------------
  var revealed = document.querySelectorAll(".reveal");
  if (reduce || !("IntersectionObserver" in window)) {
    revealed.forEach(function (el) { el.classList.add("in"); });
  } else {
    var io = new IntersectionObserver(function (entries) {
      entries.forEach(function (e) {
        if (e.isIntersecting) { e.target.classList.add("in"); io.unobserve(e.target); }
      });
    }, { rootMargin: "0px 0px -8% 0px", threshold: 0.1 });
    revealed.forEach(function (el) { io.observe(el); });
  }

  // --- Verify the packet ----------------------------------------------------
  var btn = document.getElementById("verify-btn");
  var stamp = document.getElementById("stamp");
  var rows = Array.prototype.slice.call(document.querySelectorAll(".guarantees [data-g]"));
  if (!btn || !rows.length) return;

  function setPending() {
    rows.forEach(function (r) {
      r.removeAttribute("data-ok");
      r.querySelector(".g-state").textContent = "pending";
    });
    stamp.classList.remove("show");
  }

  function markOk(row) {
    row.setAttribute("data-ok", "");
    row.querySelector(".g-state").textContent = "valid";
  }

  function verify() {
    if (reduce) {
      rows.forEach(markOk);
      stamp.classList.add("show");
      btn.textContent = "Verified ✓";
      return;
    }
    btn.disabled = true;
    setPending();
    btn.textContent = "Verifying…";
    var i = 0;
    (function step() {
      if (i < rows.length) {
        markOk(rows[i]);
        i += 1;
        setTimeout(step, 260);
      } else {
        stamp.classList.add("show");
        btn.textContent = "Verified ✓";
        btn.disabled = false;
      }
    })();
  }

  btn.addEventListener("click", verify);

  // Play the signature moment once when it scrolls into view (then it's replayable).
  if (reduce) {
    verify();
  } else if ("IntersectionObserver" in window) {
    var played = false;
    var pio = new IntersectionObserver(function (entries) {
      entries.forEach(function (e) {
        if (e.isIntersecting && !played) {
          played = true;
          setTimeout(verify, 450);
          pio.disconnect();
        }
      });
    }, { threshold: 0.55 });
    pio.observe(document.getElementById("packet"));
  }
})();
