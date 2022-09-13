mod iter_blocks;

use clap::{Arg, Command};
use fuser::{FileAttr, Filesystem, FileType, MountOption, Request, ReplyAttr, ReplyData, ReplyDirectory, ReplyEmpty, ReplyEntry, ReplyOpen, ReplyWrite};
use libc::{EINVAL, EIO, ENOENT};
use std::borrow::Cow;
use std::env::args_os;
use std::error::Error;
use std::io::{Error as IoError, ErrorKind as IoErrorKind, Read, Seek, SeekFrom, Write};
use std::ffi::{OsStr, OsString};
use std::fs::{File, OpenOptions};
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use iter_blocks::iter_blocks;

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

fn path_with_suffix(path: &Path, suffix: &str) -> Result<PathBuf, IoError> {
    let path = path.canonicalize()?;
    let mut filename = path.file_name().unwrap_or(OsStr::new("")).to_owned();
    filename.push(suffix);
    Ok(path.with_file_name(filename))
}

fn main_r() -> Result<(), Box<dyn Error>> {
    // Read command line
    let mut cli = Command::new("cowblock")
        .bin_name("cowblock")
        .version(env!("CARGO_PKG_VERSION"))
        .author(env!("CARGO_PKG_AUTHORS"))
        .about(env!("CARGO_PKG_DESCRIPTION"))
        .arg(
            Arg::new("input")
                .help("Input file name")
                .required(true)
                .takes_value(true)
                .allow_invalid_utf8(true)
        )
        .arg(
            Arg::new("mount")
                .help("Mount directory")
                .required(true)
                .takes_value(true)
                .allow_invalid_utf8(true)
        )
        .arg(
            Arg::new("diff")
                .long("diff")
                .help("Diff file, storing the overwritten blocks")
                .takes_value(true)
                .allow_invalid_utf8(true)
        )
        .arg(
            Arg::new("extra")
                .long("extra")
                .help("Extra file, storing the appended blocks")
                .takes_value(true)
                .allow_invalid_utf8(true)
        );
    let matches = cli.try_get_matches_from_mut(args_os())?;
    let input_path = Path::new(matches.value_of_os("input").unwrap());
    let mount_path = Path::new(matches.value_of_os("mount").unwrap());
    let diff_path = match matches.value_of_os("diff") {
        Some(name) => Cow::Borrowed(Path::new(name)),
        None => Cow::Owned(path_with_suffix(mount_path, "-diff")?),
    };
    let extra_path = match matches.value_of_os("extra") {
        Some(name) => Cow::Borrowed(Path::new(name)),
        None => Cow::Owned(path_with_suffix(mount_path, "-extra")?),
    };

    let options = vec![
        MountOption::RW,
        MountOption::FSName("fuse-cow-block".to_owned()),
        MountOption::DefaultPermissions,
    ];
    let filesystem = CowBlockFs::new(input_path, &diff_path, &extra_path)?;
    fuser::mount2(filesystem, mount_path, &options)
        .map_err(|e| Box::new(e) as Box<dyn Error>)
}

fn getuid() -> u32 {
    unsafe {
        libc::getuid()
    }
}

fn getgid() -> u32 {
    unsafe {
        libc::getgid()
    }
}

struct CowBlockFs {
    input: File,
    diff: File,
    extra: File,
    filename: OsString,
    file_size: u64,
    nblocks: u64,
    nbytes: u64,
}

