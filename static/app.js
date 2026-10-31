// Progressive enhancement only: every flow works without this file.
(function () {
  "use strict";

  // Confirmations: <form data-confirm="..."> asks before submitting.
  document.addEventListener("submit", function (ev) {
    var form = ev.target;
    if (!(form instanceof HTMLFormElement)) return;
    var msg = form.getAttribute("data-confirm");
    if (msg && !window.confirm(msg)) {
      ev.preventDefault();
      return;
    }
    // Prevent double submits (slow mobile taps on "Sortear").
    var buttons = form.querySelectorAll("button[type=submit]");
    window.setTimeout(function () {
      buttons.forEach(function (b) { b.disabled = true; });
    }, 0);
    window.setTimeout(function () {
      buttons.forEach(function (b) { b.disabled = false; });
    }, 8000);
  });

  // Verification code: submit as soon as 6 digits are typed or pasted.
  var otp = document.querySelector("input[data-autosubmit]");
  if (otp) {
    var wanted = parseInt(otp.getAttribute("data-autosubmit"), 10) || 6;
    otp.addEventListener("input", function () {
      otp.value = otp.value.replace(/\D/g, "").slice(0, wanted);
      if (otp.value.length === wanted && otp.form) {
        otp.form.requestSubmit ? otp.form.requestSubmit() : otp.form.submit();
      }
    });
  }

  // Participant edit dialog: one <dialog>, filled from the row's data attributes.
  var dialog = document.getElementById("edit-dialog");
  var editForm = document.getElementById("edit-form");
  if (dialog && editForm && typeof dialog.showModal === "function") {
    document.querySelectorAll("[data-edit-participant]").forEach(function (btn) {
      btn.addEventListener("click", function () {
        var base = window.location.pathname.replace(/\/$/, "");
        editForm.action = base + "/participantes/" + btn.getAttribute("data-id");
        editForm.elements.name.value = btn.getAttribute("data-name") || "";
        editForm.elements.email.value = btn.getAttribute("data-email") || "";
        dialog.showModal();
        editForm.elements.name.focus();
      });
    });
    dialog.querySelectorAll("[data-close-dialog]").forEach(function (b) {
      b.addEventListener("click", function () { dialog.close(); });
    });
    dialog.addEventListener("click", function (ev) {
      if (ev.target === dialog) dialog.close();
    });
  }

  // Reveal page: hide the drawn name while someone is looking over your shoulder.
  document.querySelectorAll("[data-toggle-blur]").forEach(function (btn) {
    var target = document.querySelector(btn.getAttribute("data-toggle-blur"));
    var label = btn.querySelector("[data-label-hide]");
    if (!target) return;
    btn.addEventListener("click", function () {
      var hidden = target.classList.toggle("blurred");
      if (label) label.textContent = hidden ? label.getAttribute("data-label-show") : label.getAttribute("data-label-hide");
      btn.setAttribute("aria-pressed", hidden ? "true" : "false");
    });
  });

  // Exclusion selects: never offer the same person on both sides.
  var selA = document.getElementById("x-a");
  var selB = document.getElementById("x-b");
  if (selA && selB) {
    var sync = function () {
      Array.prototype.forEach.call(selB.options, function (o) { o.disabled = o.value === selA.value; });
      if (selB.value === selA.value) {
        var first = Array.prototype.find.call(selB.options, function (o) { return !o.disabled; });
        if (first) selB.value = first.value;
      }
    };
    selA.addEventListener("change", sync);
    sync();
  }
})();
