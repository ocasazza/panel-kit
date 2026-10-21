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
  const tally = () => $(".pk-dt-tally").textContent;

  const results = [];
  // `ok: false` fails the step whatever its time; `note` explains.
  const report = (label, ms, budget, extra = {}) => {
    const { ok = true, ...rest } = extra;
    results.push({ label, ms, budget, pass: ms !== null && ms <= budget && ok, ...rest });
  };

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
        const before = tally();
        if (act() === false) return resolve(null);
        const poll = () => {
          if (tally() !== before) return settle();
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
  const input = (sel, value) => {
    const el = $(sel);
    if (!el) return false;
    Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, "value").set.call(el, value);
    el.dispatchEvent(new Event("input", { bubbles: true }));
  };
  const type = (sel, value) => () => input(sel, value);
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
  const scroll = (by) => () => {
    const el = $(".pk-dt-scroll");
    if (!el) return false;
    el.scrollTop += by;
  };

  const median = (xs) => {
    const s = [...xs].sort((a, b) => a - b);
    return s.length ? s[Math.floor(s.length / 2)] : null;
  };
  const MB = 1048576;

  // The longest gap between animation frames from the click on the size
  // button through the load's last painted frame.
  const load = async () => {
    let last = performance.now();
    let worst = 0;
    let running = true;
    const tick = (now) => {
      worst = Math.max(worst, now - last);
      last = now;
      if (running) requestAnimationFrame(tick);
    };
    requestAnimationFrame(tick);
    const ms = await time("tally", click('button[data-rows="1000000"]'));
    running = false;
    report("load 1,000,000 rows", ms, 4000);
    report("load longest frame gap", Math.round(worst), 200);
    await wait(50);
    const mem = $("#mem");
    if (!mem) return report("memory", null, 0, { note: "#mem missing" });
    const heap = Number(mem.dataset.heap);
    const wasm = Number(mem.dataset.wasm);
    report("memory: table data (MB)", Math.round(heap / MB), 400, { unit: "MB", ok: heap > 0 });
    report("memory: wasm linear (MB)", Math.round(wasm / MB), 1500, { unit: "MB", ok: wasm > 0 });
  };

  // Type "s", "se", "sea", "seat" 30 ms apart; time from the last keystroke
  // to the tally showing the same rows the one-shot "seat" filter showed.
  let seat = null;
  const typing = async () => {
    const before = tally();
    let t0 = 0;
    for (const value of ["s", "se", "sea", "seat"]) {
      if (input("#work-quick", value) === false) return report("quick typing s-e-a-t", null, 0);
      t0 = performance.now();
      await wait(30);
    }
    let ms = null;
    while (performance.now() - t0 < 10000) {
      await frame();
      if (tally() === seat && seat !== before) {
        await frame();
        await frame();
        ms = Math.round(performance.now() - t0);
        break;
      }
    }
    report("quick typing s-e-a-t (from last key)", ms, 700);
  };

  // Append rows for two seconds; the demo records each append-to-paint.
  const live = async () => {
    window.__live = [];
    if (click("#live")() === false) return report("live appends", null, 0);
    await wait(2000);
    click("#live")();
    await wait(400);
    const samples = window.__live.slice();
    const mid = median(samples);
    const max = samples.length ? Math.max(...samples) : null;
    const ok = samples.length >= 10;
    const note = `${samples.length} appends of 200 rows`;
    report("live append-to-paint median", mid === null ? null : Math.round(mid), 100, { ok, note });
    report("live append-to-paint max", max === null ? null : Math.round(max), 400, { ok, note });
  };

  // Export through the demo's own handlers; it records bytes and time.
  const exported = async (label, kind, button, budget) => {
    const before = Number($("#export")?.dataset.n ?? 0);
    const ms = await time(".sizes", click(button));
    const el = $("#export");
    const ok = !!el && Number(el.dataset.n) === before + 1 && el.dataset.kind === kind;
    const bytes = ok ? Number(el.dataset.bytes) : 0;
    report(label, ms, budget, { ok: ok && bytes > 1000000, bytes });
  };

  // Detail cards at the end of the data: the last row must be mounted and
  // inside the scroller's box.
  const cards = async () => {
    const view = () => $(".pk-dt-btn[title^='rows, compact']");
    const lastVisible = () => {
      const scroller = $(".pk-dt-scroll");
      const total = Number($("#work").dataset.total);
      const rows = document.querySelectorAll("tbody tr[data-index]");
      const last = rows[rows.length - 1];
      if (!last || Number(last.dataset.index) !== total - 1) return false;
      const a = last.getBoundingClientRect();
      const b = scroller.getBoundingClientRect();
      return a.height > 0 && a.bottom <= b.bottom + 1 && a.bottom > b.top;
    };
    const ms = await time("#work", () => view().click());
    report("view: compact cards", ms, 600);
    await wait(50);
    const detail = await time("#work", () => view().click());
    report("view: detail cards", detail, 600, { ok: view().textContent === "detail" });
    await wait(50);
    const scroller = $(".pk-dt-scroll");
    const t0 = performance.now();
    let ok = false;
    for (let i = 0; i < 20 && performance.now() - t0 < 10000; i++) {
      scroller.scrollTop = scroller.scrollHeight;
      await frame();
      await frame();
      if (lastVisible()) {
        ok = true;
        break;
      }
    }
    report("detail cards: scroll to the last row", Math.round(performance.now() - t0), 3000, {
      ok,
      note: ok ? "last row mounted and visible" : "last row not mounted and visible",
    });
    view().click();
    await wait(50);
    view().click();
    await wait(300);
  };

  // [label, budget ms, root watched, action, after?], or a function that
  // reports its own steps.
  const steps = [
    load,
    ["sort title (ranked at load)", 400, "#work", click('[data-dt-sort="title"]')],
    ["sort title descending", 300, "#work", click('[data-dt-sort="title"]')],
    ["sort since", 300, "#work", click('[data-dt-sort="since"]')],
    ["shift-add item (ranked at load)", 500, "#work", click('[data-dt-sort="item"]', true)],
    ["quick filter (150 ms debounce)", 600, "tally", type("#work-quick", "seat"), () => (seat = tally())],
    ["quick filter cleared", 600, "tally", type("#work-quick", "")],
    typing,
    ["quick filter cleared (after typing)", 600, "tally", type("#work-quick", "")],
    ["quick filter, accented (équipe)", 600, "tally", type("#work-quick", "équipe")],
    ["quick filter cleared (after accented)", 600, "tally", type("#work-quick", "")],
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
    ["sort note (first: ranks)", 3000, "#work", click('[data-dt-sort="note"]')],
    ["sort note descending", 1500, "#work", click('[data-dt-sort="note"]')],
    ["sort since (restore)", 300, "#work", click('[data-dt-sort="since"]')],
    ["group by since", 3000, "#work-rows", select("since")],
    ["group by note", 3000, "#work-rows", select("note")],
    ["group by state (restore)", 300, "#work-rows", select("state")],
    () => exported("csv 1,000,000 rows", "csv", "button[title^='download']", 6000),
    cards,
    live,
    // The tally must count every row selected: a duplicate id would not be.
    async function selectAll() {
      const ms = await time("tally", click("[data-dt-check-all]"));
      const [rows, selected] = (tally().match(/\d+/g) ?? []).map(Number);
      report("select all rows", ms, 3000, { ok: rows > 0 && rows === selected, note: tally() });
    },
    ["scroll with every row selected", 300, "#work-rows", scroll(4000)],
    ["key ArrowDown with every row selected", 300, "#work", key("ArrowDown")],
    // Copy takes the selected rows, so it runs once every row is selected.
    () => exported("copy 1,000,000 rows (TSV)", "copy", "button[title^='copy']", 6000),
  ];

  localStorage.clear();
  await wait(500);
  for (const step of steps) {
    try {
      if (typeof step === "function") {
        await step();
      } else {
        const [label, budget, root, act, after] = step;
        const ms = await time(root, act);
        report(label, ms, budget);
        if (after) after();
      }
    } catch (error) {
      report(String(step[0] ?? step.name), null, 0, { note: String(error) });
    }
    await wait(50);
  }
  const pass = results.every((r) => r.pass);
  out.dataset.pass = String(pass);
  out.textContent = JSON.stringify({ pass, results }, null, 1);
})();
