use std::{collections::HashMap, io::{ErrorKind, SeekFrom}, os::unix::fs::MetadataExt};

use chrono::{Local, TimeZone};
use regex::Regex;
use russh_sftp::{protocol::{Attrs, Data, File, FileAttributes, Handle as SftpHandle, Name, OpenFlags, Status, StatusCode}, server::Handler as SftpHandler};

use tokio::{fs::{self, OpenOptions, ReadDir}, io::{AsyncReadExt, AsyncSeekExt, AsyncWriteExt}};

macro_rules! match_expr {
    ($match:expr, $err_msg:literal, $id:ident) => {
        match $match {
            Ok(()) => Ok(Status { $id, status_code: StatusCode::Ok, error_message: "Ok".to_string(), language_tag: "en-US".to_string() }),
            Err(e) => {
                println!($err_msg, e);
                match e.kind() {
                    ErrorKind::NotFound => Ok(Status { $id, status_code: StatusCode::NoSuchFile, error_message: e.to_string(), language_tag: "en-US".to_string() }),
                    ErrorKind::PermissionDenied => Ok(Status { $id, status_code: StatusCode::PermissionDenied, error_message: e.to_string(), language_tag: "en-US".to_string() }),
                    ErrorKind::ConnectionReset => Ok(Status { $id, status_code: StatusCode::ConnectionLost, error_message: e.to_string(), language_tag: "en-US".to_string() }),
                    ErrorKind::NotConnected => Ok(Status { $id, status_code: StatusCode::NoConnection, error_message: e.to_string(), language_tag: "en-US".to_string() }),
                    _ => Ok(Status { $id, status_code: StatusCode::Failure, error_message: e.to_string(), language_tag: "en-US".to_string() })
                }
            }
        }
    };
}

enum Handle {
    Dir(ReadDir),
    File(fs::File)
}

pub struct SftpSession {
    jail_dir: String,
    cwd: String,
    handles: HashMap<String, Handle>
}

impl SftpSession {
    pub fn new(jail_dir: String) -> Self {
        SftpSession { jail_dir, cwd: String::from("/"), handles: HashMap::new() }
    }
}

impl SftpHandler for SftpSession {
    type Error = StatusCode;

    fn unimplemented(&self) -> Self::Error {
        StatusCode::OpUnsupported
    }
    
    async fn realpath(
        &mut self,
        id: u32,
        path: String,
    ) -> Result<Name, Self::Error> {
        println!("realpath called, path: {}", path);

        let re_1 = Regex::new(r"/[^/]+/\.\.").unwrap();
        self.cwd = re_1.replace_all(&path, "").to_string();
        while re_1.is_match(&self.cwd) {
            self.cwd = re_1.replace_all(&self.cwd, "").to_string();
        }
        
        let re_2 = Regex::new(r"/\.").unwrap();
        self.cwd = re_2.replace_all(&self.cwd, "").to_string();

        if self.cwd == "." || self.cwd == "" {
            self.cwd = String::from("/");
        }

        Ok(Name { id, files: vec![File::dummy(&self.cwd)] })
    }

    async fn open(
        &mut self,
        id: u32,
        filename: String,
        pflags: OpenFlags,
        _attrs: FileAttributes,
    ) -> Result<SftpHandle, Self::Error> {
        println!("open called, path: {}", filename);
        println!("pflags raw: {:b}", pflags.bits());
        println!("pflags: read: {}, write: {}, append: {}, create: {}, truncate: {}", pflags.contains(OpenFlags::READ), pflags.contains(OpenFlags::WRITE), pflags.contains(OpenFlags::APPEND), pflags.contains(OpenFlags::CREATE), pflags.contains(OpenFlags::TRUNCATE));
        let path = format!("{}{}", self.jail_dir, filename);
        if pflags.contains(OpenFlags::EXCLUDE) && fs::metadata(&path).await.is_ok() {
            return Err(StatusCode::Failure)
        }
        let mut options = OpenOptions::new();
            options
            .read(pflags.contains(OpenFlags::READ))
            .write(pflags.contains(OpenFlags::WRITE))
            .append(pflags.contains(OpenFlags::APPEND))
            .create(pflags.contains(OpenFlags::CREATE))
            .truncate(pflags.contains(OpenFlags::TRUNCATE));
        match options.open(&path).await {
            Ok(file) =>  {
                self.handles.insert(filename.clone(), Handle::File(file));
                Ok(SftpHandle { id, handle: filename })
            }
            Err(e) => {
                println!("error opeing file: {}", e);
                match e.kind() {
                    ErrorKind::NotFound => Err(StatusCode::NoSuchFile),
                    ErrorKind::PermissionDenied => Err(StatusCode::PermissionDenied),
                    ErrorKind::ConnectionReset => Err(StatusCode::ConnectionLost),
                    ErrorKind::NotConnected => Err(StatusCode::NoConnection),
                    _ => Err(StatusCode::Failure)
                }
            }
        }
    }

