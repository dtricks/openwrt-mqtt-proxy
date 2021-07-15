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
use simplelog::Config;
use simplelog::ConfigBuilder;
use simplelog::LevelFilter;
use simplelog::TermLogger;
use simplelog::{TerminalMode, WriteLogger};

use std::error::Error;
use std::net::Ipv4Addr;
use tokio::net::TcpListener;
use tokio::net::TcpStream;

use std::io;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let listener = TcpListener::bind((Ipv4Addr::new(0, 0, 0, 0), 9000)).await?;
    let cli = mqtt::AsyncClient::new("tcp://mqtt.eclipseprojects.io:1883")?;
    let conf = ConfigBuilder::new().set_time_format_str("%+").build();
    simplelog::CombinedLogger::init(vec![
        TermLogger::new(
            LevelFilter::Info,
            conf.clone(),
            TerminalMode::Mixed,
            ColorChoice::Auto,
        ),
        WriteLogger::new(LevelFilter::Info, conf, std::fs::File::create("proxy.log")?),
    ])?;
    log::info!("Initialized Logger");
    //println!("Initialized Logger");

    // Start an async operation and get the token for it.
    let tok = cli.connect(mqtt::ConnectOptions::new());
    tok.wait()?;

    loop {
        let cli = cli.clone();
        match listener.accept().await {
            Ok((socket, addr)) => {
                println!("new client: {:?}", addr);
                if !cli.is_connected() {
                    let tok = cli.reconnect();
                    tok.wait()?;
                }
                match handle_request(socket, cli).await {
                    Ok(_) => {}
                    Err(e) => eprintln!("Error: {}", e),
                };
            }
            Err(e) => eprintln!("couldn't get client: {:?}", e),
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
                println!("read {} bytes", n);

                let tok = cli.publish(
                    mqtt::message::MessageBuilder::new()
                        .topic("test")
                        .payload(&buf[0..n])
                        .qos(2)
                        .retained(false)
                        .finalize(),
                );
                tok.wait()?;
                println!("published");
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
