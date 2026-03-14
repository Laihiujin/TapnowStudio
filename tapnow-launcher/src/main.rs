use anyhow::{Context, Result, bail};
use std::env;
use std::fs::{self, File};
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream, ToSocketAddrs};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

const DEFAULT_TAPNOW_STUDIO_DIR: &str = r"D:\Siuyechu\TapnowStudio\Tapnow-Studio-PP";
const DEFAULT_JIMENG_API_DIR: &str = r"D:\Siuyechu\TapnowStudio\jimeng-api";
const DEFAULT_FRONTEND_PORT: u16 = 8080;
const DEFAULT_API_PORT: u16 = 5100;
const DEFAULT_LOCALSERVER_PORT: u16 = 9527;
const HEALTH_TIMEOUT_SECS: u64 = 90;

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x08000000;
#[cfg(windows)]
const CREATE_NEW_PROCESS_GROUP: u32 = 0x00000200;
#[cfg(windows)]
const DETACHED_PROCESS: u32 = 0x00000008;

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() >= 2 && args[1] == "--serve-frontend" {
        if let Err(err) = run_frontend_server_mode(&args) {
            eprintln!("[ERROR] frontend server mode failed: {err:#}");
            std::process::exit(1);
        }
        return;
    }

    if let Err(err) = run_launcher() {
        eprintln!("\n[ERROR] {err:#}");
        std::process::exit(1);
    }
}

fn run_frontend_server_mode(args: &[String]) -> Result<()> {
    if args.len() < 4 {
        bail!("usage: --serve-frontend <dist_dir> <port>");
    }
    let dist_dir = PathBuf::from(&args[2]);
    let port = args[3]
        .parse::<u16>()
        .with_context(|| format!("invalid frontend port: {}", args[3]))?;
    serve_frontend(&dist_dir, port)
}

fn run_launcher() -> Result<()> {
    let bundled_tapnow = bundled_runtime_dir("Tapnow-Studio-PP");
    let bundled_jimeng = bundled_runtime_dir("jimeng-api");

    let tapnow_dir = env::var("TAPNOW_STUDIO_DIR")
        .map(PathBuf::from)
        .ok()
        .or(bundled_tapnow)
        .unwrap_or_else(|| PathBuf::from(DEFAULT_TAPNOW_STUDIO_DIR));
    let jimeng_dir = env::var("JIMENG_API_DIR")
        .map(PathBuf::from)
        .ok()
        .or(bundled_jimeng)
        .unwrap_or_else(|| PathBuf::from(DEFAULT_JIMENG_API_DIR));

    let frontend_port = env_port("TAPNOW_FRONTEND_PORT", DEFAULT_FRONTEND_PORT);
    let api_port = env_port("JIMENG_API_PORT", DEFAULT_API_PORT);
    let localserver_port = env_port("TAPNOW_LOCALSERVER_PORT", DEFAULT_LOCALSERVER_PORT);
    let enable_localserver = env_bool("TAPNOW_ENABLE_LOCALSERVER", true);
    let localserver_script = env::var("TAPNOW_LOCALSERVER_SCRIPT")
        .map(PathBuf::from)
        .unwrap_or_else(|_| tapnow_dir.join("localserver").join("tapnow-server-full.py"));
    let log_dir = ensure_log_dir()?;

    println!("[Tapnow Launcher] starting...");
    println!("[Info] TAPNOW_STUDIO_DIR = {}", tapnow_dir.display());
    println!("[Info] JIMENG_API_DIR = {}", jimeng_dir.display());
    println!("[Info] Frontend Port = {frontend_port}");
    println!("[Info] Jimeng API Port = {api_port}");
    println!("[Info] LocalServer Port = {localserver_port}");

    ensure_path(&tapnow_dir, "Tapnow Studio directory not found")?;
    ensure_path(&jimeng_dir, "jimeng-api directory not found")?;
    ensure_file(
        tapnow_dir.join("package.json"),
        "Tapnow Studio package.json not found",
    )?;
    ensure_file(
        jimeng_dir.join("package.json"),
        "jimeng-api package.json not found",
    )?;

    ensure_tapnow_dist_ready(&tapnow_dir)?;
    ensure_jimeng_dist_ready(&jimeng_dir)?;

    let node_bin = resolve_node_command().context(
        "Node runtime not found. Installer should include runtime\\bin\\node.exe, or install Node.js.",
    )?;
    println!("[OK] Node command: {}", node_bin);

    if is_api_ready(api_port) {
        println!("[OK] Jimeng API already running: http://127.0.0.1:{api_port}");
    } else {
        println!("[Run] starting Jimeng API...");
        spawn_background_command(
            &node_bin,
            &["dist/index.js"],
            &jimeng_dir,
            log_dir.join("jimeng-api.log"),
        )?;
        wait_for(
            "Jimeng API",
            Duration::from_secs(HEALTH_TIMEOUT_SECS),
            || is_api_ready(api_port),
        )?;
    }

    if enable_localserver {
        if let Err(err) = start_localserver(&localserver_script, localserver_port, &log_dir) {
            println!("[Warn] localserver startup failed: {err}");
        }
    } else {
        println!("[Skip] localserver auto-start disabled by TAPNOW_ENABLE_LOCALSERVER=0");
    }

    let dist_dir = tapnow_dir.join("dist");
    if is_frontend_ready(frontend_port) {
        println!("[OK] Tapnow frontend already running: http://127.0.0.1:{frontend_port}");
    } else {
        println!("[Run] starting embedded frontend server...");
        let exe = env::current_exe().context("unable to get current exe path")?;
        let dist = dist_dir.to_string_lossy().to_string();
        let port = frontend_port.to_string();
        let args = ["--serve-frontend", dist.as_str(), port.as_str()];
        spawn_background_command(
            &exe.to_string_lossy(),
            &args,
            &tapnow_dir,
            log_dir.join("tapnow-web.log"),
        )?;
        wait_for(
            "Tapnow frontend",
            Duration::from_secs(HEALTH_TIMEOUT_SECS),
            || is_frontend_ready(frontend_port),
        )?;
    }

    open_browser_in_new_window(frontend_port)?;
    println!("[DONE] opened Tapnow: http://127.0.0.1:{frontend_port}");
    println!(
        "[Tip] logs directory: {}",
        log_dir.to_string_lossy().replace('\\', "/")
    );
    Ok(())
}

