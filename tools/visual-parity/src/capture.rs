//! Capture the workspace canary on both pixel backends.
//!
//! Bevy: the Bevy workspace example renders offscreen and writes its PNG and
//! node dump itself. Web: the Dioxus workspace example is built to wasm, served
//! from a local static server, and captured by headless Chrome over the
//! DevTools protocol.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use base64::Engine as _;
use serde_json::{json, Value};
use tungstenite::stream::MaybeTlsStream;
use tungstenite::{Message, WebSocket};

/// Logical capture size and device scale, shared by both backends.
pub const VIEWPORT: (u32, u32) = (1440, 900);
pub const DEVICE_SCALE: u32 = 2;

const DUMP_JS: &str = include_str!("dump.js");
const HARNESS_HTML: &str = include_str!("harness.html");

type Result<T, E = String> = std::result::Result<T, E>;

/// The workspace root this tool was built from.
pub fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn cargo() -> Command {
    let mut command = Command::new(std::env::var("CARGO").unwrap_or_else(|_| "cargo".into()));
    command.current_dir(workspace_root());
    command
}

fn run(mut command: Command, what: &str) -> Result<()> {
    let status = command
        .status()
        .map_err(|error| format!("{what}: could not start: {error}"))?;
    status
        .success()
        .then_some(())
        .ok_or_else(|| format!("{what}: exited with {status}"))
}

fn require_spec_env() -> Result<()> {
    std::env::var_os("PANEL_KIT_WORKSPACE_SPEC")
        .map(|_| ())
        .ok_or_else(|| "PANEL_KIT_WORKSPACE_SPEC is unset: run inside `nix develop`".to_owned())
}

/// Render the Bevy canary offscreen into `png` (+ `<png>.nodes.jsonl`).
pub fn capture_bevy(png: &Path) -> Result<()> {
    require_spec_env()?;
    let mut command = cargo();
    command
        .args(["run", "--quiet", "-p", "panel-kit-bevy", "--example", "workspace"])
        .env("PANEL_KIT_BEVY_SCREENSHOT", png);
    run(command, "Bevy capture")?;
    [png.to_path_buf(), png.with_extension("png.nodes.jsonl")]
        .iter()
        .find(|path| !path.exists())
        .map_or(Ok(()), |path| Err(format!("Bevy capture wrote no {}", path.display())))
}

/// Build the web canary into `site` as a static site: wasm-bindgen output
/// plus the capture harness page.
pub fn build_web(site: &Path) -> Result<()> {
    require_spec_env()?;
    let mut build = cargo();
    build.args([
        "build",
        "--quiet",
        "-p",
        "panel-kit",
        "--features",
        "web-runtime",
        "--example",
        "workspace",
        "--target",
        "wasm32-unknown-unknown",
        "--release",
    ]);
    run(build, "web canary wasm build")?;
    let target = std::env::var_os("CARGO_TARGET_DIR")
        .map_or_else(|| workspace_root().join("target"), PathBuf::from);
    let wasm = target.join("wasm32-unknown-unknown/release/examples/workspace.wasm");
    let mut bindgen = Command::new("wasm-bindgen");
    bindgen
        .args(["--target", "web", "--no-typescript", "--out-name", "app", "--out-dir"])
        .arg(site)
        .arg(&wasm);
    run(bindgen, "wasm-bindgen (from `nix develop`)")?;
    std::fs::write(site.join("index.html"), HARNESS_HTML)
        .map_err(|error| format!("writing harness page: {error}"))
}

