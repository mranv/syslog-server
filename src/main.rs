use std::fs::OpenOptions;
use std::io::BufWriter;
use std::net::UdpSocket;
use std::path::PathBuf;
use std::sync::Arc;
use chrono::Local;
use clap::Parser;
use metrics::{describe_counter, describe_gauge, increment_counter, gauge};
use metrics_exporter_prometheus::PrometheusBuilder;
use serde::Serialize;
use tokio::sync::mpsc;
use tracing::{error, info, Level};
use tracing_subscriber::{self, fmt::format::FmtSpan};
use syslog::{Facility, Formatter3164};
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
    message: String,
    severity: u8,
    facility: u8,
}

struct LogHandler {
    output_path: PathBuf,
}

impl LogHandler {
    fn new(path: PathBuf) -> Result<Self, Box<dyn Error>> {
        describe_counter!("syslog_received_total", "Total number of logs received");
        describe_counter!("syslog_written_total", "Total number of logs written");
        describe_gauge!("syslog_queue_size", "Current size of the log queue");
        
        Ok(LogHandler {
            output_path: path,
        })
    }

    async fn handle_log(&self, source_ip: String, message: String) -> Result<(), Box<dyn Error>> {
        increment_counter!("syslog_received_total");
        
        let (severity, facility) = parse_priority(&message)
            .unwrap_or((3, Facility::LOG_USER as u8)); // 3 is ERROR level

        // Create a formatted logger for this message
        let formatter = Formatter3164 {
            facility: Facility::LOG_SYSLOG,
            hostname: None,
            process: "syslog-server".into(),
            pid: std::process::id(),
        };

        let mut logger = syslog::unix(formatter)?;
        logger.err(&format!("[{}] {}", source_ip, message))?;
        
        let entry = SysLogEntry {
            event_time: Local::now().format("%Y-%m-%d %H:%M:%S%.3f").to_string(),
            device_ip: source_ip,
            message: message.replace('\n', "").trim().to_string(),
            severity,
            facility,
        };

        self.write_to_csv(entry).await?;
        increment_counter!("syslog_written_total");
        Ok(())
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

fn parse_priority(message: &str) -> Option<(u8, u8)> {
    let pri_start = message.find('<')?;
    let pri_end = message.find('>')?;
    let priority: u8 = message[pri_start + 1..pri_end].parse().ok()?;
    Some((priority & 0x7, priority >> 3))
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let args = Args::parse();

    tracing_subscriber::fmt()
        .with_target(false)
        .with_thread_ids(true)
        .with_level(true)
        .with_span_events(FmtSpan::CLOSE)
        .with_max_level(Level::INFO)
        .init();

    info!("Starting SysLog server on port {}", args.port);

    tokio::spawn(async move {
        if let Err(e) = PrometheusBuilder::new()
            .with_http_listener(([0, 0, 0, 0], args.metrics_port))
            .install() {
            error!("Metrics server error: {}", e);
        }
    });

    let socket = UdpSocket::bind(format!("0.0.0.0:{}", args.port))?;
    socket.set_nonblocking(true)?;

    let log_handler = Arc::new(LogHandler::new(args.output)?);
    let (tx, mut rx) = mpsc::channel(args.queue_size);

    let socket = Arc::new(socket);
    
    tokio::spawn({
        let socket = Arc::clone(&socket);
        let tx = tx.clone();
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
                    },
                    Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
                        continue;
                    },
                    Err(e) => {
                        error!("Socket receive error: {}", e);
                    }
                }
            }
        }
    });

    while let Some((ip, data)) = rx.recv().await {
        let handler = Arc::clone(&log_handler);
        tokio::spawn(async move {
            if let Err(e) = handler.handle_log(ip, data).await {
                error!("Error processing log: {}", e);
            }
        });
    }

    Ok(())
}