fn ensure_tapnow_dist_ready(tapnow_dir: &Path) -> Result<()> {
    let dist_index = tapnow_dir.join("dist").join("index.html");
    if dist_index.is_file() {
        return Ok(());
    }

    let npm_bin = resolve_command(&["npm", "npm.cmd"]).context(
        "Tapnow dist/index.html missing and npm is unavailable. Rebuild setup package on builder machine.",
    )?;
    println!("[Run] dist/index.html missing, building frontend with vite...");
    run_npm_command(&npm_bin, tapnow_dir, &["exec", "vite", "build"])?;
    if !dist_index.is_file() {
        bail!("frontend build completed but dist/index.html is still missing");
    }
    Ok(())
}

fn ensure_jimeng_dist_ready(jimeng_dir: &Path) -> Result<()> {
    let dist_entry = jimeng_dir.join("dist").join("index.js");
    if dist_entry.is_file() {
        return Ok(());
    }

    let npm_bin = resolve_command(&["npm", "npm.cmd"]).context(
        "jimeng-api dist/index.js missing and npm is unavailable. Rebuild setup package on builder machine.",
    )?;
    println!("[Run] jimeng dist missing, building jimeng-api...");
    run_npm_command(&npm_bin, jimeng_dir, &["run", "build"])?;
    if !dist_entry.is_file() {
        bail!("jimeng-api build completed but dist/index.js is still missing");
    }
    Ok(())
}

fn resolve_node_command() -> Option<String> {
    if let Some(node) = bundled_runtime_file("bin").map(|d| d.join("node.exe")) {
        if node.is_file() {
            return Some(node.to_string_lossy().to_string());
        }
    }
    resolve_command(&["node", "node.exe"])
}

fn resolve_python_command() -> Option<String> {
    if let Some(python) = bundled_runtime_file("bin").map(|d| d.join("python").join("python.exe"))
    {
        if python.is_file() {
            return Some(python.to_string_lossy().to_string());
        }
    }
    resolve_command(&["python", "python.exe", "py", "py.exe"])
}