/// Capture the built `site` in headless Chrome into `png` (+ node dump).
pub fn capture_web(site: &Path, png: &Path) -> Result<()> {
    let address = serve(site.to_path_buf())?;
    let chrome = Chrome::launch()?;
    let mut page = chrome.open_page()?;
    page.call(
        "Emulation.setDeviceMetricsOverride",
        json!({
            "width": VIEWPORT.0,
            "height": VIEWPORT.1,
            "deviceScaleFactor": DEVICE_SCALE,
            "mobile": false,
        }),
    )?;
    // Animations rest at their reduced-motion frame, as in the Bevy capture.
    page.call(
        "Emulation.setEmulatedMedia",
        json!({ "features": [{ "name": "prefers-reduced-motion", "value": "reduce" }] }),
    )?;
    page.call("Page.navigate", json!({ "url": format!("http://{address}/index.html") }))?;

    let deadline = Instant::now() + Duration::from_secs(60);
    while page.evaluate("document.querySelectorAll('.panel').length")?.as_u64() == Some(0)
        || page.evaluate("document.readyState")?.as_str() != Some("complete")
    {
        if Instant::now() > deadline {
            return Err("web canary rendered no panel within 60 s".into());
        }
        std::thread::sleep(Duration::from_millis(200));
    }
    page.evaluate("document.fonts.ready.then(() => true)")?;
    // Panel bodies hydrate behind loading gates after the chrome paints.
    std::thread::sleep(Duration::from_millis(2500));

    let shot = page.call(
        "Page.captureScreenshot",
        json!({ "format": "png", "fromSurface": true }),
    )?;
    let data = shot["data"].as_str().ok_or("screenshot returned no data")?;
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(data)
        .map_err(|error| format!("screenshot data: {error}"))?;
    std::fs::write(png, bytes).map_err(|error| format!("{}: {error}", png.display()))?;
    let dump = page.evaluate(DUMP_JS)?;
    let dump = dump.as_str().ok_or("DOM dump returned no text")?;
    let path = png.with_extension("png.nodes.jsonl");
    std::fs::write(&path, format!("{dump}\n")).map_err(|error| format!("{}: {error}", path.display()))
}

/// Serve `root` on an ephemeral loopback port for the life of the process.
fn serve(root: PathBuf) -> Result<String> {
    let listener = TcpListener::bind("127.0.0.1:0").map_err(|error| format!("static server: {error}"))?;
    let address = listener
        .local_addr()
        .map_err(|error| format!("static server: {error}"))?
        .to_string();
    std::thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            let root = root.clone();
            std::thread::spawn(move || {
                let _ = respond(stream, &root);
            });
        }
    });
    Ok(address)
}

