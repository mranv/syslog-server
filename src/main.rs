use std::fs::OpenOptions;
use std::io::BufWriter;
use std::net::UdpSocket;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use chrono::Local;
use clap::Parser;
use metrics::{describe_counter, describe_gauge, increment_counter, gauge};
use metrics_exporter_prometheus::PrometheusBuilder;
use serde::Serialize;
use tokio::sync::mpsc;
use tracing::{error, info, Level};
use tracing_subscriber::{self, fmt::format::FmtSpan};
use std::error::Error;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    #[arg(short, long, default_value = "514")]
    port: u16,

    #[arg(short, long, default_value = "syslog.csv")]
    output: PathBuf,

    #[arg(short, long, default_value = "9000")]
    metrics_port: u16,

    #[arg(short, long, default_value = "1000")]
    queue_size: usize,
}

#[derive(Debug, Serialize, Clone)]
struct SysLogEntry {
    event_time: String,
    device_ip: String,
    syslog: String,
    severity: u8,
    facility: u8,
}

struct LogHandler {
    output_path: PathBuf,
}

impl LogHandler {
    fn new(path: PathBuf) -> Self {
        // Initialize metrics descriptions
        describe_counter!("syslog_received_total", "Total number of logs received");
        describe_counter!("syslog_written_total", "Total number of logs written");
        describe_gauge!("syslog_queue_size", "Current size of the log queue");
        
        LogHandler {
            output_path: path,
        }
    }

    async fn handle_log(&self, source_ip: String, log_data: String) -> Result<(), Box<dyn Error>> {
        increment_counter!("syslog_received_total");
        
        let (facility, severity) = self.parse_priority(&log_data)?;
        let entry = SysLogEntry {
            event_time: Local::now().format("%Y-%m-%d %H:%M:%S%.3f").to_string(),
            device_ip: source_ip,
            syslog: log_data.replace('\n', "").trim().to_string(),
            severity,
            facility,
        };

        self.write_to_csv(entry).await?;
        increment_counter!("syslog_written_total");
        Ok(())
    }

    fn parse_priority(&self, log_data: &str) -> Result<(u8, u8), Box<dyn Error>> {
        let pri_start = log_data.find('<').ok_or("No priority found")?;
        let pri_end = log_data.find('>').ok_or("Malformed priority")?;
        let priority: u8 = log_data[pri_start + 1..pri_end].parse()?;
        Ok((priority >> 3, priority & 0x7))
    }

    async fn write_to_csv(&self, entry: SysLogEntry) -> Result<(), Box<dyn Error>> {
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.output_path)?;

        let needs_headers = file.metadata()?.len() == 0;
        let mut writer = csv::WriterBuilder::new()
            .has_headers(needs_headers)
            .double_quote(true)
            .from_writer(BufWriter::with_capacity(8192, file));

        writer.serialize(entry)?;
        writer.flush()?;
        Ok(())
    }
}

async fn run_metrics_server(port: u16) -> Result<(), Box<dyn Error>> {
    PrometheusBuilder::new()
        .with_http_listener(([0, 0, 0, 0], port))
        .install()?;
    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let args = Args::parse();

    // Initialize logging
    tracing_subscriber::fmt()
        .with_target(false)
        .with_thread_ids(true)
        .with_level(true)
        .with_span_events(FmtSpan::CLOSE)
        .with_max_level(Level::INFO)
        .init();

    info!("Starting SysLog server on port {}", args.port);

    // Initialize metrics server
    tokio::spawn(async move {
        if let Err(e) = run_metrics_server(args.metrics_port).await {
            error!("Metrics server error: {}", e);
        }
    });

    // Set up UDP socket
    let socket = UdpSocket::bind(format!("0.0.0.0:{}", args.port))?;
    socket.set_nonblocking(true)?;

    // Configure socket buffer size using OS-specific methods if needed
    #[cfg(unix)]
    {
        use socket2::{Socket, Domain, Type};
        let socket2 = Socket::new(Domain::IPV4, Type::DGRAM, None)?;
        socket2.set_recv_buffer_size(262_144)?;
    }

    let log_handler = Arc::new(LogHandler::new(args.output));
    
    // Channel for message passing between UDP receiver and processor
    let (tx, mut rx) = mpsc::channel::<(String, String)>(args.queue_size);

    // Spawn UDP receiver task
    let socket = Arc::new(socket);
    tokio::spawn({
        let socket = Arc::clone(&socket);
        async move {
            let mut buf = [0; 8192];
            loop {
                match socket.recv_from(&mut buf) {
                    Ok((size, addr)) => {
                        if let Ok(data) = String::from_utf8(buf[..size].to_vec()) {
                            if let Err(e) = tx.send((addr.ip().to_string(), data)).await {
                                error!("Failed to send to channel: {}", e);
                            }
                            gauge!("syslog_queue_size", tx.capacity() as f64);
                        }
                    }
                    Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        tokio::time::sleep(Duration::from_millis(10)).await;
                        continue;
                    }
                    Err(e) => error!("Socket receive error: {}", e),
                }
            }
        }
    });

    // Log processor task
    let handler = Arc::clone(&log_handler);
    while let Some((ip, data)) = rx.recv().await {
        let handler = Arc::clone(&handler);
        tokio::spawn(async move {
            if let Err(e) = handler.handle_log(ip, data).await {
                error!("Error processing log: {}", e);
            }
        });
    }

    Ok(())
}