    async fn read(
        &mut self,
        id: u32,
        handle: String,
        offset: u64,
        len: u32,
    ) -> Result<Data, Self::Error> {
        if let Handle::File(file) = self.handles.get_mut(&handle).unwrap() {
            let mut buf = vec![0u8; len as usize];
            match file.seek(SeekFrom::Start(offset)).await {
                Ok(_) => {
                    match file.read(&mut buf).await {
                        Ok(bytes) => {
                            if bytes != 0 {
                                buf.truncate(bytes);
                                Ok(Data { id, data: buf })
                            }
                            else {
                                Err(StatusCode::Eof)
                            }
                        }
                        Err(e) => {
                            println!("Error in reading from offset in file: {}", e);
                            Err(StatusCode::Failure)
                        }
                    }
                }
                Err(e) => {
                    println!("Error in seeking offset in file: {}", e);
                    Err(StatusCode::Failure)
                }
            }
        }
        else {
            println!("handle is not a filehandle");
            Err(StatusCode::Failure)
        }
    }

    async fn write(
        &mut self,
        id: u32,
        handle: String,
        offset: u64,
        data: Vec<u8>,
    ) -> Result<Status, Self::Error> {
        println!("write called, offset: {}, data: {:?}", offset, String::from_utf8(data.clone()));
        if let Handle::File(file) = self.handles.get_mut(&handle).unwrap() {
            match file.seek(SeekFrom::Start(offset)).await {
                Ok(_) => {
                    match file.write_all(&data).await {
                        Ok(()) => {
                            Ok(Status {
                                id,
                                status_code: StatusCode::Ok,
                                error_message: "Ok".to_string(),
                                language_tag: "en-US".to_string(),
                            })
                        }
                        Err(e) => {
                            println!("Error in writing at offset in file: {}", e);
                            Ok(Status { id, status_code: StatusCode::Failure, error_message: e.to_string(), language_tag: "en-US".to_string() })
                        }
                    }   
                }
                Err(e) => {
                    println!("Error in seeking offset in file: {}", e);
                    Ok(Status { id, status_code: StatusCode::Failure, error_message: e.to_string(), language_tag: "en-US".to_string() })
                }
            }
        }
        else {
            println!("handle is not a filehandle");
            Err(StatusCode::Failure)
        }
    }

    async fn opendir(
        &mut self,
        id: u32,
        path: String,
    ) -> Result<SftpHandle, Self::Error> {
        println!("opendir called: {}", path);
        let path = format!("{}{}", self.jail_dir, path);
        match fs::read_dir(&path).await {
            Ok(entries) => {
                self.handles.insert(path.clone(), Handle::Dir(entries));
                Ok(SftpHandle { id, handle: path })
            }
            Err(e) => {
                println!("Error in reading dir: {}", e);
                Err(StatusCode::NoSuchFile)
            } 
        }
    }

    async fn readdir(
        &mut self,
        id: u32,
        handle: String,
    ) -> Result<Name, Self::Error> {
        println!("readdir called");
        if let Handle::Dir(handle) = self.handles.get_mut(&handle).unwrap() {
            match handle.next_entry().await {
                Ok(Some(entry)) => {
                    let metadata = entry.metadata().await.unwrap();
                    let dt = Local.timestamp_opt(metadata.mtime(), 0).unwrap();
                    let longname = format!("{} {} {}", metadata.size(), dt.format("%b %e %Y"), entry.file_name().to_string_lossy());
                    Ok(Name { id, files: vec![
                        File {
                            filename: entry.file_name().to_string_lossy().into(),
                            longname: longname,
                            attrs: FileAttributes {
                                size: Some(metadata.size()),
                                permissions: Some(metadata.mode()),
                                atime: Some(metadata.atime() as u32),
                                mtime: Some(metadata.mtime() as u32),
                                ..Default::default()
                            }
                        }
                    ] })
                }
                Ok(None) => Err(StatusCode::Eof),
                Err(e) => {
                    println!("Error listing file: {}", e);
                    Err(StatusCode::Failure)
                }
            }
        }
        else {
            println!("handle is not a dirhandle");
            Err(StatusCode::Failure)
        }
    }