fn respond(mut stream: TcpStream, root: &Path) -> std::io::Result<()> {
    let mut reader = BufReader::new(stream.try_clone()?);
    let mut request = String::new();
    reader.read_line(&mut request)?;
    let mut header = String::new();
    while reader.read_line(&mut header)? > 2 {
        header.clear();
    }
    let path = request.split_whitespace().nth(1).unwrap_or("/");
    let path = path.split('?').next().unwrap_or(path).trim_start_matches('/');
    let file = root.join(if path.is_empty() { "index.html" } else { path });
    let body = if path.split('/').any(|part| part == "..") {
        None
    } else {
        std::fs::read(&file).ok()
    };
    let Some(body) = body else {
        return stream.write_all(b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n");
    };
    let mime = match file.extension().and_then(|ext| ext.to_str()) {
        Some("html") => "text/html; charset=utf-8",
        Some("js") => "text/javascript",
        Some("wasm") => "application/wasm",
        Some("css") => "text/css",
        _ => "application/octet-stream",
    };
    write!(
        stream,
        "HTTP/1.1 200 OK\r\nContent-Type: {mime}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    )?;
    stream.write_all(&body)
}

/// Headless Chrome with a throwaway profile (so localStorage starts empty),
/// killed on drop.
struct Chrome {
    child: Child,
    endpoint: String,
    profile: PathBuf,
}

impl Chrome {
    fn executable() -> Result<PathBuf> {
        if let Some(path) = std::env::var_os("PANEL_KIT_CHROME") {
            return Ok(path.into());
        }
        let mac = Path::new("/Applications/Google Chrome.app/Contents/MacOS/Google Chrome");
        if mac.exists() {
            return Ok(mac.into());
        }
        let path = std::env::var_os("PATH").unwrap_or_default();
        ["google-chrome", "google-chrome-stable", "chromium", "chromium-browser"]
            .iter()
            .flat_map(|name| std::env::split_paths(&path).map(move |dir| dir.join(name)))
            .find(|candidate| candidate.exists())
            .ok_or_else(|| "no Chrome found: set PANEL_KIT_CHROME to a Chrome or Chromium binary".into())
    }

    fn launch() -> Result<Self> {
        let profile = std::env::temp_dir().join(format!("visual-parity-chrome-{}", std::process::id()));
        let mut child = Command::new(Self::executable()?)
            .args([
                "--headless=new",
                "--remote-debugging-port=0",
                "--no-first-run",
                "--no-default-browser-check",
                "--hide-scrollbars",
                "--mute-audio",
            ])
            .arg(format!("--user-data-dir={}", profile.display()))
            .arg("about:blank")
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|error| format!("starting Chrome: {error}"))?;
        let mut stderr = BufReader::new(child.stderr.take().ok_or("Chrome stderr")?);
        let mut line = String::new();
        let endpoint = loop {
            line.clear();
            if stderr.read_line(&mut line).map_err(|error| format!("Chrome stderr: {error}"))? == 0 {
                let _ = child.kill();
                return Err("Chrome exited before printing its DevTools endpoint".into());
            }
            if let Some(url) = line.trim().strip_prefix("DevTools listening on ") {
                break url.to_owned();
            }
        };
        std::thread::spawn(move || {
            let _ = std::io::copy(&mut stderr.into_inner().take(u64::MAX), &mut std::io::sink());
        });
        Ok(Self {
            child,
            endpoint,
            profile,
        })
    }

    fn open_page(&self) -> Result<Page> {
        let (socket, _) =
            tungstenite::connect(self.endpoint.as_str()).map_err(|error| format!("DevTools connect: {error}"))?;
        if let MaybeTlsStream::Plain(stream) = socket.get_ref() {
            let _ = stream.set_read_timeout(Some(Duration::from_secs(60)));
        }
        let mut page = Page {
            socket,
            next_id: 0,
            session: None,
        };
        let target = page.call("Target.createTarget", json!({ "url": "about:blank" }))?;
        let attached = page.call(
            "Target.attachToTarget",
            json!({ "targetId": target["targetId"], "flatten": true }),
        )?;
        page.session = attached["sessionId"].as_str().map(str::to_owned);
        page.session.as_ref().ok_or("DevTools attach returned no session")?;
        Ok(page)
    }
}

impl Drop for Chrome {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        let _ = std::fs::remove_dir_all(&self.profile);
    }
}

/// One DevTools session over the browser socket (flattened target session).
struct Page {
    socket: WebSocket<MaybeTlsStream<TcpStream>>,
    next_id: u64,
    session: Option<String>,
}

impl Page {
    fn call(&mut self, method: &str, params: Value) -> Result<Value> {
        self.next_id += 1;
        let id = self.next_id;
        let mut message = json!({ "id": id, "method": method, "params": params });
        if let Some(session) = &self.session {
            message["sessionId"] = json!(session);
        }
        self.socket
            .send(Message::Text(message.to_string()))
            .map_err(|error| format!("{method}: {error}"))?;
        loop {
            let reply = self.socket.read().map_err(|error| format!("{method}: {error}"))?;
            let Message::Text(text) = reply else {
                continue;
            };
            let value: Value = serde_json::from_str(&text).map_err(|error| format!("{method}: {error}"))?;
            if value["id"].as_u64() != Some(id) {
                continue;
            }
            if let Some(error) = value.get("error") {
                return Err(format!("{method}: {error}"));
            }
            return Ok(value["result"].clone());
        }
    }

    /// Evaluate `expression` in the page, awaiting a returned promise.
    fn evaluate(&mut self, expression: &str) -> Result<Value> {
        let result = self.call(
            "Runtime.evaluate",
            json!({ "expression": expression, "returnByValue": true, "awaitPromise": true }),
        )?;
        if let Some(exception) = result.get("exceptionDetails") {
            return Err(format!("page script failed: {exception}"));
        }
        Ok(result["result"]["value"].clone())
    }
}
