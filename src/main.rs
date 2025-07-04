use std::{collections::HashMap, net::SocketAddr, sync::Arc, time::Duration};

use russh::{keys::ssh_key::{rand_core::OsRng, PublicKey}, server::{Auth, Handler as SshHandler, Msg, Server, Session}, Channel, ChannelId, Error};
use russh_sftp::{protocol::{File, FileAttributes, Handle, Name, Status, StatusCode}, server::Handler as SftpHandler};


struct SftpServer;

impl Server for SftpServer {
    type Handler = SshSession;

    fn new_client(&mut self, _peer_addr: Option<SocketAddr>) -> Self::Handler {
        SshSession{ channel: None, user: None }
    }
}

struct SshSession {
    channel: Option<Channel<Msg>>,
    user: Option<String>
}

impl SshHandler for SshSession {
    type Error = Error;

    async fn auth_publickey_offered(
        &mut self,
        user: &str,
        public_key: &PublicKey,
    ) -> Result<Auth, Self::Error> {
        let _ = public_key;
        self.user = Some(user.to_string());
        Ok(Auth::Accept)
    }

    async fn auth_publickey(
        &mut self,
        user: &str,
        public_key: &PublicKey,
    ) -> Result<Auth, Self::Error> {
        let _ = user;
        let _ = public_key;
        Ok(Auth::Accept)
    }

    async fn channel_open_session(
        &mut self,
        channel: russh::Channel<Msg>,
        _session: &mut Session,
    ) -> Result<bool, Self::Error> {
        self.channel = Some(channel);
        Ok(true)
    }

    async fn channel_eof(
        &mut self,
        channel_id: ChannelId,
        session: &mut Session,
    ) -> Result<(), Self::Error> {
        session.close(channel_id)
    }

    async fn subsystem_request(
        &mut self,
        channel_id: ChannelId,
        name: &str,
        session: &mut Session,
    ) -> Result<(), Self::Error> {
        if name == "sftp" {
            session.channel_success(channel_id)?;
            let root_dir = format!("/srv/sftp/{}", self.user.take().unwrap());
            let sftp_handler = SftpSession { cwd: root_dir.clone(), root_dir, handle_map: HashMap::new() };
            russh_sftp::server::run(self.channel.take().ok_or(Self::Error::WrongChannel)?.into_stream(), sftp_handler).await;
        }
        else {
            session.channel_failure(channel_id)?;
        }
        Ok(())
    }
}

struct SftpSession {
    root_dir: String,
    cwd: String,
    handle_map: HashMap<String, bool>
}

impl SftpHandler for SftpSession {
    type Error = StatusCode;

    fn unimplemented(&self) -> Self::Error {
        Self::Error::OpUnsupported
    }

    async fn realpath(
        &mut self,
        id: u32,
        path: String,
    ) -> Result<Name, Self::Error> {
        let paths = path.split('/');
        for path_part in paths {
            match path_part {
                ".." => {
                    if self.cwd != self.root_dir {
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
        self.handle_map.insert(path.clone(), false);
        Ok(Handle { id, handle: path })
    }

    async fn readdir(
        &mut self,
        id: u32,
        handle: String,
    ) -> Result<Name, Self::Error> {
        if !self.handle_map.get(&handle).unwrap() {
            *self.handle_map.get_mut(&handle).unwrap() = true;
            return Ok(Name { id, files: vec![File::new("test", FileAttributes::default())] })
        }
        Err(StatusCode::Eof)
    }

    async fn close(
        &mut self,
        id: u32,
        handle: String,
    ) -> Result<Status, Self::Error> {
        self.handle_map.remove(&handle);
        Ok(Status {
            id,
            status_code: StatusCode::Ok,
            error_message: "Ok".to_string(),
            language_tag: "en-US".to_string(),
        })
    }

}



#[tokio::main]
async fn main() {

    let config = russh::server::Config {
        auth_rejection_time: Duration::from_secs(3),
        auth_rejection_time_initial: Some(Duration::from_secs(0)),
        keys: vec![
            russh::keys::PrivateKey::random(&mut OsRng, russh::keys::Algorithm::Ed25519).unwrap(),
        ],
        ..Default::default()
    };
    let mut server = SftpServer;

    server.run_on_address(Arc::new(config), ("0.0.0.0", 2222)).await.unwrap();
}
