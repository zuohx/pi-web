#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::{
    net::{SocketAddr, TcpStream},
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;

/// Prevents the child process from opening a console window on Windows.
#[cfg(target_os = "windows")]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

use tauri::{async_runtime::spawn, webview::WebviewWindowBuilder, AppHandle, Manager, RunEvent, WebviewUrl};

struct ServerProcess(Arc<Mutex<Option<Child>>>);

impl ServerProcess {
    fn new() -> Self {
        Self(Arc::new(Mutex::new(None)))
    }

    fn set(&self, child: Child) {
        *self.0.lock().expect("server process lock poisoned") = Some(child);
    }

    fn kill(&self) {
        if let Some(mut child) = self.0.lock().expect("server process lock poisoned").take() {
            #[cfg(target_os = "windows")]
            {
                // taskkill /T kills the entire tree (pi-web-desktop.js → server.js).
                // Must run BEFORE child.wait(); after wait() the PID is invalid.
                let pid = child.id();
                let _ = Command::new("taskkill")
                    .args(["/T", "/F", "/PID", &pid.to_string()])
                    .creation_flags(CREATE_NO_WINDOW)
                    .output();
            }
            #[cfg(not(target_os = "windows"))]
            {
                let _ = child.kill();
            }
            let _ = child.wait();
        }
    }
}

fn project_root(app: &AppHandle) -> PathBuf {
    if let Ok(resource_dir) = app.path().resource_dir() {
        if resource_dir.join("bin").join("pi-web-desktop.js").exists() {
            return resource_dir;
        }
    }

    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("src-tauri should live under the project root")
        .to_path_buf()
}

fn desktop_script_path(root: &Path) -> PathBuf {
    root.join("bin").join("pi-web-desktop.js")
}

fn resolve_node_command() -> String {
    std::env::var("PI_WEB_NODE").unwrap_or_else(|_| "node".to_string())
}

fn wait_for_port(addr: SocketAddr, timeout: Duration) -> bool {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        if TcpStream::connect_timeout(&addr, Duration::from_millis(300)).is_ok() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(250));
    }
    false
}

fn spawn_server(root: &Path, port: u16) -> Result<Child, String> {
    let script = desktop_script_path(root);
    if !script.exists() {
        return Err(format!("Desktop launcher not found: {}", script.display()));
    }

    let node = resolve_node_command();
    let mut cmd = Command::new(node);
    cmd.arg(script)
        .arg("--port")
        .arg(port.to_string())
        .arg("--hostname")
        .arg("127.0.0.1")
        .current_dir(root)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    // Prevent the child node process from spawning a visible console window.
    #[cfg(target_os = "windows")]
    cmd.creation_flags(CREATE_NO_WINDOW);

    cmd.spawn()
        .map_err(|err| format!("Failed to start pi-web desktop server: {err}"))
}

fn create_loading_window(app: &AppHandle) -> tauri::Result<()> {
    let builder = WebviewWindowBuilder::new(app, "loading", WebviewUrl::App("index.html".into()))
        .title("Starting pi-web…")
        .inner_size(520.0, 320.0)
        .min_inner_size(420.0, 260.0)
        .resizable(false)
        .focused(true)
        .visible(true)
        .center();

    let _window = builder.build()?;
    Ok(())
}

fn create_window(app: &AppHandle, url: &str) -> tauri::Result<()> {
    let webview_url = WebviewUrl::External(url.parse().expect("desktop URL should be valid"));
    let builder = WebviewWindowBuilder::new(app, "main", webview_url)
        .title("pi-web")
        .inner_size(1440.0, 960.0)
        .min_inner_size(960.0, 640.0)
        .resizable(true)
        .focused(true)
        .visible(true);

    let _window = builder.build()?;
    Ok(())
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let app_handle = app.handle().clone();
            let server = ServerProcess::new();
            app.manage(server);

            if let Err(err) = create_loading_window(&app_handle) {
                eprintln!("Failed to create loading window: {err}");
                app_handle.exit(1);
                return Ok(());
            }

            spawn(async move {
                let root = project_root(&app_handle);
                let port = 30141u16;
                let addr = SocketAddr::from(([127, 0, 0, 1], port));

                let child = match spawn_server(&root, port) {
                    Ok(child) => child,
                    Err(err) => {
                        eprintln!("{err}");
                        app_handle.exit(1);
                        return;
                    }
                };

                app_handle.state::<ServerProcess>().set(child);

                if !wait_for_port(addr, Duration::from_secs(45)) {
                    eprintln!("Timed out waiting for pi-web desktop server on http://127.0.0.1:{port}");
                    app_handle.state::<ServerProcess>().kill();
                    app_handle.exit(1);
                    return;
                }

                let url = format!("http://127.0.0.1:{port}");
                if let Err(err) = create_window(&app_handle, &url) {
                    eprintln!("Failed to create main window: {err}");
                    app_handle.state::<ServerProcess>().kill();
                    app_handle.exit(1);
                    return;
                }

                if let Some(window) = app_handle.get_webview_window("loading") {
                    let _ = window.close();
                }
            });

            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app_handle, event| {
            if matches!(event, RunEvent::Exit { .. } | RunEvent::ExitRequested { .. }) {
                app_handle.state::<ServerProcess>().kill();
            }
        });
}
