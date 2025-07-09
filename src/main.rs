mod sftp;
mod config;

use std::{io::ErrorKind, net::SocketAddr, sync::Arc, time::Duration};
use config::{Config, DBConfig};
use russh::{keys::ssh_key::{rand_core::OsRng, PublicKey}, server::{Auth, Handler as SshHandler, Msg, Server, Session}, Channel, ChannelId};
use sftp::SftpSession;
use sqlx::{mysql::MySqlPoolOptions, postgres::PgPoolOptions, sqlite::SqlitePoolOptions, MySql, Pool, Postgres, Row, Sqlite};
use tokio::fs;

macro_rules! fetch_pub_key {
    ($pool:ident, $query:literal, $user:ident) => {
        {
            let row_res = sqlx::query($query)
                .bind($user)
                .fetch_one($pool).await;
            match row_res {
                Ok(row) => Some(row.get("public_key")),
                Err(_) => None
            }
        }
    };
}

struct SftpServer {
    pool: Arc<DBPool>,
    config: Arc<Config>
}

impl Server for SftpServer {
    type Handler = SshSession;

    fn new_client(&mut self, _peer_addr: Option<SocketAddr>) -> Self::Handler {
        let session_pool = self.pool.clone();
        let config = self.config.clone();
        SshSession { channel: None, user: None, pool: session_pool, config }
    }
}

struct SshSession {
    channel: Option<Channel<Msg>>,
    user: Option<String>,
    pool: Arc<DBPool>,
    config: Arc<Config>
}

impl SshHandler for SshSession {
    type Error = russh::Error;

    async fn auth_publickey_offered(
        &mut self,
        user: &str,
        public_key: &PublicKey,
    ) -> Result<Auth, Self::Error> {
        self.user = Some(user.to_string());

        let offered_key = public_key.to_string();
        
        let stored_key_opt: Option<String> = match &*self.pool {
            DBPool::Sqlite(pool) => fetch_pub_key!(pool, "SELECT * FROM users WHERE username = ?", user),
            DBPool::Postgres(pool) => fetch_pub_key!(pool, "SELECT * FROM users WHERE username = $1", user),
            DBPool::Mysql(pool) => fetch_pub_key!(pool, "SELECT * FROM users WHERE username = ?", user)
        };

        if let Some(stored_key) = stored_key_opt {
            if stored_key == offered_key {
                Ok(Auth::Accept)
            }
            else {
                println!("invalid key");
                Ok(Auth::reject())
            }
        }
        else {
            println!("user not found");
            Ok(Auth::reject())
        }

    }

    async fn auth_publickey(
        &mut self,
        _user: &str,
        _public_key: &PublicKey,
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
            let jail_dir = format!("{}/{}", self.config.general.jail_dir, self.user.as_ref().unwrap());
            let sftp_handler = SftpSession::new(jail_dir);
            russh_sftp::server::run(self.channel.take().ok_or(Self::Error::WrongChannel)?.into_stream(), sftp_handler).await;
        }
        else {
            session.channel_failure(channel_id)?;
        }
        Ok(())
    }
}


enum DBPool {
    Sqlite(Pool<Sqlite>),
    Postgres(Pool<Postgres>),
    Mysql(Pool<MySql>)
}


#[tokio::main]
async fn main() -> Result<(), sqlx::Error> {

    const CONFIG_PATH: &str = "/etc/flux-sftp/config.toml";
    let config: Arc<Config>;
    match fs::read_to_string(CONFIG_PATH).await {
        Ok(toml) => {
            match toml::from_str::<Config>(&toml) {
                Ok(c) => config = Arc::new(c),
                Err(e) => {
                    println!("error parsing config file: {}\n please make sure config file is valid", e);
                    return Ok(())
                }
            }

        }
        Err(e) => {
            match e.kind() {
                ErrorKind::NotFound => println!("config file not found, please ensure config file is present at: {}", CONFIG_PATH),
                _ => println!("error occured reading config file: {}", e)
            }
            return Ok(())
        }
    }

    let url = match &config.database {
        DBConfig::Sqlite { path } => format!("sqlite:{}", path),
        DBConfig::Postgres { host, port, user, password, dbname }  => format!("postgres://{}:{}@{}:{}/{}", user, password, host, port, dbname),
        DBConfig::Mysql { host, port, user, password, dbname } => format!("mysql://{}:{}@{}:{}/{}", user, password, host, port, dbname),
    };

    let pool = match &config.database {
        DBConfig::Sqlite { .. } => DBPool::Sqlite(SqlitePoolOptions::new().max_connections(3).connect(&url).await?),
        DBConfig::Postgres { .. } => DBPool::Postgres(PgPoolOptions::new().max_connections(3).connect(&url).await?),
        DBConfig::Mysql { .. } => DBPool::Mysql(MySqlPoolOptions::new().max_connections(3).connect(&url).await?)
    };

    let mut server = SftpServer { pool: Arc::new(pool), config: config.clone() };

    let russh_config = russh::server::Config {
        auth_rejection_time: Duration::from_secs(3),
        auth_rejection_time_initial: Some(Duration::from_secs(0)),
        keys: vec![
            russh::keys::PrivateKey::random(&mut OsRng, russh::keys::Algorithm::Ed25519).unwrap(),
        ],
        ..Default::default()
    };

    server.run_on_address(Arc::new(russh_config), (&config.general.listen_address as &str, config.general.port)).await.unwrap();
    Ok(())
}
