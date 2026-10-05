//! USB HID devices through `/dev/hidrawN`: feature reports (the MSI board)
//! and output/input reports (the Cooler Master mouse).

use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Write};
use std::os::fd::AsRawFd;
use std::path::PathBuf;

use anyhow::{Context, anyhow};

/// Opens the hidraw node of the USB device `vendor:product`, on USB
/// interface `interface` when the device exposes several.
pub fn open(vendor: u16, product: u16, interface: Option<u8>) -> anyhow::Result<(File, PathBuf)> {
    let id = format!("HID_ID=0003:{vendor:08X}:{product:08X}");
    let mut entries: Vec<_> = fs::read_dir("/sys/class/hidraw")?.collect::<Result<_, _>>()?;
    entries.sort_by_key(|e| e.file_name());
    for entry in entries {
        let device = entry.path().join("device");
        let uevent = fs::read_to_string(device.join("uevent")).unwrap_or_default();
        if !uevent.lines().any(|line| line == id) {
            continue;
        }
        if let Some(interface) = interface {
            // device -> .../1-2.2:1.1/0003:2516:0101.0005; the parent names
            // the USB interface.
            let real = fs::canonicalize(&device)?;
            let parent = real
                .parent()
                .and_then(|p| p.file_name())
                .and_then(|n| n.to_str())
                .unwrap_or("");
            if !parent.ends_with(&format!(".{interface}")) {
                continue;
            }
        }
        let path = PathBuf::from("/dev").join(entry.file_name());
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .open(&path)
            .with_context(|| format!("cannot open {}", path.display()))?;
        return Ok((file, path));
    }
    Err(anyhow!("no HID device {vendor:04x}:{product:04x}"))
}

/// _IOC(_IOC_READ | _IOC_WRITE, 'H', nr, len), as in <linux/hidraw.h>.
fn hid_ioc(nr: u8, len: usize) -> libc::c_ulong {
    (3 << 30)
        | ((len as libc::c_ulong) << 16)
        | ((b'H' as libc::c_ulong) << 8)
        | nr as libc::c_ulong
}

/// Reads the feature report whose id is `buf[0]` into `buf`.
pub fn get_feature(file: &File, buf: &mut [u8]) -> io::Result<()> {
    // SAFETY: the request encodes buf's length; the kernel writes at most that.
    let res = unsafe {
        libc::ioctl(
            file.as_raw_fd(),
            hid_ioc(0x07, buf.len()) as _,
            buf.as_mut_ptr(),
        )
    };
    if res < 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}

/// Sends `buf` as a feature report; `buf[0]` is the report id.
pub fn set_feature(file: &File, buf: &[u8]) -> io::Result<()> {
    // SAFETY: as above; the kernel only reads buf.
    let res = unsafe {
        libc::ioctl(
            file.as_raw_fd(),
            hid_ioc(0x06, buf.len()) as _,
            buf.as_ptr(),
        )
    };
    if res < 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}

/// Sends an output report; `buf[0]` is the report id (0 when unnumbered).
pub fn write(mut file: &File, buf: &[u8]) -> io::Result<()> {
    file.write_all(buf)
}

/// Reads one input report into `buf`, or returns `None` after `timeout_ms`.
pub fn read_timeout(mut file: &File, buf: &mut [u8], timeout_ms: i32) -> io::Result<Option<usize>> {
    let mut poll = libc::pollfd {
        fd: file.as_raw_fd(),
        events: libc::POLLIN,
        revents: 0,
    };
    // SAFETY: one valid pollfd.
    let ready = unsafe { libc::poll(&mut poll, 1, timeout_ms) };
    if ready < 0 {
        return Err(io::Error::last_os_error());
    }
    if ready == 0 {
        return Ok(None);
    }
    file.read(buf).map(Some)
}