fn start_localserver(
    localserver_script: &Path,
    localserver_port: u16,
    log_dir: &Path,
) -> Result<()> {
    if is_localserver_ready(localserver_port) {
        println!("[OK] localserver already running: http://127.0.0.1:{localserver_port}");
        return Ok(());
    }

    if !localserver_script.is_file() {
        bail!(
            "localserver script not found at {}",
            localserver_script.display()
        );
    }

    let Some(python_bin) = resolve_python_command() else {
        bail!(
            "python runtime not found (expected runtime/bin/python/python.exe or system PATH)"
        );
    };
    println!("[OK] Python command: {python_bin}");

    let script_dir = localserver_script
        .parent()
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));
    let script_name = localserver_script
        .file_name()
        .and_then(|s| s.to_str())
        .context("invalid localserver script filename")?
        .to_string();
    let port = localserver_port.to_string();
    let args = [script_name.as_str(), "--port", port.as_str()];

    println!("[Run] starting Tapnow localserver...");
    spawn_background_command(
        &python_bin,
        &args,
        &script_dir,
        log_dir.join("tapnow-localserver.log"),
    )?;
    wait_for(
        "Tapnow localserver",
        Duration::from_secs(HEALTH_TIMEOUT_SECS),
        || is_localserver_ready(localserver_port),
    )
}

fn serve_frontend(dist_dir: &Path, port: u16) -> Result<()> {
    let index_file = dist_dir.join("index.html");
    let index_bytes =
        fs::read(&index_file).with_context(|| format!("missing file: {}", index_file.display()))?;
    let dist_dir = Arc::new(dist_dir.to_path_buf());
    let index_bytes = Arc::new(index_bytes);
    let listener = TcpListener::bind(("127.0.0.1", port))
        .with_context(|| format!("cannot bind frontend port 127.0.0.1:{port}"))?;

    for stream in listener.incoming() {
        match stream {
            Ok(mut s) => {
                let dist_dir = Arc::clone(&dist_dir);
                let index_bytes = Arc::clone(&index_bytes);
                thread::spawn(move || {
                    let _ = s.set_read_timeout(Some(Duration::from_secs(3)));
                    let _ = s.set_write_timeout(Some(Duration::from_secs(3)));
                    if let Err(err) = handle_frontend_request(&mut s, &dist_dir, &index_bytes) {
                        let _ = write_simple_response(
                            &mut s,
                            500,
                            "text/plain; charset=utf-8",
                            err.to_string().as_bytes(),
                            false,
                        );
                    }
                });
            }
            Err(err) => {
                eprintln!("[Warn] frontend accept failed: {err}");
            }
        }
    }
    Ok(())
}

fn handle_frontend_request(stream: &mut TcpStream, dist_dir: &Path, index: &[u8]) -> Result<()> {
    let mut buffer = [0_u8; 8192];
    let read_len = stream.read(&mut buffer).context("read request failed")?;
    if read_len == 0 {
        return Ok(());
    }

    let request = String::from_utf8_lossy(&buffer[..read_len]);
    let first_line = request.lines().next().unwrap_or_default();
    let mut parts = first_line.split_whitespace();
    let method = parts.next().unwrap_or_default();
    let raw_path = parts.next().unwrap_or("/");
    let is_head = method.eq_ignore_ascii_case("HEAD");

    if !method.eq_ignore_ascii_case("GET") && !is_head {
        write_simple_response(
            stream,
            405,
            "text/plain; charset=utf-8",
            b"Method Not Allowed",
            is_head,
        )?;
        return Ok(());
    }

    let path = raw_path.split('?').next().unwrap_or("/");
    if path.contains("..") {
        write_simple_response(
            stream,
            400,
            "text/plain; charset=utf-8",
            b"Bad Request",
            is_head,
        )?;
        return Ok(());
    }

    if path == "/" || path == "/index.html" {
        write_simple_response(stream, 200, "text/html; charset=utf-8", index, is_head)?;
        return Ok(());
    }

    let rel = path.trim_start_matches('/').replace('/', "\\");
    let target = dist_dir.join(rel);
    if target.is_file() {
        let bytes =
            fs::read(&target).with_context(|| format!("failed to read {}", target.display()))?;
        write_simple_response(stream, 200, content_type_for(&target), &bytes, is_head)?;
        return Ok(());
    }

    write_simple_response(stream, 200, "text/html; charset=utf-8", index, is_head)?;
    Ok(())
}

