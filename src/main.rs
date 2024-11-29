use std::fs::{File, OpenOptions};
use std::io::{BufWriter, Write};
use std::net::UdpSocket;
use std::sync::Arc;
use std::thread;
use std::time::SystemTime;
use chrono::{DateTime, Local};
use csv::WriterBuilder;
use serde::Serialize;
use std::error::Error;

#[derive(Debug, Serialize)]
struct SysLogEntry {
    event_time: String,
    device_ip: String,
    syslog: String,
}

struct LogHandler {
    output_path: String,
}

impl LogHandler {
    fn new(path: String) -> Self {
        LogHandler {
            output_path: path,
        }
    }

    fn handle_log(&self, source_ip: String, log_data: String) -> Result<(), Box<dyn Error>> {
        let entry = SysLogEntry {
            event_time: Local::now().format("%Y-%m-%d %H:%M:%S").to_string(),
            device_ip: source_ip,
            syslog: log_data.replace('\n', "").trim().to_string(),
        };

        self.write_to_csv(entry)?;
        Ok(())
    }

    fn write_to_csv(&self, entry: SysLogEntry) -> Result<(), Box<dyn Error>> {
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.output_path)?;

        let needs_headers = file.metadata()?.len() == 0;
        let mut writer = WriterBuilder::new()
            .has_headers(needs_headers)
            .from_writer(BufWriter::new(file));

        writer.serialize(entry)?;
        writer.flush()?;
        Ok(())
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    let socket = UdpSocket::bind("0.0.0.0:514")?;
    println!("SysLog server listening on port 514...");

    let log_handler = Arc::new(LogHandler::new("syslog.csv".to_string()));
    let mut buf = [0; 4096];

    loop {
        match socket.recv_from(&mut buf) {
            Ok((size, addr)) => {
                let data = String::from_utf8_lossy(&buf[..size]).to_string();
                let source_ip = addr.ip().to_string();
                let handler = Arc::clone(&log_handler);

                thread::spawn(move || {
                    if let Err(e) = handler.handle_log(source_ip, data) {
                        eprintln!("Error handling log: {}", e);
                    }
                });
            }
            Err(e) => eprintln!("Error receiving data: {}", e),
        }
    }
}