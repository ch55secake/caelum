use crate::remote::boat::checkout_quote as quote;
use crate::{database::WorkspaceDatabase, github::cache::Cache, remote::native::NativeTarget};
use std::{
    collections::HashMap,
    net::TcpListener,
    path::{Path, PathBuf},
    process::Stdio,
    time::Duration,
};
use tokio::{io::AsyncReadExt, process::Command, runtime::Handle};

pub(crate) struct Metadata {
    pub(crate) branches: WorkspaceDatabase,
    target: NativeTarget,
    parent: PathBuf,
    cache: Cache,
}
impl Metadata {
    pub(super) fn open(target: &NativeTarget, parent: &Path, store: &Path) -> Result<Self, String> {
        Ok(Self {
            branches: WorkspaceDatabase::open(store.with_file_name("branches.sqlite3"))
                .map_err(|e| e.to_string())?,
            target: target.clone(),
            parent: parent.into(),
            cache: Cache::for_store(store),
        })
    }
    pub(crate) fn review<T>(&self, f: impl FnOnce(&Cache) -> T) -> Option<T> {
        self.target
            .with_live_profile(&self.parent, || Ok(f(&self.cache)))
            .ok()
    }
}

/// Operations can only use this authenticated master; OpenSSH must not fall back
/// to a fresh connection when its control socket disappears.
pub(crate) struct Access {
    socket: PathBuf,
    host: String,
    forwards: tokio::sync::Mutex<HashMap<(String, u16), u16>>,
    pub(crate) root: String,
    pub(crate) io: Handle,
    pub(crate) metadata: Option<Metadata>,
    pub(crate) opencode_used: std::sync::atomic::AtomicBool,
    pub(crate) codex_used: std::sync::atomic::AtomicBool,
    pub(crate) live: std::sync::Arc<std::sync::atomic::AtomicBool>,
}

impl Access {
    pub(super) fn new(socket: PathBuf, host: String, root: String) -> Self {
        Self {
            socket,
            host,
            forwards: Default::default(),
            root,
            io: Handle::current(),
            metadata: None,
            opencode_used: Default::default(),
            codex_used: Default::default(),
            live: std::sync::Arc::new(true.into()),
        }
    }
    pub(in crate::remote::native) fn channel(&self, options: &[&str]) -> Command {
        let mut cmd = Command::new("ssh");
        cmd.args(["-F", "/dev/null", "-S"])
            .arg(&self.socket)
            .args(["-o", "ProxyCommand=false", "-o", "BatchMode=yes", "-T"])
            .args(options)
            .arg("--")
            .arg(&self.host)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .kill_on_drop(true);
        cmd
    }

    pub(in crate::remote::native) async fn run(
        &self,
        script: &str,
        limit: usize,
        codes: &[i32],
    ) -> Result<Vec<u8>, String> {
        if !self.live.load(std::sync::atomic::Ordering::Acquire) {
            return Err("The VM connection closed.".into());
        }
        let mut child = self
            .channel(&[])
            .arg(script)
            .spawn()
            .map_err(|e| e.to_string())?;
        tokio::time::timeout(Duration::from_secs(35), async {
            let mut data = Vec::new();
            child
                .stdout
                .take()
                .expect("piped")
                .take(limit as u64 + 1)
                .read_to_end(&mut data)
                .await
                .map_err(|e| e.to_string())?;
            if data.len() > limit {
                return Err("VM output exceeds the preview limit.".into());
            }
            let status = child.wait().await.map_err(|e| e.to_string())?;
            if !status.code().is_some_and(|c| codes.contains(&c)) {
                return Err(format!("VM command failed ({status})."));
            }
            Ok(data)
        })
        .await
        .map_err(|_| "VM command timed out.")?
    }

    async fn forward(&self, host: &str, remote: u16) -> Result<u16, String> {
        let key = (host.to_owned(), remote);
        let mut forwards = self.forwards.lock().await;
        if let Some(port) = forwards.get(&key) {
            return Ok(*port);
        }
        if forwards.len() >= 8 {
            return Err(
                "VM provider endpoints changed repeatedly. Reconnect to refresh them.".into(),
            );
        }
        let local = TcpListener::bind(("127.0.0.1", 0))
            .and_then(|l| l.local_addr())
            .map_err(|e| e.to_string())?
            .port();
        let spec = format!("127.0.0.1:{local}:{host}:{remote}");
        let status = self
            .channel(&["-O", "forward", "-L", &spec])
            .status()
            .await
            .map_err(|e| e.to_string())?;
        if !status.success() {
            return Err("Could not forward the native provider over SSH.".into());
        }
        forwards.insert(key, local);
        Ok(local)
    }

    fn start(&self, script: &str, limit: usize) -> Result<Vec<u8>, String> {
        let cmd = format!("cd -P {} && sh -c {}", quote(&self.root), quote(script));
        self.io.block_on(self.run(&cmd, limit, &[0]))
    }
    pub(crate) fn opencode(&self) -> Result<(String, Option<String>, u64), String> {
        #[derive(serde::Deserialize)]
        struct Registration {
            url: String,
            password: Option<String>,
            pid: u64,
        }
        let bytes=self.start(include_str!("opencode.sh"),16384).map_err(|_|
            "Could not start or discover OpenCode on the VM. Check `opencode service status` on the machine.")?;
        let reg: Registration = serde_json::from_slice(&bytes)
            .map_err(|_| "Invalid VM OpenCode service registration.")?;
        let url = reqwest::Url::parse(&reg.url).map_err(|_| "Invalid VM OpenCode endpoint.")?;
        let host = match (url.scheme(), url.host_str()) {
            ("http", Some("127.0.0.1" | "localhost")) => "127.0.0.1",
            ("http", Some("[::1]")) => "[::1]",
            _ => return Err("The VM OpenCode service must listen on loopback.".into()),
        };
        let remote = url
            .port_or_known_default()
            .ok_or("Missing OpenCode port.")?;
        let port = self.io.block_on(self.forward(host, remote))?;
        Ok((format!("http://127.0.0.1:{port}"), reg.password, reg.pid))
    }

    pub(crate) fn codex(&self) -> Result<String, String> {
        let bytes=self.start(include_str!("codex.sh"),16).map_err(|_|"Codex on the VM could not serve WebSocket app-server. Check `codex app-server --help` and its loopback listener.")?;
        let remote = std::str::from_utf8(&bytes)
            .ok()
            .and_then(|s| s.trim().parse::<u16>().ok())
            .filter(|p| *p > 0)
            .ok_or("Invalid Codex listener port.")?;
        let port = self.io.block_on(self.forward("127.0.0.1", remote))?;
        Ok(format!("ws://127.0.0.1:{port}"))
    }

    pub(crate) fn gh<'a>(
        &self,
        args: impl Iterator<Item = &'a std::ffi::OsStr>,
        timeout: Duration,
    ) -> Result<Vec<u8>, String> {
        let args = args
            .map(|a| a.to_str().map(quote).ok_or("Invalid gh argument"))
            .collect::<Result<Vec<_>, _>>()?
            .join(" ");
        let script=format!("cd -P {} && unset GH_REPO && PATH=\"$HOME/.local/bin:$PATH\" GH_PROMPT_DISABLED=1 GH_PAGER=cat GH_NO_UPDATE_NOTIFIER=1 gh {args}",quote(&self.root));
        self.io.block_on(async {
            tokio::time::timeout(timeout, self.run(&script, 1024 * 1024, &[0]))
                .await
                .map_err(|_| "GitHub timed out on the VM.")?
        })
    }
}
