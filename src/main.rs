mod iter_blocks;

use fuser::{FileAttr, Filesystem, FileType, MountOption, Request, ReplyAttr, ReplyData, ReplyDirectory, ReplyEmpty, ReplyEntry, ReplyOpen, ReplyWrite};
use libc::{EINVAL, EIO, ENOENT, ENOTSUP};
use std::env::args_os;
use std::error::Error;
use std::io::{Error as IoError, ErrorKind as IoErrorKind, Read, Seek, SeekFrom, Write};
use std::ffi::{OsStr, OsString};
use std::fs::{File, OpenOptions};
use std::path::Path;
use std::time::UNIX_EPOCH;

const BLOCK_SIZE: u64 = 4096;

fn main() {
    match main_r() {
        Ok(()) => {}
        Err(e) => {
            eprintln!("{}", e);
            std::process::exit(1);
        }
    }
}

fn main_r() -> Result<(), Box<dyn Error>> {
    // Read command line
    let mut args = args_os();
    match args.next() {
        None => return Err("Not enough arguments".into()),
        Some(_) => {}
    }
    let input_path: OsString = match args.next() {
        None => return Err("Not enough arguments".into()),
        Some(f) => f,
    };
    let mount_path = match args.next() {
        None => return Err("Not enough arguments".into()),
        Some(f) => f,
    };
    let diff_path = match args.next() {
        None => return Err("Not enough arguments".into()),
        Some(f) => f,
    };
    match args.next() {
        None => {}
        Some(_) => return Err("Too many arguments".into()),
    }

    mount(Path::new(&input_path), Path::new(&mount_path), Path::new(&diff_path))
        .map_err(|e| Box::new(e) as Box<dyn Error>)
}

fn mount(input_path: &Path, mount_path: &Path, diff_path: &Path) -> Result<(), IoError> {
    let options = vec![
        MountOption::RW,
        MountOption::FSName("fuse-cow-block".to_owned()),
        MountOption::DefaultPermissions,
    ];
    let filesystem = CowBlockFs::new(input_path, diff_path)?;
    fuser::mount2(filesystem, mount_path, &options)
}

struct CowBlockFs {
    input: File,
    diff: File,
    filename: OsString,
    file_size: u64,
    nblocks: u64,
    nbytes: u64,
}

const ROOT_ATTR: FileAttr = FileAttr {
    ino: 1,
    size: 0,
    blocks: 0,
    atime: UNIX_EPOCH,
    mtime: UNIX_EPOCH,
    ctime: UNIX_EPOCH,
    crtime: UNIX_EPOCH,
    kind: FileType::Directory,
    perm: 0o755,
    nlink: 2,
    uid: 1000,
    gid: 1000,
    rdev: 0,
    flags: 0,
    blksize: 512,
};

impl CowBlockFs {
    fn new(input_path: &Path, diff_path: &Path) -> Result<CowBlockFs, IoError> {
        let filename = input_path.file_name().ok_or(IoError::new(IoErrorKind::NotFound, "Invalid input filename"))?.to_owned();
        let metadata = std::fs::metadata(input_path)?;
        let file_size = metadata.len();

        let mut diff = OpenOptions::new().read(true).write(true).create(true).open(diff_path)?;

        // Measure the header, which is the index of the blocks
        let nblocks = (file_size - 1) / BLOCK_SIZE + 1;
        let nbytes = if nblocks < 1 << 32 {
            4
        } else {
            8
        };

        if file_size != 0 {
            let current_diff_len = diff.seek(SeekFrom::End(0))?;
            if current_diff_len == 0 {
                // Allocate space for the index
                diff.seek(SeekFrom::Start(nblocks * nbytes - 1))?;
                diff.write_all(b"\0")?;
            } else if current_diff_len < nblocks * nbytes {
                return Err(IoError::new(IoErrorKind::InvalidData, "Diff file is too small"));
            }
        }

        Ok(CowBlockFs {
            input: OpenOptions::new().read(true).write(true).open(input_path)?,
            diff,
            filename,
            file_size,
            nblocks,
            nbytes,
        })
    }

    fn file_attr(&mut self) -> FileAttr {
        FileAttr {
            ino: 2,
            size: self.file_size,
            blocks: (self.file_size - 1) / 512 + 1,
            atime: UNIX_EPOCH,
            mtime: UNIX_EPOCH,
            ctime: UNIX_EPOCH,
            crtime: UNIX_EPOCH,
            kind: FileType::RegularFile,
            perm: 0o755,
            nlink: 2,
            uid: 1000,
            gid: 1000,
            rdev: 0,
            flags: 0,
            blksize: 512,
        }
    }

