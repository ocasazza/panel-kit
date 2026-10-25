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
        { token: '', foreground: syntax('text'), background: syntax('surface') },
        { token: 'comment', foreground: syntax('text-muted') },
        { token: 'string', foreground: syntax('success') },
        { token: 'string.quote', foreground: syntax('success') },
        { token: 'string.escape', foreground: syntax('info') },
        { token: 'keyword', foreground: syntax('info') },
        { token: 'number', foreground: syntax('tertiary') },
        { token: 'type.identifier', foreground: syntax('tertiary') },
        { token: 'identifier', foreground: syntax('text') },
        { token: 'operator', foreground: syntax('text') },
        { token: 'delimiter', foreground: syntax('text-muted') },
        { token: 'invalid', foreground: syntax('error') },
      ],
      colors: {
        'editor.background': color('surface'),
        'editor.foreground': color('text'),
        'editorGutter.background': color('background'),
        'editor.lineHighlightBackground': color('background'),
        'editor.lineHighlightBorder': color('divider'),
        'editorLineNumber.foreground': color('text-muted'),
        'editorLineNumber.activeForeground': color('text'),
        'editorCursor.foreground': color('highlight'),
        'editor.selectionBackground': color('inverse-background'),
        'editor.selectionForeground': color('inverse-text'),
        'editor.inactiveSelectionBackground': alpha('inverse-background', '40'),
        'editor.selectionHighlightBackground': alpha('inverse-background', '22'),
        'editorWhitespace.foreground': color('border'),
        'editorIndentGuide.background1': color('divider'),
        'editorIndentGuide.activeBackground1': color('border'),
        'editorRuler.foreground': color('divider'),
        'editorBracketMatch.background': alpha('highlight', '22'),
        'editorBracketMatch.border': color('highlight'),
        'editorOverviewRuler.background': color('surface'),
        'editorOverviewRuler.border': color('divider'),
        'editorWidget.background': color('surface'),
        'editorWidget.foreground': color('text'),
        'editorWidget.border': color('border'),
        'editorHoverWidget.background': color('surface'),
        'editorHoverWidget.foreground': color('text'),
        'editorHoverWidget.border': color('border'),
        'editorHoverWidget.statusBarBackground': color('background'),
        'editorSuggestWidget.background': color('surface'),
        'editorSuggestWidget.foreground': color('text'),
        'editorSuggestWidget.border': color('border'),
        'editorSuggestWidget.selectedBackground': color('inverse-background'),
        'editorSuggestWidget.selectedForeground': color('inverse-text'),
        'editorSuggestWidget.highlightForeground': color('info'),
        'focusBorder': color('highlight'),
        'input.background': color('background'),
        'input.foreground': color('text'),
        'input.border': color('border'),
        'input.placeholderForeground': color('text-muted'),
        'inputOption.activeBackground': color('background'),
        'inputOption.activeBorder': color('highlight'),
        'inputOption.activeForeground': color('text'),
        'scrollbar.shadow': color('divider'),
        'scrollbarSlider.background': color('border'),
        'scrollbarSlider.hoverBackground': color('text-muted'),
        'scrollbarSlider.activeBackground': color('text'),
        'list.hoverBackground': color('background'),
        'list.hoverForeground': color('text'),
        'list.focusBackground': color('inverse-background'),
        'list.focusForeground': color('inverse-text'),
        'list.activeSelectionBackground': color('inverse-background'),
        'list.activeSelectionForeground': color('inverse-text'),
        'editor.findMatchBackground': color('inverse-background'),
        'editor.findMatchForeground': color('inverse-text'),
        'editor.findMatchBorder': color('highlight'),
        'editor.findMatchHighlightBackground': alpha('inverse-background', '40'),
        'editorError.foreground': color('error'),
        'editorWarning.foreground': color('tertiary'),
        'editorInfo.foreground': color('info'),
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
        e.submitSub?.dispose();
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

    // ⌘/Ctrl+Enter. Registered as a Monaco action rather than a DOM listener
    // so it fires while focus is inside any editor widget (suggestions,
    // find) and never double-fires against the editor's own key handling.
    onSubmit(id, callback) {
      const e = editor(id);
      e.submitSub?.dispose();
      e.submitSub = e.addAction({
        id: 'panel-kit.submit',
        label: 'Submit',
        keybindings: [monaco.KeyMod.CtrlCmd | monaco.KeyCode.Enter],
        run: () => callback(),
      });
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
