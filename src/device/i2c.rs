//! Raw access to an i2c bus through `/dev/i2c-N`: the SMBus transfers the ENE
//! RAM controllers speak, and plain writes for the GPU's controller.

use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::os::fd::AsRawFd;
use std::path::PathBuf;

use anyhow::{Context, anyhow};

const I2C_SLAVE_FORCE: libc::c_ulong = 0x0706;
const I2C_SMBUS: libc::c_ulong = 0x0720;
const SMBUS_WRITE: u8 = 0;
const SMBUS_READ: u8 = 1;
const SMBUS_BYTE: u32 = 1;
const SMBUS_BYTE_DATA: u32 = 2;
const SMBUS_WORD_DATA: u32 = 3;

#[repr(C)]
struct SmbusIoctl {
    read_write: u8,
    command: u8,
    size: u32,
    data: *mut [u8; 34],
}

pub struct Bus {
    file: File,
    pub path: PathBuf,
}

impl Bus {
    /// The bus whose adapter name starts with `name`. Bus numbers change from
    /// one boot to the next; adapter names don't.
    pub fn find(name: &str) -> anyhow::Result<Bus> {
        for entry in fs::read_dir("/sys/class/i2c-dev")? {
            let entry = entry?;
            let adapter = fs::read_to_string(entry.path().join("name")).unwrap_or_default();
            if adapter.starts_with(name) {
                let path = PathBuf::from("/dev").join(entry.file_name());
                let file = OpenOptions::new()
                    .read(true)
                    .write(true)
                    .open(&path)
                    .with_context(|| format!("cannot open {}", path.display()))?;
                return Ok(Bus { file, path });
            }
        }
        Err(anyhow!("no i2c adapter named `{name}`"))
    }

    fn address(&self, addr: u8) -> io::Result<()> {
        // SAFETY: I2C_SLAVE_FORCE takes the address as a plain integer.
        let res = unsafe {
            libc::ioctl(
                self.file.as_raw_fd(),
                I2C_SLAVE_FORCE as _,
                addr as libc::c_ulong,
            )
        };
        if res < 0 {
            Err(io::Error::last_os_error())
        } else {
            Ok(())
        }
    }

    fn smbus(
        &self,
        addr: u8,
        read_write: u8,
        command: u8,
        size: u32,
        data: &mut [u8; 34],
    ) -> io::Result<()> {
        self.address(addr)?;
        let mut args = SmbusIoctl {
            read_write,
            command,
            size,
            data,
        };
        // SAFETY: `args` and the buffer it points to outlive the call.
        let res = unsafe { libc::ioctl(self.file.as_raw_fd(), I2C_SMBUS as _, &mut args) };
        if res < 0 {
            Err(io::Error::last_os_error())
        } else {
            Ok(())
        }
    }

    /// SMBus "receive byte": the quick presence probe OpenRGB uses.
    pub fn read_byte(&self, addr: u8) -> io::Result<u8> {
        let mut data = [0; 34];
        self.smbus(addr, SMBUS_READ, 0, SMBUS_BYTE, &mut data)?;
        Ok(data[0])
    }

    pub fn read_byte_data(&self, addr: u8, command: u8) -> io::Result<u8> {
        let mut data = [0; 34];
        self.smbus(addr, SMBUS_READ, command, SMBUS_BYTE_DATA, &mut data)?;
        Ok(data[0])
    }

    pub fn write_byte_data(&self, addr: u8, command: u8, value: u8) -> io::Result<()> {
        let mut data = [0; 34];
        data[0] = value;
        self.smbus(addr, SMBUS_WRITE, command, SMBUS_BYTE_DATA, &mut data)
    }

    pub fn write_word_data(&self, addr: u8, command: u8, value: u16) -> io::Result<()> {
        let mut data = [0; 34];
        data[..2].copy_from_slice(&value.to_le_bytes());
        self.smbus(addr, SMBUS_WRITE, command, SMBUS_WORD_DATA, &mut data)
    }

    /// A plain i2c write of `bytes`, with no SMBus framing.
    pub fn write(&self, addr: u8, bytes: &[u8]) -> io::Result<()> {
        self.address(addr)?;
        (&self.file).write_all(bytes)
    }
}
