use std::{net::SocketAddr, sync::Arc, time::Duration};

use russh::{keys::ssh_key::{rand_core::OsRng, PublicKey}, server::{Auth, Handler as SshHandler, Msg, Server, Session}, Channel, ChannelId, Error};
use russh_sftp::{protocol::{Name, StatusCode}, server::Handler as SftpHandler};


struct SftpServer;

impl Server for SftpServer {
    type Handler = SshSession;

    fn new_client(&mut self, _peer_addr: Option<SocketAddr>) -> Self::Handler {
        SshSession{ channel: None }
    }
}

struct SshSession {
    channel: Option<Channel<Msg>>
}

impl SshHandler for SshSession {
    type Error = Error;

    async fn auth_publickey_offered(
            &mut self,
            user: &str,
            public_key: &PublicKey,
        ) -> Result<Auth, Self::Error> {
        Ok(Auth::Accept)
    }

    async fn auth_publickey(
            &mut self,
            user: &str,
            public_key: &PublicKey,
        ) -> Result<Auth, Self::Error> {
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
            let sftp_handler = SftpSession {};
            russh_sftp::server::run(self.channel.take().ok_or(Self::Error::WrongChannel)?.into_stream(), sftp_handler).await;
        }
        else {
            session.channel_failure(channel_id)?;
        }
        Ok(())
    }
}

struct SftpSession;

impl SftpHandler for SftpSession {
    type Error = StatusCode;

    fn unimplemented(&self) -> Self::Error {
        Self::Error::OpUnsupported
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
