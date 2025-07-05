use std::{collections::HashMap, os::unix::fs::MetadataExt};

use chrono::{Local, TimeZone};
use russh_sftp::{protocol::{File, FileAttributes, Handle, Name, Status, StatusCode}, server::Handler as SftpHandler};

use tokio::fs::{self, ReadDir};

pub struct SftpSession {
    jail_dir: String,
    cwd: String,
    handles: HashMap<String, ReadDir>
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
        let path_parts = path.split('/');
        for path_part in path_parts {
            match path_part {
                ".." => {
                    if self.cwd != "/" {
                        if let Some(pos) = self.cwd.rfind('/') {
                            self.cwd.truncate(pos);
                        }
                    }
                },
                "." => {},
                _ => self.cwd.push_str(&format!("/{}", path_part))
            }
        }

        Ok(Name { id, files: vec![File::dummy(&self.cwd)] })
    }

    async fn opendir(
        &mut self,
        id: u32,
        path: String,
    ) -> Result<Handle, Self::Error> {
        let path = format!("{}/{}", self.jail_dir, path);
        match fs::read_dir(&path).await {
            Ok(entries) => {
                self.handles.insert(path.clone(), entries);
                Ok(Handle { id, handle: path })
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
        match self.handles.get_mut(&handle).unwrap().next_entry().await {
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

    async fn close(
        &mut self,
        id: u32,
        handle: String,
    ) -> Result<Status, Self::Error> {
        self.handles.remove(&handle);
        Ok(Status {
            id,
            status_code: StatusCode::Ok,
            error_message: "Ok".to_string(),
            language_tag: "en-US".to_string(),
        })
    }

}

