//use std::net::{SocketAddr, TcpListener, TcpStream};
//
//fn handle_client(stream: TcpStream) {
//    println!("Incoming from {}", stream.peer_addr().unwrap());
//}
//
//fn main() -> std::io::Result<()> {
//    let listener = TcpListener::bind(SocketAddr::from(([0, 0, 0, 0], 8008)))?;
//    println!("Started listener on {}", listener.local_addr().unwrap());
//
//    // accept connections and process them serially
//    for stream in listener.incoming() {
//        handle_client(stream?);
//    }
//    Ok(())
//}

//use futures::future::Future;
use mqtt::AsyncClient;
use paho_mqtt as mqtt;
use simplelog::ColorChoice;
use simplelog::ConfigBuilder;
use simplelog::LevelFilter;
use simplelog::TermLogger;
use simplelog::{TerminalMode, WriteLogger};

use chrono::prelude::*;
use std::env;
use std::error::Error;
use std::net::Ipv4Addr;
use tokio::net::TcpListener;
use tokio::net::TcpStream;

use std::io;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    dotenv::dotenv().ok();
    let conf = ConfigBuilder::new().set_time_format_str("%+").build();
    let time = Local::now().format("%F_%X").to_string();
    let level_filter = str::parse::<LevelFilter>(&env::var("log_level")?)?;
    simplelog::CombinedLogger::init(vec![
        TermLogger::new(
            level_filter,
            conf.clone(),
            TerminalMode::Mixed,
            ColorChoice::Auto,
        ),
        WriteLogger::new(
            level_filter,
            conf,
            std::fs::File::create(format!("{}_{}", time, "proxy.log"))?,
        ),
    ])?;
    log::info!("Initialized Logger");

    let ip = str::parse::<Ipv4Addr>(&env::var("server_host")?)?;
    let listener = TcpListener::bind((ip, str::parse::<u16>(&env::var("server_port")?)?)).await?;

    log::info!("Server started at {}", listener.local_addr()?);
    let cli = mqtt::AsyncClient::new(env::var("mqtt_broker")?)?;

    // Start an async operation and get the token for it.
    let tok = cli.connect(mqtt::ConnectOptions::new());
    tok.wait()?;

    loop {
        let cli = cli.clone();
        match listener.accept().await {
            Ok((socket, addr)) => {
                log::info!("new client: {:?}", addr);
                if !cli.is_connected() {
                    let tok = cli.reconnect();
                    tok.wait()?;
                }
                match handle_request(socket, cli).await {
                    Ok(_) => {}
                    Err(e) => log::error!("Error: {}", e),
                };
            }
            Err(e) => log::error!("couldn't get client: {:?}", e),
        }
    }
}

async fn handle_request(stream: TcpStream, cli: AsyncClient) -> Result<(), Box<dyn Error>> {
    Ok(loop {
        stream.readable().await?;
        let mut buf = [0; 4096];

        // Try to read data, this may still fail with `WouldBlock`
        // if the readiness event is a false positive.
        match stream.try_read(&mut buf) {
            Ok(0) => break,
            Ok(n) => {
                log::debug!("read {} bytes", n);

                let tok = cli.publish(
                    mqtt::message::MessageBuilder::new()
                        .topic(env::var("mqtt_topic")?)
                        .payload(&buf[0..n])
                        .qos(str::parse(&env::var("mqtt_qos")?).unwrap_or(0))
                        .retained(false)
                        .finalize(),
                );
                tok.wait()?;
                let mut hex: String = String::new();
                for i in &buf[0..n] {
                    hex.push_str(&format!("{:02X}", i));
                }
                log::debug!("Published Payload: {}", hex);
            }
            Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                continue;
            }
            Err(e) => {
                return Err(e.into());
            }
        }
    })
}
