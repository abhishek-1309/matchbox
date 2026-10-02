//! Execute a decoded hypercall against the host jail.
//!
//! Guest ABI. Pointers are guest physical addresses into guest RAM.
//! - Mkdir:     arg0 path, arg1 len
//! - WriteFile: arg0 path, arg1 path len, arg2 data, arg3 data len
//! - CopyFile:  arg0 src,  arg1 src len,  arg2 dst,  arg3 dst len
//! - HttpFetch: arg0 url,  arg1 url len,  arg2 out buffer, arg3 buffer len
//!
//! `status` is 0 for successful file ops, the byte count for a successful
//! fetch, and a negative code on failure.

use std::path::Path;

use super::{HypercallCommand, HypercallFrame};
use crate::security::{self, SecurityError};

pub const OK: i32 = 0;
pub const ERR_INVAL: i32 = -1;
pub const ERR_SECURITY: i32 = -2;
pub const ERR_IO: i32 = -3;
pub const ERR_HTTP: i32 = -4;
pub const ERR_FAULT: i32 = -5;

const MAX_BLOB: u64 = 1024 * 1024;

pub fn dispatch(frame: &mut HypercallFrame, guest_mem: &mut [u8], jail_root: &Path) -> i32 {
    let status = execute(frame, guest_mem, jail_root);
    frame.status = status;
    status
}

fn execute(frame: &HypercallFrame, guest_mem: &mut [u8], jail_root: &Path) -> i32 {
    match HypercallFrame::decode(frame.command_id) {
        Ok(HypercallCommand::Mkdir) => mkdir(frame, guest_mem, jail_root),
        Ok(HypercallCommand::WriteFile) => write_file(frame, guest_mem, jail_root),
        Ok(HypercallCommand::CopyFile) => copy_file(frame, guest_mem, jail_root),
        Ok(HypercallCommand::HttpFetch) => http_fetch(frame, guest_mem),
        Ok(HypercallCommand::Exit) => OK,
        Err(_) => ERR_INVAL,
    }
}

fn mkdir(frame: &HypercallFrame, mem: &[u8], jail_root: &Path) -> i32 {
    let path = match read_str(mem, frame.arg0, frame.arg1) {
        Ok(path) => path,
        Err(code) => return code,
    };
    let host_path = match security::resolve_creatable_path(jail_root, path) {
        Ok(path) => path,
        Err(err) => return map_security(err),
    };
    match std::fs::create_dir(&host_path) {
        Ok(()) => OK,
        Err(err) => map_io(err),
    }
}

fn write_file(frame: &HypercallFrame, mem: &[u8], jail_root: &Path) -> i32 {
    let path = match read_str(mem, frame.arg0, frame.arg1) {
        Ok(path) => path.to_string(),
        Err(code) => return code,
    };
    let data = match read_bytes(mem, frame.arg2, frame.arg3) {
        Ok(data) => data.to_vec(),
        Err(code) => return code,
    };
    let host_path = match security::resolve_creatable_path(jail_root, &path) {
        Ok(path) => path,
        Err(err) => return map_security(err),
    };
    match std::fs::write(&host_path, &data) {
        Ok(()) => OK,
        Err(err) => map_io(err),
    }
}

fn copy_file(frame: &HypercallFrame, mem: &[u8], jail_root: &Path) -> i32 {
    let src = match read_str(mem, frame.arg0, frame.arg1) {
        Ok(path) => path.to_string(),
        Err(code) => return code,
    };
    let dst = match read_str(mem, frame.arg2, frame.arg3) {
        Ok(path) => path.to_string(),
        Err(code) => return code,
    };
    let host_src = match security::resolve_sandboxed_path(jail_root, &src) {
        Ok(path) => path,
        Err(err) => return map_security(err),
    };
    let host_dst = match security::resolve_creatable_path(jail_root, &dst) {
        Ok(path) => path,
        Err(err) => return map_security(err),
    };
    match std::fs::copy(&host_src, &host_dst) {
        Ok(_) => OK,
        Err(err) => map_io(err),
    }
}

fn http_fetch(frame: &HypercallFrame, mem: &mut [u8]) -> i32 {
    let url = match read_str(mem, frame.arg0, frame.arg1) {
        Ok(url) => url.to_string(),
        Err(code) => return code,
    };
    if reqwest::Url::parse(&url).is_err() {
        return ERR_HTTP;
    }

    let start = match usize::try_from(frame.arg2) {
        Ok(start) => start,
        Err(_) => return ERR_FAULT,
    };
    let cap = match usize::try_from(frame.arg3) {
        Ok(cap) => cap,
        Err(_) => return ERR_FAULT,
    };
    if start.checked_add(cap).is_none_or(|end| end > mem.len()) {
        return ERR_FAULT;
    }

    let client = match reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
    {
        Ok(client) => client,
        Err(err) => {
            tracing::warn!("http client: {err}");
            return ERR_HTTP;
        }
    };
    let response = match client.get(&url).send() {
        Ok(response) => response,
        Err(err) => {
            tracing::warn!("http fetch: {err}");
            return ERR_HTTP;
        }
    };
    if !response.status().is_success() {
        tracing::warn!("http status {}", response.status());
        return ERR_HTTP;
    }
    let body = match response.bytes() {
        Ok(body) => body,
        Err(err) => {
            tracing::warn!("http body: {err}");
            return ERR_HTTP;
        }
    };
    if body.len() > cap {
        return ERR_INVAL;
    }
    mem[start..start + body.len()].copy_from_slice(&body);
    i32::try_from(body.len()).unwrap_or(ERR_INVAL)
}