fn write_simple_response(
    stream: &mut TcpStream,
    status: u16,
    content_type: &str,
    body: &[u8],
    head_only: bool,
) -> Result<()> {
    let header = format!(
        "HTTP/1.1 {} {}\r\nContent-Type: {}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        status,
        status_text(status),
        content_type,
        body.len()
    );
    stream
        .write_all(header.as_bytes())
        .context("write header failed")?;
    if !head_only {
        stream.write_all(body).context("write body failed")?;
    }
    Ok(())
}

fn status_text(status: u16) -> &'static str {
    match status {
        200 => "OK",
        400 => "Bad Request",
        405 => "Method Not Allowed",
        500 => "Internal Server Error",
        _ => "OK",
    }
}

fn content_type_for(path: &Path) -> &'static str {
    match path
        .extension()
        .and_then(|s| s.to_str())
        .map(|s| s.to_ascii_lowercase())
        .as_deref()
    {
        Some("html") => "text/html; charset=utf-8",
        Some("js") | Some("mjs") => "application/javascript; charset=utf-8",
        Some("css") => "text/css; charset=utf-8",
        Some("json") => "application/json; charset=utf-8",
        Some("svg") => "image/svg+xml",
        Some("png") => "image/png",
        Some("jpg") | Some("jpeg") => "image/jpeg",
        Some("gif") => "image/gif",
        Some("ico") => "image/x-icon",
        Some("wasm") => "application/wasm",
        Some("txt") => "text/plain; charset=utf-8",
        _ => "application/octet-stream",
    }
}

fn bundled_runtime_dir(name: &str) -> Option<PathBuf> {
    let exe = env::current_exe().ok()?;
    let base = exe.parent()?;
    let runtime_dir = base.join("runtime").join(name);
    if runtime_dir.exists() {
        Some(runtime_dir)
    } else {
        None
    }
}

fn bundled_runtime_file(name: &str) -> Option<PathBuf> {
    let exe = env::current_exe().ok()?;
    let base = exe.parent()?;
    let runtime_path = base.join("runtime").join(name);
    if runtime_path.exists() {
        Some(runtime_path)
    } else {
        None
    }
}

fn env_port(key: &str, default: u16) -> u16 {
    env::var(key)
        .ok()
        .and_then(|v| v.parse::<u16>().ok())
        .unwrap_or(default)
}

fn env_bool(key: &str, default: bool) -> bool {
    match env::var(key) {
        Ok(v) => matches!(
            v.trim().to_ascii_lowercase().as_str(),
            "1" | "true" | "yes" | "on"
        ),
        Err(_) => default,
    }
}

