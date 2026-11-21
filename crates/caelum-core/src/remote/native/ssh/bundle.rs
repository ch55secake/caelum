use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    process::Child,
};

pub(super) struct Bundle {
    directory: PathBuf,
    assets: BTreeMap<String, (String, u64)>,
}

impl Bundle {
    pub(super) fn discover() -> Result<Option<Self>, String> {
        let exe = std::env::current_exe().map_err(|e| e.to_string())?;
        let Some(contents) = exe.parent().and_then(Path::parent) else {
            return Ok(None);
        };
        Self::load(&contents.join("Resources/vm-peers"))
    }

    pub(super) fn load(directory: &Path) -> Result<Option<Self>, String> {
        let manifest = match std::fs::read(directory.join("manifest.json")) {
            Ok(m) => m,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(e.to_string()),
        };
        let assets: BTreeMap<String, (String, u64)> =
            serde_json::from_slice(&manifest).map_err(|_| "Invalid bundled VM peer manifest.")?;
        if !["x86_64", "aarch64"].iter().all(|&arch| {
            assets.get(arch).is_some_and(|(hash, size)| {
                hash.len() == 64
                    && hash.bytes().all(|b| b.is_ascii_hexdigit())
                    && (1..=512 * 1024 * 1024).contains(size)
            })
        }) {
            return Err("The desktop bundle is missing a compatible Linux peer.".into());
        }
        Ok(Some(Self {
            directory: directory.into(),
            assets,
        }))
    }

    pub(super) fn arguments(&self) -> String {
        ["x86_64", "aarch64"]
            .map(|a| {
                let (hash, size) = &self.assets[a];
                format!(" {hash} {size}")
            })
            .concat()
    }

    pub(super) async fn install(&self, child: &mut Child) -> Result<(), String> {
        let stdout = child.stdout.as_mut().expect("piped stdout");
        let mut line = Vec::new();
        loop {
            let byte = stdout.read_u8().await.map_err(|_|"SSH closed before the VM could select its Linux peer. Check that the VM is running and SSH is authorized.")?;
            if byte == b'\n' {
                break;
            }
            if line.len() >= 32 {
                return Err("Invalid VM peer installer response.".into());
            }
            line.push(byte);
        }
        if line == b"cached" {
            return Ok(());
        }
        let arch = std::str::from_utf8(&line)
            .ok()
            .and_then(|l| l.strip_prefix("install "))
            .filter(|a| matches!(*a, "x86_64" | "aarch64"))
            .ok_or("Unsupported VM peer architecture.")?;
        let file = tokio::fs::File::open(self.directory.join(arch))
            .await
            .map_err(|e| e.to_string())?;
        let size = self.assets[arch].1;
        if file.metadata().await.map_err(|e| e.to_string())?.len() != size {
            return Err("Incomplete bundled VM peer.".into());
        }
        let input = child.stdin.as_mut().expect("piped stdin");
        let copied = tokio::io::copy(&mut file.take(size), input)
            .await
            .map_err(|e| e.to_string())?;
        if copied != size {
            return Err("Incomplete VM peer upload.".into());
        }
        input.flush().await.map_err(|e| e.to_string())
    }
}