fn read_bytes(mem: &[u8], gpa: u64, len: u64) -> Result<&[u8], i32> {
    if len > MAX_BLOB {
        return Err(ERR_INVAL);
    }
    let start = usize::try_from(gpa).map_err(|_| ERR_FAULT)?;
    let len = usize::try_from(len).map_err(|_| ERR_FAULT)?;
    let end = start.checked_add(len).ok_or(ERR_FAULT)?;
    mem.get(start..end).ok_or(ERR_FAULT)
}

fn read_str(mem: &[u8], gpa: u64, len: u64) -> Result<&str, i32> {
    let bytes = read_bytes(mem, gpa, len)?;
    std::str::from_utf8(bytes).map_err(|_| ERR_INVAL)
}

fn map_security(err: SecurityError) -> i32 {
    tracing::warn!("sandbox: {err}");
    ERR_SECURITY
}

fn map_io(err: std::io::Error) -> i32 {
    tracing::warn!("io: {err}");
    ERR_IO
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hypercalls::HypercallFrame;
    use std::fs;

    fn jail() -> std::path::PathBuf {
        let path = std::env::temp_dir().join(format!(
            "matchbox_hc_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&path).unwrap();
        path
    }

    fn place(mem: &mut [u8], at: usize, bytes: &[u8]) -> (u64, u64) {
        mem[at..at + bytes.len()].copy_from_slice(bytes);
        (at as u64, bytes.len() as u64)
    }

    #[test]
    fn mkdir_write_and_copy() {
        let root = jail();
        let mut mem = vec![0u8; 4096];
        let (path_gpa, path_len) = place(&mut mem, 0x100, b"notes");

        let mut frame = HypercallFrame {
            command_id: HypercallCommand::Mkdir as u32,
            status: -99,
            arg0: path_gpa,
            arg1: path_len,
            arg2: 0,
            arg3: 0,
        };
        assert_eq!(dispatch(&mut frame, &mut mem, &root), OK);
        assert!(root.join("notes").is_dir());

        let (file_gpa, file_len) = place(&mut mem, 0x180, b"notes/a.txt");
        let (data_gpa, data_len) = place(&mut mem, 0x200, b"hello");
        frame.command_id = HypercallCommand::WriteFile as u32;
        frame.arg0 = file_gpa;
        frame.arg1 = file_len;
        frame.arg2 = data_gpa;
        frame.arg3 = data_len;
        assert_eq!(dispatch(&mut frame, &mut mem, &root), OK);
        assert_eq!(fs::read(root.join("notes/a.txt")).unwrap(), b"hello");

        let (dst_gpa, dst_len) = place(&mut mem, 0x220, b"notes/b.txt");
        frame.command_id = HypercallCommand::CopyFile as u32;
        frame.arg0 = file_gpa;
        frame.arg1 = file_len;
        frame.arg2 = dst_gpa;
        frame.arg3 = dst_len;
        assert_eq!(dispatch(&mut frame, &mut mem, &root), OK);
        assert_eq!(fs::read(root.join("notes/b.txt")).unwrap(), b"hello");

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn rejects_path_escape() {
        let root = jail();
        let mut mem = vec![0u8; 256];
        let (gpa, len) = place(&mut mem, 0, b"../outside");
        let mut frame = HypercallFrame {
            command_id: HypercallCommand::Mkdir as u32,
            status: 0,
            arg0: gpa,
            arg1: len,
            arg2: 0,
            arg3: 0,
        };
        assert_eq!(dispatch(&mut frame, &mut mem, &root), ERR_SECURITY);
        assert_eq!(frame.status, ERR_SECURITY);
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn rejects_pointer_outside_guest_ram() {
        let root = jail();
        let mut mem = vec![0u8; 64];
        let mut frame = HypercallFrame {
            command_id: HypercallCommand::Mkdir as u32,
            status: 0,
            arg0: 10_000,
            arg1: 4,
            arg2: 0,
            arg3: 0,
        };
        assert_eq!(dispatch(&mut frame, &mut mem, &root), ERR_FAULT);
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn http_rejects_bad_url_and_bad_buffer() {
        let root = jail();
        let mut mem = vec![0u8; 128];
        let (gpa, len) = place(&mut mem, 0, b"not a url");
        let mut frame = HypercallFrame {
            command_id: HypercallCommand::HttpFetch as u32,
            status: 0,
            arg0: gpa,
            arg1: len,
            arg2: 64,
            arg3: 32,
        };
        assert_eq!(dispatch(&mut frame, &mut mem, &root), ERR_HTTP);

        let (gpa, len) = place(&mut mem, 0, b"http://example.com/");
        frame.arg0 = gpa;
        frame.arg1 = len;
        frame.arg2 = 200;
        frame.arg3 = 32;
        assert_eq!(dispatch(&mut frame, &mut mem, &root), ERR_FAULT);
        let _ = fs::remove_dir_all(&root);
    }
}
