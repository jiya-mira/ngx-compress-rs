//! NGINX dynamic module entrypoint for `ngx-compress-rs`.
//!
//! The header filter negotiates a content coding from the request's
//! `Accept-Encoding` and the location's `compress_*` configuration; the body
//! filter streams the response through the selected codec with free/busy chain
//! backpressure. Builds as both a static and a dynamic NGINX module.

mod conf;
mod filter;
mod profile;
mod registration;
mod select;
mod worker;
