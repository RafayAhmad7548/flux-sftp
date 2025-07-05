mod sftp;
use std::{net::SocketAddr, sync::Arc, time::Duration};
use russh::{keys::ssh_key::{rand_core::OsRng, PublicKey}, server::{Auth, Handler as SshHandler, Msg, Server, Session}, Channel, ChannelId};
use sftp::SftpSession;


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
    type Error = russh::Error;

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
            let jail_dir = format!("/srv/sftp/{}", self.user.take().unwrap());
            let sftp_handler = SftpSession::new(jail_dir);
            russh_sftp::server::run(self.channel.take().ok_or(Self::Error::WrongChannel)?.into_stream(), sftp_handler).await;
        }
        else {
            session.channel_failure(channel_id)?;
        }
        Ok(())
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
