@ECHO off
SET RUST_BACKTRACE=1
SET OPENSSL_DIR=C:\Program Files\OpenSSL-Win64
SET OPENSSL_STATIC=1
cargo run -- -C D:\Dokumente\Developement\Rust\haendlerspiel-backend\cert\haendler.crt -K D:\Dokumente\Developement\Rust\haendlerspiel-backend\cert\haendler.key
