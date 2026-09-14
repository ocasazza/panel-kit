// panel-kit ↔ monaco-editor integration glue. panel_kit::editor injects
// this file inline (include_str!) and then talks to it exclusively through
// the typed wasm-bindgen externs declared on `globalThis.__panelKitMonaco`.
// The vendored ESM bundle itself (monaco.esm.js) is pulled in with a dynamic
// import from the consumer-served assets directory, so nothing here needs a
// bundler and the same files work under trunk and Tauri. Rust supplies the
// canonical palette and font stack; this shim maps those values onto Monaco
// and bridges DOM/JS APIs wasm-bindgen cannot express (dynamic import, Worker
// construction, Monaco's JS object graph).
(() => {
  if (globalThis.__panelKitMonaco) return;

  let monaco = null;
  let loadPromise = null;
  const editors = new Map();
  let nextId = 1;

  function load(base) {
    if (!loadPromise) {
      const root = base.replace(/\/+$/, '');
      const css = document.createElement('link');
      css.rel = 'stylesheet';
      css.href = `${root}/monaco.esm.css`;
      document.head.appendChild(css);
      // Module worker: the esbuild bundle ends in `export{…}` (ESM), which a
      // classic worker rejects with "Unexpected token 'export'".
      globalThis.MonacoEnvironment = {
        getWorker: () => new Worker(`${root}/editor.worker.js`, { type: 'module' }),
      };
      loadPromise = import(new URL(`${root}/monaco.esm.js`, document.baseURI).href).then((m) => {
        monaco = m;
      });
    }
    return loadPromise;
  }

  function editor(id) {
    const e = editors.get(id);
    if (!e) throw new Error(`panel-kit: unknown monaco editor ${id}`);
    return e;
  }

  function applyConsoleTypography(el, fontFamily) {
    // Monaco can portal menus and widgets outside the editor host. Keep the
    // canonical stack available at document scope, but apply it only to
    // Monaco chrome and preserve the codicon glyph font.
    document.documentElement.style.setProperty('--panel-kit-editor-mono', fontFamily);
    el.style.setProperty('--panel-kit-editor-mono', fontFamily);
    if (document.getElementById('panel-kit-monaco-typography')) return;

    const style = document.createElement('style');
    style.id = 'panel-kit-monaco-typography';
    style.textContent = `
      .pk-monaco .monaco-editor,
      .pk-monaco .monaco-editor *:not(.codicon),
      .monaco-menu,
      .monaco-menu *:not(.codicon),
      .monaco-hover,
      .monaco-hover *:not(.codicon),
      .monaco-list,
      .monaco-list *:not(.codicon),
      .monaco-inputbox,
      .monaco-inputbox *:not(.codicon),
      .context-view.monaco-component,
      .context-view.monaco-component *:not(.codicon) {
        font-family: var(--panel-kit-editor-mono) !important;
      }
    `;
    document.head.appendChild(style);
  }

  function surfaceAwareOptions(el, options) {
    applyConsoleTypography(el, options.fontFamily);

    // Rust's SurfaceProfile exposes pointer capability through --hit-min.
    // Read it directly: viewport tier is not a proxy for pointer precision.
    const hitMin = Number.parseFloat(getComputedStyle(el).getPropertyValue('--hit-min'));
    if (!Number.isFinite(hitMin) || hitMin <= 0) return options;

    return {
      ...options,
      scrollbar: {
        ...(options.scrollbar ?? {}),
        verticalScrollbarSize: hitMin,
        verticalSliderSize: hitMin,
        horizontalScrollbarSize: hitMin,
        horizontalSliderSize: hitMin,
      },
    };
  }

  function panelKitTheme(tokens) {
    const color = (name) => {
      const value = tokens[name];
      if (typeof value !== 'string') {
        throw new Error(`panel-kit: Monaco theme is missing token "${name}"`);
      }
      return value;
    };
    const syntax = (name) => color(name).replace(/^#/, '');
    const alpha = (name, opacity) => `${color(name)}${opacity}`;

    return {
      // Monaco requires a base family, but inheritance is disabled so its
      // built-in vs-dark palette never leaks into panel-kit token rules.
      base: 'vs-dark',
      inherit: false,
      rules: [
        { token: '', foreground: syntax('fg'), background: syntax('panel') },
        { token: 'comment', foreground: syntax('dim') },
        { token: 'string', foreground: syntax('green') },
        { token: 'string.quote', foreground: syntax('green') },
        { token: 'string.escape', foreground: syntax('badge-info') },
        { token: 'keyword', foreground: syntax('badge-info') },
        { token: 'number', foreground: syntax('yellow') },
        { token: 'type.identifier', foreground: syntax('yellow') },
        { token: 'identifier', foreground: syntax('fg') },
        { token: 'operator', foreground: syntax('fg') },
        { token: 'delimiter', foreground: syntax('dim') },
        { token: 'invalid', foreground: syntax('red') },
      ],
      colors: {
        'editor.background': color('panel'),
        'editor.foreground': color('fg'),
        'editorGutter.background': color('bg'),
        'editor.lineHighlightBackground': color('bg'),
        'editor.lineHighlightBorder': color('line'),
        'editorLineNumber.foreground': color('dim'),
        'editorLineNumber.activeForeground': color('fg'),
        'editorCursor.foreground': color('accent'),
        'editor.selectionBackground': color('inv-bg'),
        'editor.selectionForeground': color('inv-fg'),
        'editor.inactiveSelectionBackground': alpha('inv-bg', '40'),
        'editor.selectionHighlightBackground': alpha('inv-bg', '22'),
        'editorWhitespace.foreground': color('line2'),
        'editorIndentGuide.background1': color('line'),
        'editorIndentGuide.activeBackground1': color('line2'),
        'editorRuler.foreground': color('line'),
        'editorBracketMatch.background': alpha('accent', '22'),
        'editorBracketMatch.border': color('accent'),
        'editorOverviewRuler.background': color('panel'),
        'editorOverviewRuler.border': color('line'),
        'editorWidget.background': color('panel'),
        'editorWidget.foreground': color('fg'),
        'editorWidget.border': color('line2'),
        'editorHoverWidget.background': color('panel'),
        'editorHoverWidget.foreground': color('fg'),
        'editorHoverWidget.border': color('line2'),
        'editorHoverWidget.statusBarBackground': color('bg'),
        'editorSuggestWidget.background': color('panel'),
        'editorSuggestWidget.foreground': color('fg'),
        'editorSuggestWidget.border': color('line2'),
        'editorSuggestWidget.selectedBackground': color('inv-bg'),
        'editorSuggestWidget.selectedForeground': color('inv-fg'),
        'editorSuggestWidget.highlightForeground': color('badge-info'),
        'focusBorder': color('accent'),
        'input.background': color('bg'),
        'input.foreground': color('fg'),
        'input.border': color('line2'),
        'input.placeholderForeground': color('dim'),
        'inputOption.activeBackground': color('bg'),
        'inputOption.activeBorder': color('accent'),
        'inputOption.activeForeground': color('fg'),
        'scrollbar.shadow': color('line'),
        'scrollbarSlider.background': color('line2'),
        'scrollbarSlider.hoverBackground': color('dim'),
        'scrollbarSlider.activeBackground': color('fg'),
        'list.hoverBackground': color('bg'),
        'list.hoverForeground': color('fg'),
        'list.focusBackground': color('inv-bg'),
        'list.focusForeground': color('inv-fg'),
        'list.activeSelectionBackground': color('inv-bg'),
        'list.activeSelectionForeground': color('inv-fg'),
        'editor.findMatchBackground': color('inv-bg'),
        'editor.findMatchForeground': color('inv-fg'),
        'editor.findMatchBorder': color('accent'),
        'editor.findMatchHighlightBackground': alpha('inv-bg', '40'),
        'editorError.foreground': color('red'),
        'editorWarning.foreground': color('yellow'),
        'editorInfo.foreground': color('badge-info'),
      },
    };
  }

  globalThis.__panelKitMonaco = {
    load,

    create(el, options) {
      const id = nextId++;
      const configured = surfaceAwareOptions(el, options);
      editors.set(id, monaco.editor.create(el, configured));
      return id;
    },

    dispose(id) {
      const e = editors.get(id);
      if (e) {
        e.changeSub?.dispose();
        e.dispose();
        e.getModel()?.dispose();
        editors.delete(id);
      }
    },

    // Plain replace (resets the undo stack) — imperative EditorHandle writes.
    setValue(id, value) {
      const e = editor(id);
      if (e.getValue() !== value) e.setValue(value);
    },

    // Full-model edit that keeps the undo stack and restores the cursor /
    // selection — external Signal<String> writes into a bound editor.
    setValueKeepCursor(id, value) {
      const e = editor(id);
      const model = e.getModel();
      if (!model || e.getValue() === value) return;
      const selections = e.getSelections();
      e.pushUndoStop();
      e.executeEdits('panel-kit', [{ range: model.getFullModelRange(), text: value }]);
      e.pushUndoStop();
      if (selections) e.setSelections(selections);
    },

    getValue(id) {
      return editor(id).getValue();
    },

    setLanguage(id, language) {
      const model = editor(id).getModel();
      if (model) monaco.editor.setModelLanguage(model, language);
    },

    setReadOnly(id, readOnly) {
      editor(id).updateOptions({ readOnly });
    },

    layout(id) {
      editor(id).layout();
    },

    setTheme(name) {
      monaco.editor.setTheme(name);
    },

    onChange(id, callback) {
      const e = editor(id);
      e.changeSub = e.onDidChangeModelContent(() => callback(e.getValue()));
    },

    registerLanguage(id, monarch, configuration) {
      if (!monaco.languages.getLanguages().some((l) => l.id === id)) {
        monaco.languages.register({ id });
      }
      monaco.languages.setMonarchTokensProvider(id, monarch);
      if (configuration) monaco.languages.setLanguageConfiguration(id, configuration);
    },

    defineTheme(name, tokens) {
      monaco.editor.defineTheme(name, panelKitTheme(tokens));
    },
  };
})();
