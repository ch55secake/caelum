use super::NativeTarget;
mod access;
use crate::remote::{
    boat::{api, checkout_quote, Boat},
    credentials,
};
pub(crate) use access::Access;
use base64::{engine::general_purpose::STANDARD, Engine};
use boat_sdk::access::models::{SshKeyRequest, SshKeyResponse};
use std::{
    fs,
    net::{IpAddr, SocketAddr},
    os::unix::fs::DirBuilderExt,
    path::{Path, PathBuf},
    process::Stdio,
    time::Duration,
};
use tokio::process::{Child, Command};

pub(super) struct Tunnel {
    pub child: Child,
    // Child::wait closes its stdin; keep the master session's lifetime distinct
    // from its exit observer.
    _keepalive: tokio::process::ChildStdin,
    pub access: std::sync::Arc<Access>,
    _pin: tempfile::TempDir,
}

fn identity(directory: &Path) -> Result<(PathBuf, String), String> {
    fs::DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(directory)
        .map_err(|e| e.to_string())?;
    let key = directory.join("id_ed25519");
    if !key.exists() {
        let temp = tempfile::tempdir_in(directory).map_err(|e| e.to_string())?;
        let path = temp.path().join("identity");
        let status = std::process::Command::new("ssh-keygen")
            .args(["-q", "-t", "ed25519", "-N", "", "-C", "caelum-vm", "-f"])
            .arg(&path)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map_err(|e| e.to_string())?;
        if !status.success() {
            return Err("Could not create this workspace’s SSH identity.".into());
        }
        // Never overwrite an identity already installed by another client.
        if let Err(e) = fs::hard_link(path, &key) {
            if e.kind() != std::io::ErrorKind::AlreadyExists {
                return Err(e.to_string());
            }
        }
    }
    let output = std::process::Command::new("ssh-keygen")
        .args(["-y", "-P", "", "-f"])
        .arg(&key)
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .map_err(|e| e.to_string())?;
    let public = String::from_utf8(output.stdout).map_err(|e| e.to_string())?;
    if !output.status.success() || !public.starts_with("ssh-ed25519 ") {
        return Err("Could not read this workspace’s SSH identity.".into());
    }
    Ok((key, public.trim().to_owned()))
}

async fn authorize(
    boat: &Boat,
    target: &NativeTarget,
    public: String,
) -> Result<SshKeyResponse, String> {
    boat.owns(&target.machine)?;
    boat.client
        .add_ssh_key(&target.machine.id, &SshKeyRequest { key: public })
        .await
        .map_err(|e| {
            if matches!(&e,boat_sdk::Error::Api{status,..} if status.as_u16()==403) {
                "The Boat key needs the ssh action to open a native workspace.".into()
            } else {
                format!("Could not authorize SSH on Boat: {}", api(e))
            }
        })
}

struct Endpoint {
    host: String,
    port: u16,
    user: String,
    pin: String,
}
impl Endpoint {
    fn new(r: SshKeyResponse) -> Result<Self, String> {
        if !r.ok || r.success == Some(false) {
            return Err("Boat did not authorize this SSH connection.".into());
        }
        let (host, port) =
            if let Some(endpoint) = r.ssh_endpoint.as_deref().filter(|s| !s.is_empty()) {
                if let Ok(s) = endpoint.parse::<SocketAddr>() {
                    (s.ip().to_string(), s.port())
                } else {
                    let (host, port) = endpoint
                        .rsplit_once(':')
                        .ok_or("Invalid Boat SSH endpoint")?;
                    if host.is_empty()
                        || host.starts_with('-')
                        || !host
                            .bytes()
                            .all(|b| b.is_ascii_alphanumeric() || b".-".contains(&b))
                    {
                        return Err("Invalid Boat SSH host".into());
                    }
                    (
                        host.into(),
                        port.parse::<u16>().map_err(|_| "Invalid Boat SSH port")?,
                    )
                }
            } else {
                let ip: IpAddr = r
                    .machine_ip
                    .as_deref()
                    .ok_or("Boat did not return a machine IP")?
                    .parse()
                    .map_err(|_| "Invalid Boat machine IP")?;
                (ip.to_string(), 22)
            };
        let user = r.ssh_user.unwrap_or_else(|| "user".into());
        if port == 0
            || user.is_empty()
            || user.starts_with('-')
            || !user
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"_-".contains(&b))
        {
            return Err("Invalid Boat SSH login".into());
        }
        let key = r.host_key.ok_or("Boat did not return a host key")?;
        if key.contains(['\r', '\n', '\0']) {
            return Err("Invalid Boat SSH host key".into());
        }
        let mut parts = key.split_ascii_whitespace();
        let kind = parts.next().unwrap_or("");
        let blob = parts.next().ok_or("Invalid Boat SSH host key")?;
        if blob.len() > 16 * 1024 {
            return Err("Invalid Boat SSH host key".into());
        }
        let decoded = STANDARD.decode(blob).ok().filter(|key| {
            key.get(..4) == Some(&(kind.len() as u32).to_be_bytes())
                && key[4..].starts_with(kind.as_bytes())
                && key.len() > 4 + kind.len()
        });
        if !matches!(
            kind,
            "ssh-ed25519"
                | "ssh-rsa"
                | "ecdsa-sha2-nistp256"
                | "ecdsa-sha2-nistp384"
                | "ecdsa-sha2-nistp521"
        ) || decoded.is_none()
        {
            return Err("Invalid Boat SSH host key".into());
        }
        Ok(Self {
            host,
            port,
            user,
            pin: format!("caelum-boat {kind} {blob}\n"),
        })
    }
}

