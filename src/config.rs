use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone)]
pub(crate) struct Config {
    pub(crate) general: GeneralConfig,
    pub(crate) database: DBConfig
}

#[derive(Serialize, Deserialize, Clone)]
pub(crate) struct GeneralConfig {
    pub(crate) listen_address: String,
    pub(crate) port: u16,
    pub(crate) jail_dir: String
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(tag = "driver")]
pub(crate) enum DBConfig {
    #[serde(rename = "sqlite")]
    Sqlite {
        path: String
    },
    #[serde(rename = "postgres")]
    Postgres {
        host: String,
        port: u16,
        user: String,
        password: String,
        dbname: String
    },
    #[serde(rename = "mysql")]
    Mysql {
        host: String,
        port: u16,
        user: String,
        password: String,
        dbname: String
    }
}


impl Default for Config {
    fn default() -> Self {
        Config {
            general: GeneralConfig {
                listen_address: String::from("0.0.0.0"),
                port: 2222,
                jail_dir: String::from("/srv/sftp")
            },
            database: DBConfig::Sqlite {
                path: String::from("/var/lib/flux-sftp/auth.db")
            }
        }
    }
}
