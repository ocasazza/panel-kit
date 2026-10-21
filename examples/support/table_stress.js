// The table demo's stress run (`?stress`): load a million rows, drive every
// interaction through real DOM events, and time each from the event to the
// second painted frame after the DOM changes. Each step has a budget; the
// result lands in #stress as JSON with data-pass, for any browser or driver to
// read. Budgets are about three times the timings measured in Chromium on an
// M-series laptop.
(async () => {
  if (!new URLSearchParams(location.search).has("stress")) return;
  const out = document.getElementById("stress");
  const frame = () => new Promise((r) => requestAnimationFrame(() => r()));
  const wait = (ms) => new Promise((r) => setTimeout(r, ms));
  const $ = (sel) => document.querySelector(sel);

  // Time `act` until `root` first changes, plus two frames. A `"tally"` root
  // instead waits for the row count's text to change: re-rendering replaces
  // that text node, so an observer on it would never fire.
  const time = (root, act) =>
    new Promise((resolve) => {
      const t0 = performance.now();
      const settle = async () => {
        await frame();
        await frame();
        resolve(Math.round(performance.now() - t0));
      };
      if (root === "tally") {
        const before = $(".pk-dt-tally").textContent;
        if (act() === false) return resolve(null);
        const poll = () => {
          if ($(".pk-dt-tally").textContent !== before) return settle();
          if (performance.now() - t0 > 10000) return resolve(null);
          requestAnimationFrame(poll);
        };
        return requestAnimationFrame(poll);
      }
      let seen = false;
      const obs = new MutationObserver(() => {
        if (seen) return;
        seen = true;
        obs.disconnect();
        settle();
      });
      obs.observe($(root), { subtree: true, childList: true, characterData: true, attributes: true });
      if (act() === false) {
        obs.disconnect();
        return resolve(null);
      }
      setTimeout(() => {
        if (!seen) {
          obs.disconnect();
          resolve(null);
        }
      }, 10000);
    });

  const click = (sel, shift = false) => () => {
    const el = $(sel);
    if (!el) return false;
    el.dispatchEvent(new MouseEvent("click", { bubbles: true, cancelable: true, shiftKey: shift }));
  };
  const type = (sel, value) => () => {
    const el = $(sel);
    if (!el) return false;
    Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, "value").set.call(el, value);
    el.dispatchEvent(new Event("input", { bubbles: true }));
  };
  const key = (name) => () => {
    const el = $("tbody tr[data-id]");
    if (!el) return false;
    el.focus();
    el.dispatchEvent(new KeyboardEvent("keydown", { key: name, bubbles: true, cancelable: true }));
  };
  const select = (value) => () => {
    const el = $(".pk-dt-group select");
    el.value = value;
    el.dispatchEvent(new Event("change", { bubbles: true }));
  };

  // [label, budget ms, root watched, action]
  const steps = [
    ["load 1,000,000 rows", 1500, "tally", click('button[data-rows="1000000"]')],
    ["sort title (first: ranks)", 400, "#work", click('[data-dt-sort="title"]')],
    ["sort title descending", 300, "#work", click('[data-dt-sort="title"]')],
    ["sort since", 300, "#work", click('[data-dt-sort="since"]')],
    ["shift-add item (first: ranks)", 500, "#work", click('[data-dt-sort="item"]', true)],
    ["quick filter (150 ms debounce)", 600, "tally", type("#work-quick", "seat")],
    ["quick filter cleared", 600, "tally", type("#work-quick", "")],
    ["tag segment on", 200, "tally", click("button[data-dt-tag]")],
    ["tag segment off", 300, "tally", click("button[data-dt-tag].on")],
    ["fold a group", 200, "#work-rows", click("tr[data-group] .pk-dt-fold")],
    ["unfold it", 200, "#work-rows", click("tr[data-group] .pk-dt-fold")],
    ["select a row", 150, "#work", click("tbody tr[data-id]")],
    ["key ArrowDown", 150, "#work", key("ArrowDown")],
    ["key End", 150, "#work", key("End")],
    ["key Home", 150, "#work", key("Home")],
    ["ungroup", 300, "#work-rows", select("")],
    ["group by state", 300, "#work-rows", select("state")],
  ];

  localStorage.clear();
  await wait(500);
  const results = [];
  for (const [label, budget, root, act] of steps) {
    const ms = await time(root, act);
    results.push({ label, ms, budget, pass: ms !== null && ms <= budget });
    await wait(50);
  }
  const pass = results.every((r) => r.pass);
  out.dataset.pass = String(pass);
  out.textContent = JSON.stringify({ pass, results }, null, 1);
})();
