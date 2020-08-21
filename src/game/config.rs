use std::fs::File;
use std::io::{self, BufReader};
use std::path::PathBuf;

use structopt::StructOpt;

use tokio_rustls::rustls::internal::pemfile::{certs, rsa_private_keys};
use tokio_rustls::rustls::{Certificate, PrivateKey};

#[derive(StructOpt, Debug)]
#[structopt(name = "haendlerspiel")]
pub struct Options {
  /// Path to TLS certificate
  #[structopt(short = "C", long = "tls-cert", parse(from_os_str))]
  cert: PathBuf,

  /// Path to TLS key
  #[structopt(short = "K", long = "tls-key", parse(from_os_str))]
  key: PathBuf,
}

impl Options {
  pub fn load(self) -> io::Result<(Vec<Certificate>, PrivateKey)> {
    Ok((
      certs(&mut BufReader::new(File::open(self.cert)?))
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "invalid certificate"))?,
      rsa_private_keys(&mut BufReader::new(File::open(self.key)?))
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "invalid rsa key"))?
        .remove(0),
    ))
  }
}
