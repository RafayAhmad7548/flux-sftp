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
    pub(crate) jail_dir: String,
    pub(crate) private_key_file: String
}

#[derive(Serialize, Deserialize, Clone)]
pub(crate) struct DBConfig {
    #[serde(flatten)]
    pub(crate) driver: DriverConfig,
    #[serde(flatten)]
    pub(crate) common: CommonConfig
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(tag = "driver")]
pub(crate) enum DriverConfig {
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

#[derive(Serialize, Deserialize, Clone)]
pub(crate) struct CommonConfig {
    pub(crate) table: String,
    pub(crate) username_field: String,
    pub(crate) public_key_field: Option<String>,
    pub(crate) password_field: Option<String>
}


impl Default for Config {
    fn default() -> Self {
        Config {
            general: GeneralConfig {
                listen_address: String::from("0.0.0.0"),
                port: 2222,
                jail_dir: String::from("/srv/sftp"),
                private_key_file: String::from("~/.ssh/flux-sftp")
            },
            database: DBConfig {
                driver: DriverConfig::Sqlite {
                    path: String::from("/var/lib/flux-sftp/auth.db")
                },
                common: CommonConfig {
                    table: String::from("users"),
                    username_field: String::from("username"),
                    public_key_field: Some(String::from("public_key")),
                    password_field: None
                } 
            }
        }
    }
}
