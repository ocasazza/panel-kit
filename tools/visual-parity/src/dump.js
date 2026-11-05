// Every rendered element under the workspace and dock, one JSON object per
// line (WebNode in src/dump.rs). Evaluated in the captured page.
(() => {
  const num = (v) => +(+v).toFixed(2);
  const rgb = (s) => {
    const m = s.match(/rgba?\(([^)]+)\)/);
    if (!m) return null;
    const p = m[1].split(",").map((x) => +x.trim());
    return [p[0], p[1], p[2], p.length > 3 ? p[3] : 1];
  };
  const out = [];
  for (const el of document.querySelectorAll(".ws *, .dock, .dock *")) {
    const s = getComputedStyle(el);
    if (s.display === "none" || s.visibility === "hidden") continue;
    const r = el.getBoundingClientRect();
    if (r.width === 0 && r.height === 0) continue;
    const texts = [...el.childNodes].filter((n) => n.nodeType === 3 && n.textContent.trim());
    let textRect = null;
    if (texts.length) {
      const range = document.createRange();
      range.setStartBefore(texts[0]);
      range.setEndAfter(texts[texts.length - 1]);
      const t = range.getBoundingClientRect();
      textRect = [num(t.x), num(t.y), num(t.width), num(t.height)];
    }
    const host = el.closest(".panel");
    out.push(JSON.stringify({
      x: num(r.x), y: num(r.y), w: num(r.width), h: num(r.height),
      tag: el.tagName.toLowerCase(),
      cls: el.getAttribute("class") || "",
      text: texts.map((n) => n.textContent).join(""),
      text_rect: textRect,
      panel: host ? host.querySelector(".panel-title")?.textContent ?? null : null,
      font: parseFloat(s.fontSize),
      weight: s.fontWeight,
      tt: s.textTransform,
      lh: s.lineHeight,
      color: rgb(s.color),
      bg: rgb(s.backgroundColor),
      border: rgb(s.borderTopColor),
      border_w: parseFloat(s.borderTopWidth),
    }));
  }
  return out.join("\n");
})()