pub(super) async fn open(target: &NativeTarget, parent_store: &Path) -> Result<Tunnel, String> {
    let owner = target.clone();
    let parent = parent_store.to_owned();
    let ((key, public), metadata) = tokio::task::spawn_blocking(move || {
        owner.with_live_profile(&parent, || {
            let store = owner.store_path(&parent);
            let dir = store.parent().expect("workspace directory");
            Ok((
                identity(&dir.join("ssh"))?,
                access::Metadata::open(&owner, &parent, &store)?,
            ))
        })
    })
    .await
    .map_err(|e| e.to_string())??;
    let endpoint = tokio::time::timeout(Duration::from_secs(22), async {
        let key = credentials::read().await?;
        let boat = Boat::authenticate(boat_sdk::Client::new(key.0.trim()).map_err(api)?).await?;
        Endpoint::new(authorize(&boat, target, public).await?)
    })
    .await
    .map_err(|_| "Boat timed out authorizing SSH.")??;
    let mut tunnel = connect(key, endpoint, &target.repository, &target.checkout_root).await?;
    std::sync::Arc::get_mut(&mut tunnel.access)
        .expect("new SSH attempt")
        .metadata = Some(metadata);
    Ok(tunnel)
}

async fn connect(
    key: PathBuf,
    endpoint: Endpoint,
    repository: &str,
    root: &str,
) -> Result<Tunnel, String> {
    let pin = tempfile::tempdir().map_err(|e| e.to_string())?;
    let known = pin.path().join("known_hosts");
    fs::write(&known, endpoint.pin).map_err(|e| e.to_string())?;
    let socket = pin.path().join("control");
    let dir = checkout_quote(root);
    // Opening the session channel confirms the master can accept multiplexed
    // requests. Keep it alive until the desktop detaches or SSH loses the VM.
    let remote = format!("cd -P {dir} && test \"$PWD\" = {dir} && test \"$(git rev-parse --show-toplevel)\" = \"$PWD\" && git remote get-url origin && exec cat >/dev/null");
    let mut child = Command::new("ssh")
        .args([
            "-F",
            "/dev/null",
            "-T",
            "-o",
            "BatchMode=yes",
            "-o",
            "StrictHostKeyChecking=yes",
            "-o",
            "HostKeyAlias=caelum-boat",
            "-o",
            "GlobalKnownHostsFile=/dev/null",
            "-o",
            "IdentitiesOnly=yes",
            "-o",
            "IdentityAgent=none",
            "-o",
            "ForwardAgent=no",
            "-o",
            "ExitOnForwardFailure=yes",
            "-o",
            "ConnectTimeout=10",
            "-o",
            "ServerAliveInterval=10",
            "-o",
            "ServerAliveCountMax=2",
        ])
        .args(["-M", "-S"])
        .arg(&socket)
        .arg("-o")
        .arg(format!("UserKnownHostsFile=\"{}\"", known.display()))
        .arg("-i")
        .arg(key)
        .arg("-p")
        .arg(endpoint.port.to_string())
        .arg("-l")
        .arg(endpoint.user)
        .arg("--")
        .arg(&endpoint.host)
        .arg(remote)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .kill_on_drop(true)
        .spawn()
        .map_err(|e| format!("Start OpenSSH: {e}"))?;
    use tokio::io::{AsyncBufReadExt, AsyncReadExt, BufReader};
    let mut origin = String::new();
    let mut reader = BufReader::new(child.stdout.take().expect("piped stdout").take(8192));
    tokio::time::timeout(Duration::from_secs(15), reader.read_line(&mut origin))
        .await
        .map_err(|_| "SSH timed out verifying the VM checkout.")?
        .map_err(|e| e.to_string())?;
    if !origin.ends_with('\n')
        || !crate::remote::github_slug(origin.trim())
            .is_some_and(|r| r.eq_ignore_ascii_case(repository))
    {
        return Err("SSH could not verify the VM host key and Git checkout. Check the machine and refresh its connection.".into());
    }
    Ok(Tunnel {
        _keepalive: child.stdin.take().expect("piped stdin"),
        child,
        access: std::sync::Arc::new(Access::new(socket, endpoint.host, root.to_owned())),
        _pin: pin,
    })
}

#[cfg(test)]
mod tests;
