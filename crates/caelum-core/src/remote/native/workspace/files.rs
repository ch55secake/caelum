use super::Access;
use crate::{files, images, remote::boat::checkout_quote as quote};
use caelum_protocol::{
    ProjectDirectoryListing, ProjectEntryKind, ProjectFileContent, ProjectFileEntry,
};
use russh_sftp::client::SftpSession;
use std::{path::Path, process::Stdio};
use tokio::{io::AsyncReadExt, process::Child};

fn error(e: impl std::fmt::Display) -> String {
    format!("VM file access: {e}")
}

impl Access {
    pub(super) async fn stamps(&self, changes: &mut [caelum_protocol::GitFileStatus]) {
        if changes.is_empty() {
            return;
        }
        let Ok((_child, session)) = self.sftp().await else {
            return;
        };
        use futures_util::StreamExt;
        futures_util::stream::iter(changes)
            .for_each_concurrent(16, |change| {
                let session = &session;
                async move {
                    if files::validate_relative_path(&change.path).is_err() {
                        return;
                    }
                    if let Ok(attrs) = session
                        .symlink_metadata(format!("{}/{}", self.root, change.path))
                        .await
                    {
                        change.modified_at = attrs
                            .mtime
                            .map(|s| (u64::from(s) * 1_000_000_000).to_string());
                        change.revision =
                            format!("{} {:?} {:?}", change.revision, attrs.mtime, attrs.size);
                    }
                }
            })
            .await;
    }
    async fn sftp(&self) -> Result<(Child, SftpSession), String> {
        let mut child = self
            .channel(&["-s"])
            .arg("sftp")
            .stdin(Stdio::piped())
            .spawn()
            .map_err(error)?;
        let stream = tokio::io::join(child.stdout.take().unwrap(), child.stdin.take().unwrap());
        let session = SftpSession::new(stream).await.map_err(error)?;
        Ok((child, session))
    }
    async fn scoped(&self, s: &SftpSession, path: &str) -> Result<String, String> {
        if !path.is_empty() {
            files::validate_relative_path(path).map_err(error)?;
        }
        let absolute = if path.is_empty() {
            self.root.clone()
        } else {
            format!("{}/{path}", self.root)
        };
        let physical = s.canonicalize(absolute).await.map_err(error)?;
        if !Path::new(&physical)
            .strip_prefix(&self.root)
            .is_ok_and(|p| {
                p.components()
                    .all(|c| matches!(c, std::path::Component::Normal(_)))
            })
        {
            return Err("Path resolves outside the VM checkout.".into());
        }
        Ok(physical)
    }
    pub(super) async fn directory(
        &self,
        path: &str,
        offset: usize,
    ) -> Result<ProjectDirectoryListing, String> {
        let (_child, s) = self.sftp().await?;
        let dir = self.scoped(&s, path).await?;
        let mut entries = vec![];
        for entry in s.read_dir(dir).await.map_err(error)? {
            let name = entry.file_name();
            if name == ".git" || name.contains('/') || files::validate_relative_path(&name).is_err()
            {
                continue;
            }
            let attrs = entry.metadata();
            let kind = if attrs.is_dir() {
                ProjectEntryKind::Directory
            } else if attrs.is_regular() {
                ProjectEntryKind::File
            } else if attrs.is_symlink() {
                ProjectEntryKind::Symlink
            } else {
                continue;
            };
            entries.push(ProjectFileEntry {
                path: if path.is_empty() {
                    name.clone()
                } else {
                    format!("{path}/{name}")
                },
                name,
                kind,
                size_bytes: if kind == ProjectEntryKind::File {
                    attrs.size
                } else {
                    None
                },
            });
        }
        Ok(files::directory_page(
            entries,
            path,
            offset,
            files::DEFAULT_DIRECTORY_PAGE_SIZE,
        ))
    }
    async fn bytes(&self, path: &str, limit: u64) -> Result<(Vec<u8>, u64), String> {
        files::validate_relative_path(path).map_err(error)?;
        let (_child, s) = self.sftp().await?;
        let physical = self.scoped(&s, path).await?;
        let file = s.open(physical).await.map_err(error)?;
        let attrs = file.metadata().await.map_err(error)?;
        if !attrs.is_regular() {
            return Err("VM path is not a regular file.".into());
        }
        let mut bytes = vec![];
        file.take(limit + 1)
            .read_to_end(&mut bytes)
            .await
            .map_err(error)?;
        let size = attrs.size.unwrap_or(bytes.len() as u64);
        Ok((bytes, size))
    }
    pub(super) async fn file(&self, path: &str) -> Result<ProjectFileContent, String> {
        let (bytes, size) = self.bytes(path, files::MAX_TEXT_FILE_BYTES as u64).await?;
        files::text_preview(path, size, bytes).map_err(error)
    }
    pub(super) async fn image(&self, source: &str) -> Result<String, String> {
        let bytes = if source.starts_with("https://") || source.starts_with("http://") {
            self.run(&format!("curl -fsSL --proto '=http,https' --proto-redir '=http,https' --max-time 20 -- {}",quote(source)),images::MAX_IMAGE_BYTES as usize,&[0]).await?
        } else {
            self.bytes(source, images::MAX_IMAGE_BYTES).await?.0
        };
        images::data_url(bytes.as_slice())
    }
}
