#[macro_use]
extern crate log;

use std::net::Ipv4Addr;
use std::pin::pin;
use std::{env, io};

use futures::stream::StreamExt;
use ip_roam::{Address, Addresses, Connection};

use app::App;

mod app;

fn parse_addr(am: &Address, if_name: &str, only_prefix: bool) -> Option<(String, Ipv4Addr)> {
    let label = am.label();
    if if only_prefix {
        label.starts_with(if_name)
    } else {
        label == if_name
    } {
        Some((label.to_string(), *am.addr()))
    } else {
        None
    }
}

async fn find_addr(
    addresses: Addresses,
    if_name: &str,
    only_prefix: bool,
) -> Option<(String, Ipv4Addr)> {
    let mut addrs = pin!(addresses.stream());
    while let Some(am) = addrs.next().await {
        let r = parse_addr(&am, if_name, only_prefix);
        if r.is_some() {
            return r;
        }
    }
    None
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> io::Result<()> {
    if env::var("RUST_LOG").is_err() {
        env::set_var("RUST_LOG", "info");
    }
    if env::var("PRETTY_ENV_LOGGER_COMPACT").is_err() {
        env::set_var("PRETTY_ENV_LOGGER_COMPACT", "1");
    }
    pretty_env_logger::init_timed();

    let app = App::new();
    let mut if_pattern = app.if_name.as_str();

    let only_prefix = if_pattern.ends_with('*');
    if only_prefix {
        if_pattern = &if_pattern[..if_pattern.len() - 1];
    }

    let c = Connection::new()?;

    let h = c.handle;
    tokio::spawn(c.conn);

    if let Some((if_name, addr)) = find_addr(h.addresses, if_pattern, only_prefix).await {
        info!("{}: {}", if_name, addr);
        app.initialize(addr).await;
    } else {
        info!("{}: no address", if_pattern);
        app.initialize(Ipv4Addr::UNSPECIFIED).await;
    }

    let mut msgs = pin!(h.monitor.stream());
    while let Some(msg) = msgs.next().await {
        let am = msg.addr();
        if let Some((if_name, addr)) = parse_addr(am, if_pattern, only_prefix) {
            let enter = msg.is_new();
            if enter {
                info!("new: {}: {}", if_name, addr);
            } else {
                info!("del: {}: {}", if_name, addr);
            }
            app.notify(addr, enter).await;
        }
    }
    Err(io::Error::from(io::ErrorKind::ConnectionAborted))
}