    fn read_index(&mut self, block_num: u64) -> Result<Option<u64>, IoError> {
        if self.nbytes == 4 {
            self.diff.seek(SeekFrom::Start(block_num * 4))?;
            let mut data = [0u8; 4];
            self.diff.read_exact(&mut data)?;
            let data = 
                (data[0] << 24) as u64
                | (data[1] << 16) as u64
                | (data[2] << 8) as u64
                | data[3] as u64;
            if data == 0 {
                Ok(None)
            } else {
                Ok(Some(data - 1))
            }
        } else {
            self.diff.seek(SeekFrom::Start(block_num * 8))?;
            let mut data = [0u8; 8];
            self.diff.read_exact(&mut data)?;
            let data =
                (data[0] << 56) as u64
                | (data[1] << 48) as u64
                | (data[2] << 40) as u64
                | (data[3] << 32) as u64
                | (data[4] << 24) as u64
                | (data[5] << 16) as u64
                | (data[6] << 8) as u64
                | data[7] as u64;
            if data == 0 {
                Ok(None)
            } else {
                Ok(Some(data - 1))
            }
        }
    }

    fn do_read(&mut self, offset: u64, size: u64) -> Result<Vec<u8>, IoError> {
        todo!()
    }

    fn do_write(&mut self, offset: u64, data: &[u8]) -> Result<u32, IoError> {
        todo!()
    }
}

const ZERO: std::time::Duration = std::time::Duration::ZERO;

impl Filesystem for CowBlockFs {
    fn lookup(&mut self, _req: &Request, parent: u64, name: &OsStr, reply: ReplyEntry) {
        if parent == 1 && name == self.filename {
            reply.entry(&ZERO, &self.file_attr(), 0);
        } else {
            reply.error(ENOENT);
        }
    }

    fn readlink(&mut self, _req: &Request, ino: u64, reply: ReplyData) {
        match ino {
            1|2 => reply.error(EINVAL),
            _ => reply.error(ENOENT),
        }
    }

    fn getattr(&mut self, _req: &Request, ino: u64, reply: ReplyAttr) {
        match ino {
            1 => reply.attr(&ZERO, &ROOT_ATTR),
            2 => reply.attr(&ZERO, &self.file_attr()),
            _ => reply.error(ENOENT),
        }
    }

    fn open(&mut self, _req: &Request, _ino: u64, _flags: i32, reply: ReplyOpen) {
        reply.opened(0, 0);
    }

    fn opendir(&mut self, _req: &Request<'_>, _ino: u64, _flags: i32, reply: ReplyOpen) {
        reply.opened(0, 0);
    }

    fn readdir(
        &mut self,
        _req: &Request<'_>,
        ino: u64,
        _fh: u64,
        offset: i64,
        mut reply: ReplyDirectory,
    ) {
        let entries = match ino {
            1 => [
                (1, FileType::Directory, OsStr::new(".")),
                (1, FileType::Directory, OsStr::new("..")),
                (2, FileType::RegularFile, &self.filename),
            ],
            _ => {
                reply.error(ENOENT);
                return;
            }
        };
        for (i, entry) in entries.into_iter().enumerate().skip(offset as usize) {
            // ino, offset, kind, name
            if reply.add(entry.0, (i + 1) as i64, entry.1, entry.2) {
                break;
            }
        }
    }

    fn read(&mut self, _req: &Request, ino: u64, _fh: u64, offset: i64, size: u32, _flags: i32, _lock_owner: Option<u64>, reply: ReplyData) {
        if ino != 2 {
            reply.error(ENOENT);
            return;
        }

        let offset = offset as u64;
        let size = size as u64;
        match self.do_read(offset as u64, size as u64) {
            Ok(result) => reply.data(&result),
            Err(e) => {
                eprintln!("Read error: {}", e);
                reply.error(EIO);
            }
        }
    }

    fn write(&mut self, _req: &Request, ino: u64, _fh: u64, offset: i64, data: &[u8], write_flags: u32, flags: i32, _lock_owner: Option<u64>, reply: ReplyWrite) {
        if ino != 2 {
            reply.error(ENOENT);
            return;
        }

        match self.do_write(offset as u64, data) {
            Ok(bytes) => reply.written(bytes),
            Err(e) => {
                eprintln!("Write error: {}", e);
                reply.error(EIO);
            }
        }
    }

    fn flush(&mut self, _req: &Request, ino: u64, _fh: u64, _lock_owner: u64, reply: ReplyEmpty) {
        if ino == 1 {
            return;
        } else if ino != 2 {
            reply.error(ENOENT);
            return;
        }
        if let Err(e) = self.diff.sync_all() {
            reply.error(e.raw_os_error().unwrap_or(ENOTSUP));
        }
    }

    fn fsync(&mut self, _req: &Request, ino: u64, _fh: u64, datasync: bool, reply: ReplyEmpty) {
        if ino == 1 {
            return;
        } else if ino != 2 {
            reply.error(ENOENT);
            return;
        }
        let res = if datasync {
            self.diff.sync_data()
        } else {
            self.diff.sync_all()
        };
        if let Err(e) = res {
            reply.error(e.raw_os_error().unwrap_or(EIO));
        }
    }
}