fn ensure_log_dir() -> Result<PathBuf> {
    let base = env::var("LOCALAPPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("."));
    let dir = base.join("TapnowStudio").join("logs");
    fs::create_dir_all(&dir).context("failed to create log directory")?;
    Ok(dir)
}

fn ensure_path(path: &Path, message: &str) -> Result<()> {
    if !path.exists() {
        bail!("{message}: {}", path.display());
    }
    Ok(())
}

fn ensure_file(path: PathBuf, message: &str) -> Result<()> {
    if !path.is_file() {
        bail!("{message}: {}", path.display());
    }
    Ok(())
}

fn resolve_command(candidates: &[&str]) -> Option<String> {
    candidates
        .iter()
        .find(|&&cmd| {
            Command::new(cmd)
                .arg("--version")
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status()
                .map(|s| s.success())
                .unwrap_or(false)
        })
        .map(|s| (*s).to_string())
}

fn run_npm_command(npm_cmd: &str, project_dir: &Path, args: &[&str]) -> Result<()> {
    let status = Command::new(npm_cmd)
        .args(args)
        .current_dir(project_dir)
        .status()
        .with_context(|| format!("failed to execute {npm_cmd} {:?}", args))?;
    if !status.success() {
        bail!("{npm_cmd} {:?} failed", args);
    }
    Ok(())
}

fn spawn_background_command(
    cmd: &str,
    args: &[&str],
    work_dir: &Path,
    log_file: PathBuf,
) -> Result<()> {
    let log_out = File::options()
        .create(true)
        .append(true)
        .open(&log_file)
        .with_context(|| format!("failed to open log file: {}", log_file.display()))?;
    let log_err = log_out
        .try_clone()
        .with_context(|| format!("failed to clone log file handle: {}", log_file.display()))?;

    let mut process = Command::new(cmd);
    process
        .args(args)
        .current_dir(work_dir)
        .stdin(Stdio::null())
        .stdout(Stdio::from(log_out))
        .stderr(Stdio::from(log_err));

    apply_windows_detach(&mut process);
    process
        .spawn()
        .with_context(|| format!("failed to start background command: {cmd} {:?}", args))?;
    Ok(())
}

#[cfg(windows)]
fn apply_windows_detach(cmd: &mut Command) {
    use std::os::windows::process::CommandExt;
    cmd.creation_flags(CREATE_NO_WINDOW | CREATE_NEW_PROCESS_GROUP | DETACHED_PROCESS);
}

#[cfg(not(windows))]
fn apply_windows_detach(_cmd: &mut Command) {}

fn wait_for<F>(name: &str, timeout: Duration, checker: F) -> Result<()>
where
    F: Fn() -> bool,
{
    let start = Instant::now();
    while start.elapsed() < timeout {
        if checker() {
            println!("[OK] {name} started");
            return Ok(());
        }
        thread::sleep(Duration::from_millis(800));
    }
    bail!("{name} start timeout ({}s)", timeout.as_secs())
}

fn is_port_open(port: u16) -> bool {
    let mut addrs = match ("127.0.0.1", port).to_socket_addrs() {
        Ok(a) => a,
        Err(_) => return false,
    };
    if let Some(addr) = addrs.next() {
        TcpStream::connect_timeout(&addr, Duration::from_millis(500)).is_ok()
    } else {
        false
    }
}

fn is_api_ready(api_port: u16) -> bool {
    if !is_port_open(api_port) {
        return false;
    }
    http_get(api_port, "/ping")
        .map(|resp| resp.contains("200") && resp.to_ascii_lowercase().contains("pong"))
        .unwrap_or(false)
}

fn is_localserver_ready(localserver_port: u16) -> bool {
    if !is_port_open(localserver_port) {
        return false;
    }
    http_get(localserver_port, "/ping")
        .map(|resp| resp.contains("200") && resp.to_ascii_lowercase().contains("running"))
        .unwrap_or(false)
}

fn is_frontend_ready(frontend_port: u16) -> bool {
    if !is_port_open(frontend_port) {
        return false;
    }
    http_get(frontend_port, "/")
        .map(|resp| resp.contains("200") && resp.contains("Tapnow"))
        .unwrap_or(false)
}

fn http_get(port: u16, path: &str) -> Result<String> {
    let mut stream = TcpStream::connect(("127.0.0.1", port))
        .with_context(|| format!("failed to connect 127.0.0.1:{port}"))?;
    stream
        .set_read_timeout(Some(Duration::from_secs(2)))
        .context("failed to set read timeout")?;
    stream
        .set_write_timeout(Some(Duration::from_secs(2)))
        .context("failed to set write timeout")?;

    let request =
        format!("GET {path} HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nConnection: close\r\n\r\n");
    stream
        .write_all(request.as_bytes())
        .with_context(|| format!("failed to write request to 127.0.0.1:{port}{path}"))?;

    let mut resp = String::new();
    stream
        .read_to_string(&mut resp)
        .with_context(|| format!("failed to read response from 127.0.0.1:{port}{path}"))?;
    Ok(resp)
}

fn open_browser_in_new_window(frontend_port: u16) -> Result<()> {
    let url = format!("http://127.0.0.1:{frontend_port}");

    let candidates: [(&[&str], &[&str]); 3] = [
        (&["msedge", "msedge.exe"], &["--new-window"]),
        (&["chrome", "chrome.exe"], &["--new-window"]),
        (&["firefox", "firefox.exe"], &["-new-window"]),
    ];

    for (bins, window_args) in candidates {
        if let Some(browser) = resolve_command(bins) {
            let mut args: Vec<&str> = window_args.to_vec();
            args.push(url.as_str());
            Command::new(browser)
                .args(args)
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
                .context("failed to launch browser in new window")?;
            return Ok(());
        }
    }

    Command::new("cmd")
        .args(["/C", "start", "", &url])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .with_context(|| format!("failed to open browser: {url}"))?;
    Ok(())
}