impl CowBlockFs {
    fn new(input_path: &Path, diff_path: &Path, extra_path: &Path) -> Result<CowBlockFs, IoError> {
        let mut input = OpenOptions::new().read(true).write(true).open(input_path)?;
        let input_file_size = input.seek(SeekFrom::End(0))?;
        let mut diff = OpenOptions::new().read(true).write(true).create(true).open(diff_path)?;
        let mut extra = OpenOptions::new().read(true).write(true).create(true).open(extra_path)?;

        // Measure the header, which is the index of the blocks
        let nblocks = input_file_size / BLOCK_SIZE;
        println!(
            "Input file is {} bytes, that's {} full blocks of {} bytes",
            input_file_size,
            nblocks,
            BLOCK_SIZE,
        );
        let nbytes = if nblocks < 1 << 32 {
            4
        } else {
            8
        };
        println!(
            "Using {}-byte offsets in header, total header size {} bytes",
            nbytes,
            nblocks * nbytes,
        );

        if input_file_size != 0 {
            let current_diff_len = diff.seek(SeekFrom::End(0))?;
            if current_diff_len == 0 {
                // Allocate space for the index
                diff.seek(SeekFrom::Start(nblocks * nbytes - 1))?;
                diff.write_all(b"\0")?;
            } else if current_diff_len < nblocks * nbytes {
                return Err(IoError::new(IoErrorKind::InvalidData, "Diff file exists but is too small"));
            }
        }

        let mut extra_file_size = extra.seek(SeekFrom::End(0))?;

        // If the input file contains a partial last block
        // (ie the size is not a multiple of the block size)
        if input_file_size % BLOCK_SIZE != 0 && extra_file_size == 0 {
            // Copy last block to extra file
            let mut buf = vec![0u8; (input_file_size % BLOCK_SIZE) as usize];
            input.seek(SeekFrom::Start((input_file_size / BLOCK_SIZE) * BLOCK_SIZE))?;
            input.read_exact(&mut buf)?;
            extra.write_all(&buf)?;
            extra_file_size += buf.len() as u64;
        }
        let file_size = nblocks * BLOCK_SIZE + extra_file_size;

        let filename = input_path.file_name().ok_or(IoError::new(IoErrorKind::NotFound, "Invalid input filename"))?.to_owned();

        Ok(CowBlockFs {
            input,
            diff,
            extra,
            filename,
            file_size,
            nblocks,
            nbytes,
        })
    }

    fn folder_attr(&self) -> FileAttr {
        FileAttr {
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
            uid: getuid(),
            gid: getgid(),
            rdev: 0,
            flags: 0,
            blksize: 512,
        }
    }

