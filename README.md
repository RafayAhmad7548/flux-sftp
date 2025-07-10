# Overview
FluxSFTP is a regular SFTP server with the following additions:
* custom authentication (virtual users)
* jail directories i.e. limit users to a certain directory

SQLite, PostgreSQL and MYSQL are supported for the database, Authentication can be done either via public key or password, password authentication uses bcrypt hashing with the default cost i.e. 12

# Installation
Start by downloading the tarball from the latest release from the [releases](https://forgejo.fluxgrid.pk/RafayAhmad/flux-sftp/releases), then extract the tarball as follows
```bash
tar -xzvf flux-sftp.tar.gz -C /
```
## Database
before you can run the server you need to setup a database, SQLite, PostgreSQL and MYSQL are supported. get a database server running or simply create a sqlite database file and configure the server as mentioned in the [configuration section](#configuration).
the database table should have a username field, and optionally public key and password for authentication, you can use either one authentication type or both, up to you. the public key field should not store the comment of the public key in the database, and for the passoword it should be hashed using bcrypt with default cost of 12.
***Note that registering users must be done manually by inserting records into the database as of now.***

## Enable and Start
after you have configured the database you can start the server as follows
```bash
sudo systemctl start flux-sftp
```
and if you want enable on startup
```bash
sudo systemctl enable flux-sftp
```

# Configuration
The configuration file is located at `/etc/flux-sftp/config.toml`, here is the default configuration:

```toml
[general]
listen_address = "0.0.0.0"
port = 2222
jail_dir = "/srv/sftp"
private_key_file = "~/.ssh/flux-sftp"

[database]
driver = "sqlite"
path = "/var/lib/flux-sftp/auth.db"
# host = "127.0.0.1"
# port = 3306
# user = "testuser"
# password = "testpass"
# dbname = "testdb"
table = "users"
username_field = "username"
public_key_field = "public_key"
# password_field = "password"
```

## Options
### general
* `listen_address` the address that the server listens on
* `port` the port that the server listens on
* `jail_dir` the directory that the all the users will be jailed into, each user will be jailed to the directory `jail_dir/{username}`, e.g. example_user will be jailed to `/srv/sftp/example_user` if `jail_dir` is set to `/srv/sftp`
* `private_key_file` the private key for the server, the server will use this to present its identity
### database
* `driver` which database to use, can be `sqlite`, `postgres`, `mysql`. in case of sqlite `path` option must be specified and for `postgres` and `mysql` the relevant options to connect to the database must be specified
* `path` path to sqlite db file, only specify if using `sqlite`
* `host` host address for the database, only specify if using `postgres` or `mysql`
* `port` port the database server is running on, only specify if using `postgres` or `mysql`
* `user` database user, only specify if using `postgres` or `mysql`
* `password` password for the database user, only specify if using `postgres` or `mysql`
* `dbname` name of the database to use, only specify if using `postgres` or `mysql`
* `table` the database table to query to get the hashed password or the public_key
* `username_field` name of the database column which stores the username
* `public_key_field` name of the database column which stores the public key, if this is not specifed this auth method will be disabled rejecting all requests
* `password_field` name of the database column which stores the hashed password, if this is not specifed this auth method will be disabled rejecting all requests

