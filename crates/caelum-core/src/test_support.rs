use std::{
    io::Write,
    path::Path,
    process::{Command, Stdio},
};

pub(crate) fn write_executable(path: &Path, contents: impl AsRef<[u8]>) {
    // Keep writable executable handles out of the test process. A concurrent
    // spawn can inherit them until exec, causing ETXTBSY on Linux even after
    // the writing test has closed its own handle.
    let mut writer = Command::new("/bin/sh")
        .args(["-c", "cat > \"$1\" && chmod 755 \"$1\"", "write-fixture"])
        .arg(path)
        .stdin(Stdio::piped())
        .spawn()
        .expect("start executable fixture writer");
    writer
        .stdin
        .take()
        .expect("piped fixture input")
        .write_all(contents.as_ref())
        .expect("write executable fixture");
    assert!(
        writer.wait().expect("wait for fixture writer").success(),
        "could not create executable fixture {}",
        path.display()
    );
}
