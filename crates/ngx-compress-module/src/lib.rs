//! NGINX dynamic module entrypoint for `ngx-compress-rs`.
//!
//! M1 installs an identity pass-through header and body filter to prove module
//! registration, filter ordering, configuration merge, and build integration
//! under both static and dynamic linking. Codec transformation arrives in M2.

mod http;
