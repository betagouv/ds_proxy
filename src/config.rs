use sodiumoxide::crypto::pwhash;
use sodiumoxide::crypto::pwhash::scryptsalsa208sha256::Salt;
use sodiumoxide::crypto::secretstream::xchacha20poly1305::*;
use std::env;
use actix_web::http::Uri;
use std::fs::File;
use std::io;
use std::io::prelude::*;
use std::path::Path;
use std::error::Error;
use super::args;
use sodiumoxide::crypto::pwhash::argon2i13::{pwhash_verify, HashedPassword};

pub type DsKey = Key;

// match nginx default (proxy_buffer_size in ngx_stream_proxy_module)
pub const DEFAULT_CHUNK_SIZE: usize = 16 * 1024;

#[derive(Debug, Clone)]
pub struct Config {
    pub upstream_base_url: Option<String>,
    pub noop: bool,
    pub password: String,
    pub salt: String,
    pub chunk_size: usize,
}

impl Config {
    pub fn create_config(args: &args::Args) -> Config {
        let password = match &args.arg_password_file {
            Some(password_file) => read_password(password_file),
            None => env::var("DS_PASSWORD").expect("Missing password, use DS_PASSWORD env or --password-file cli argument")
        };

        ensure_valid_password(&password);

        let salt = match &args.arg_salt {
            Some(salt) => salt.to_string(),
            None => env::var("DS_SALT").expect("Missing salt, use DS_SALT env or --salt cli argument").to_string()
        };

        let chunk_size = match &args.arg_chunk_size {
            Some(chunk_size) => chunk_size.clone(),
            None => match env::var("DS_CHUNK_SIZE") {
                Ok(chunk_str) => chunk_str.parse::<usize>().unwrap_or(DEFAULT_CHUNK_SIZE),
                _ => DEFAULT_CHUNK_SIZE
            }
        };

        let upstream_base_url = if args.cmd_proxy {
            match &args.arg_upstream_url {
                Some(upstream_url) => Some(upstream_url.to_string()),
                None => Some(env::var("DS_UPSTREAM_URL").expect("Missing upstream_url, use DS_UPSTREAM_URL env or --upstream-url cli argument").to_string())
            }
        } else {
            None
        };

        Config{
            password: password,
            salt: salt,
            chunk_size: chunk_size,
            upstream_base_url: upstream_base_url,
            noop: args.flag_noop,
        }
    }

    pub fn create_key(self) -> Result<Key, &'static str> {
        if let Some(salt) = Salt::from_slice(&self.salt.as_bytes()[..]) {
            let mut raw_key = [0u8; KEYBYTES];

            pwhash::derive_key(
                &mut raw_key,
                self.password.as_bytes(),
                &salt,
                pwhash::OPSLIMIT_INTERACTIVE,
                pwhash::MEMLIMIT_INTERACTIVE,
                )
                .unwrap();

            Ok(Key(raw_key))
        } else {
            Err("Unable to derive a key from the salt")
        }
    }

    pub fn create_url(&self, uri: &Uri) -> String {
        format!("{}{}", self.upstream_base_url.clone().unwrap(), uri)
    }
}

fn read_password(path_string: &str) -> String {
    let path = Path::new(path_string);
    let display = path.display();

    let file = match File::open(&path) {
        Err(why) => panic!("couldn't open {}: {}", display, why.description()),
        Ok(file) => file,
    };

    let reader = io::BufReader::new(file);
    reader.lines().nth(0).unwrap().unwrap()
}


fn ensure_valid_password(password: &str) {
    match std::fs::read("hash.key") {
        Err(_) => {
            panic!("hash.key not found");
        },
        Ok(file) => {
            let hash = HashedPassword::from_slice(&file[..]);

            if !pwhash_verify(&hash.unwrap(), password.clone().trim_end().as_bytes()) {
                panic!("Incorrect password, aborting");
            }

        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_key_creation() {
        let passwd = "Correct Horse Battery Staple".to_string();
        let salt = "abcdefghabcdefghabcdefghabcdefgh".to_string();
        let config_ok = Config{ salt: salt, password: passwd, chunk_size: 512, noop: false, upstream_base_url: None };

        assert_eq!(true, config_ok.create_key().is_ok());
    }
}
