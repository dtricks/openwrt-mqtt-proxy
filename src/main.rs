use mqtt::AsyncClient;
use paho_mqtt as mqtt;
use simplelog::ColorChoice;
use simplelog::ConfigBuilder;
use simplelog::LevelFilter;
use simplelog::TermLogger;
use simplelog::{TerminalMode, WriteLogger};

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
    let level_filter = str::parse::<LevelFilter>(&env::var("log_level")?)?;
    simplelog::CombinedLogger::init(vec![
        TermLogger::new(
            level_filter,
            conf.clone(),
            TerminalMode::Mixed,
            ColorChoice::Auto,
        ),
        WriteLogger::new(level_filter, conf, std::fs::File::create("proxy.log")?),
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
                log::debug!("new client: {:?}", addr);
                if !cli.is_connected() {
                    let tok = cli.reconnect();
                    tok.wait()?;
                }
                match handle_request(socket, cli).await {
                    Ok(_) => {}
                    Err(e) => log::error!("Error Handling Client: {:?}", e),
                };
            }
            Err(e) => log::error!("Couldn't connect to client: {:?}", e),
        }
    }
}

async fn handle_request(stream: TcpStream, cli: AsyncClient) -> Result<(), Box<dyn Error>> {
    let topic = format!("{}/{}", env::var("mqtt_topic")?, stream.peer_addr()?.ip());
    let time_at_req_start = std::time::Instant::now();
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
                        .topic(&topic)
                        .payload(&buf[0..n])
                        .qos(str::parse(&env::var("mqtt_qos")?).unwrap_or(0))
                        .retained(false)
                        .finalize(),
                );
                tok.wait()?;
                let elapsed = time_at_req_start.elapsed();
                let mut hex: String = String::new();
                for i in &buf[0..n] {
                    hex.push_str(&format!("{:02X}", i));
                }
                log::info!(
                    "Published to Broker: {} Topic: {}, Time since request start: {:?} Payload: {}",
                    &env::var("mqtt_broker")?,
                    &topic,
                    &elapsed,
                    &hex
                );
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