    fn file_attr(&self) -> FileAttr {
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
            uid: getuid(),
            gid: getgid(),
            rdev: 0,
            flags: 0,
            blksize: 512,
        }
    }

    fn read_index(&mut self, block_num: u64) -> Result<Option<u64>, IoError> {
        let diff_block_num = if self.nbytes == 4 {
            self.diff.seek(SeekFrom::Start(block_num * 4))?;
            let mut data = [0u8; 4];
            self.diff.read_exact(&mut data)?;
            let data =
                (data[0] as u64) << 24
                | (data[1] as u64) << 16
                | (data[2] as u64) << 8
                | data[3] as u64;
            if data == 0 {
                return Ok(None);
            } else {
                data - 1
            }
        } else {
            self.diff.seek(SeekFrom::Start(block_num * 8))?;
            let mut data = [0u8; 8];
            self.diff.read_exact(&mut data)?;
            let data =
                (data[0] as u64) << 56
                | (data[1] as u64) << 48
                | (data[2] as u64) << 40
                | (data[3] as u64) << 32
                | (data[4] as u64) << 24
                | (data[5] as u64) << 16
                | (data[6] as u64) << 8
                | data[7] as u64;
            if data == 0 {
                return Ok(None);
            } else {
                data - 1
            }
        };
        let position = self.nbytes * self.nblocks + diff_block_num * BLOCK_SIZE;
        Ok(Some(position))
    }

    fn write_index(&mut self, block_num: u64, position: u64) -> Result<(), IoError> {
        if position < self.nbytes * self.nblocks
            || (position - self.nbytes * self.nblocks) % BLOCK_SIZE != 0
        {
            return Err(IoError::new(IoErrorKind::InvalidData, "Diff block has invalid location"));
        }
        let diff_block_num = (position - self.nbytes * self.nblocks) / BLOCK_SIZE;
        if self.nbytes == 4 {
            self.diff.seek(SeekFrom::Start(block_num * 4))?;
            let data = diff_block_num + 1;
            let data = [
                (data >> 24) as u8,
                (data >> 16) as u8,
                (data >> 8) as u8,
                data as u8,
            ];
            self.diff.write_all(&data)
        } else {
            self.diff.seek(SeekFrom::Start(block_num * 8))?;
            let data = diff_block_num + 1;
            let data = [
                (data >> 56) as u8,
                (data >> 48) as u8,
                (data >> 40) as u8,
                (data >> 32) as u8,
                (data >> 24) as u8,
                (data >> 16) as u8,
                (data >> 8) as u8,
                data as u8,
            ];
            self.diff.write_all(&data)
        }
    }

    fn do_read(&mut self, start: u64, size: u64) -> Result<Vec<u8>, IoError> {
        // Clamp read to file size
        let size = self.file_size.min(start + size) - start;

        let mut result = vec![0u8; size as usize];
        let mut blocks = iter_blocks(BLOCK_SIZE, start, size);
        while let Some(block) = blocks.next() {
            let result_slice = &mut result[
                block.offset as usize
                ..
                (block.offset + block.size()) as usize
            ];

            // Is this over the input file's length?
            if block.num() >= self.nblocks {
                // Read from extra file
                self.extra.seek(SeekFrom::Start(block.start - block.num() * BLOCK_SIZE))?;
                self.extra.read_exact(result_slice)?;
            } else {
                // Has this block been overwritten?
                match self.read_index(block.num())? {
                    None => {
                        // No, read from input file
                        self.input.seek(SeekFrom::Start(block.start))?;
                        self.input.read_exact(result_slice)?;
                    }
                    Some(position) => {
                        // Yes, read from diff file
                        self.diff.seek(SeekFrom::Start(position))?;
                        self.diff.read_exact(result_slice)?;
                    }
                }
            }
        }

        Ok(result)
    }

    fn do_write(&mut self, start: u64, data: &[u8]) -> Result<u32, IoError> {
        let mut blocks = iter_blocks(BLOCK_SIZE, start, data.len() as u64);
        while let Some(block) = blocks.next() {
            // Is this over the input file's length?
            if block.num() >= self.nblocks {
                // Write to extra file
                self.extra.seek(SeekFrom::Start(block.start - block.num() * BLOCK_SIZE))?;
                // As an optimization, write all the remaining blocks and stop,
                // rather than continuing to write the blocks one-by-one
                self.extra.write_all(&data[block.offset as usize..])?;
                self.file_size = self.file_size.max(block.start + data.len() as u64 - block.offset);
                break;
            } else {
                // Has this block been overwritten?
                match self.read_index(block.num())? {
                    Some(position) => {
                        // Yes, just write to diff file
                        self.diff.seek(SeekFrom::Start(position + block.start % BLOCK_SIZE))?;
                        self.diff.write_all(&data[block.offset as usize..block.offset as usize + block.size() as usize])?;
                    }
                    None => {
                        // No
                        // Allocate a block in diff file
                        let position = self.diff.seek(SeekFrom::End(0))?;
                        self.write_index(block.num(), position)?;

                        // Are we writing a whole block?
                        if block.size() == BLOCK_SIZE {
                            // Yes, just do it
                            self.diff.seek(SeekFrom::Start(position))?;
                            self.diff.write(&data[block.offset as usize..block.offset as usize + block.size() as usize])?;
                        } else {
                            // No, read the rest of the block from input file
                            let mut buf = [0u8; BLOCK_SIZE as usize];
                            self.input.seek(SeekFrom::Start(block.num() * BLOCK_SIZE))?;
                            self.input.read_exact(&mut buf)?;

                            // Put the new data in it
                            buf[(block.start - block.num() * BLOCK_SIZE) as usize..(block.end - block.num() * BLOCK_SIZE) as usize].clone_from_slice(&data[block.offset as usize..(block.offset + block.size()) as usize]);

                            // Write it to diff file
                            self.diff.seek(SeekFrom::Start(position))?;
                            self.diff.write_all(&buf)?;
                        }
                    }
                }
            }
        }

        Ok(data.len() as u32)
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
            1 => reply.attr(&ZERO, &self.folder_attr()),
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
        for (i, entry) in IntoIterator::into_iter(entries).enumerate().skip(offset as usize) {
            // ino, offset, kind, name
            if reply.add(entry.0, (i + 1) as i64, entry.1, entry.2) {
                break;
            }
        }
        reply.ok();
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

    fn write(&mut self, _req: &Request, ino: u64, _fh: u64, offset: i64, data: &[u8], _write_flags: u32, _flags: i32, _lock_owner: Option<u64>, reply: ReplyWrite) {
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
            eprintln!("Flush error: {}", e);
            reply.error(e.raw_os_error().unwrap_or(EIO));
        } else {
            reply.ok();
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
            eprintln!("Fsync error: {}", e);
            reply.error(e.raw_os_error().unwrap_or(EIO));
        } else {
            reply.ok();
        }
    }
}