    async fn close(
        &mut self,
        id: u32,
        handle: String,
    ) -> Result<Status, Self::Error> {
        println!("close called");
        self.handles.remove(&handle);
        Ok(Status {
            id,
            status_code: StatusCode::Ok,
            error_message: "Ok".to_string(),
            language_tag: "en-US".to_string(),
        })
    }

    async fn stat(
        &mut self,
        id: u32,
        path: String,
    ) -> Result<Attrs, Self::Error> {
        println!("stat called: {}", path);
        let path = format!("{}{}", self.jail_dir, path);
        match fs::metadata(path).await {
            Ok(metadata) => Ok(Attrs { id, attrs: FileAttributes {
                size: Some(metadata.size()),
                permissions: Some(metadata.mode()),
                atime: Some(metadata.atime() as u32),
                mtime: Some(metadata.mtime() as u32),
                ..Default::default()
            }}),
            Err(_) => Err(StatusCode::NoSuchFile)
        }
    }

    async fn lstat(
        &mut self,
        id: u32,
        path: String,
    ) -> Result<Attrs, Self::Error> {
        println!("lstat called: {}", path);
        let path = format!("{}{}", self.jail_dir, path);
        match fs::symlink_metadata(path).await {
            Ok(metadata) => Ok(Attrs { id, attrs: FileAttributes {
                size: Some(metadata.size()),
                permissions: Some(metadata.mode()),
                atime: Some(metadata.atime() as u32),
                mtime: Some(metadata.mtime() as u32),
                ..Default::default()
            }}),
            Err(_) => Err(StatusCode::OpUnsupported)
        }

    }

    async fn fstat(
        &mut self,
        id: u32,
        handle: String,
    ) -> Result<Attrs, Self::Error> {
        println!("fstat called: {}", handle);
        if let Handle::File(file) = self.handles.get(&handle).unwrap() {
            let metadata = file.metadata().await.unwrap();
            Ok(Attrs { id, attrs: FileAttributes {
                size: Some(metadata.size()),
                permissions: Some(metadata.mode()),
                atime: Some(metadata.atime() as u32),
                mtime: Some(metadata.mtime() as u32),
                ..Default::default()
            }})
        }
        else {
            println!("handle is not a filehandle");
            Err(StatusCode::Failure)
        }
        
    }

    async fn remove(
        &mut self,
        id: u32,
        filename: String,
    ) -> Result<Status, Self::Error> {
        println!("remove called: {}", filename);
        let path = format!("{}{}", self.jail_dir, filename);
        match_expr!(fs::remove_file(path).await, "error removing file: {}", id)
    }

    async fn mkdir(
        &mut self,
        id: u32,
        path: String,
        _attrs: FileAttributes,
    ) -> Result<Status, Self::Error> {
        println!("mkdir called: {}", path);
        let path = format!("{}{}", self.jail_dir, path);
        match_expr!(fs::create_dir(path).await, "error creating dir: {}", id)
    }

    async fn rmdir(
        &mut self,
        id: u32,
        path: String,
    ) -> Result<Status, Self::Error> {
        println!("rmdir called: {}", path);
        let path = format!("{}{}", self.jail_dir, path);
        match_expr!(fs::remove_dir(path).await, "error removing file: {}", id)
    }

    async fn rename(
        &mut self,
        id: u32,
        oldpath: String,
        newpath: String,
    ) -> Result<Status, Self::Error> {
        println!("rename called from: {}, to: {}", oldpath, newpath);
        let oldpath = format!("{}{}", self.jail_dir, oldpath);
        let newpath = format!("{}{}", self.jail_dir, newpath);
        match_expr!(fs::rename(oldpath, newpath).await, "error renaming file: {}", id)
    }

}

