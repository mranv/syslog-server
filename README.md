# SysLog Server

A production-ready, multithreaded SysLog server implementation in Rust with support for metrics, monitoring, and CSV logging.

## Features

This SysLog server provides robust functionality for enterprise environments:

- Asynchronous processing using Tokio
- Prometheus metrics integration
- CSV logging with proper error handling
- Configurable buffer sizes and ports
- Production-grade logging with tracing
- Cross-platform support

## Quick Start

### Prerequisites

- Rust 1.70 or higher
- Cargo package manager
- Linux/Unix environment (for optimal performance)

### Installation

Clone the repository:

```bash
git clone https://github.com/mranv/syslog-server.git
cd syslog-server
```

Build the release version:

```bash
cargo build --release
```

### Basic Usage

Run the server with default settings:

```bash
./target/release/syslog-server
```

This will start the server with:
- SysLog port: 514
- Metrics port: 9000
- Output file: syslog.csv

### Custom Configuration

Customize the server settings:

```bash
./target/release/syslog-server --port 515 --output /var/log/custom.csv --metrics-port 9090
```

## Examples

### Send Test Messages

Using logger (Linux):
```bash
logger -n localhost -P 514 "Test syslog message"
```

Using netcat:
```bash
echo "<13>MyApp: Test message" | nc -u localhost 514
```

### Monitor Metrics

View Prometheus metrics:
```bash
curl http://localhost:9000/metrics
```

Example output:
```
# HELP syslog_received_total Total number of logs received
# TYPE syslog_received_total counter
syslog_received_total 150

# HELP syslog_written_total Total number of logs written
# TYPE syslog_written_total counter
syslog_written_total 150

# HELP syslog_queue_size Current size of the log queue
# TYPE syslog_queue_size gauge
syslog_queue_size 0
```

### View Logs

The logs are stored in CSV format:
```bash
head -n 5 syslog.csv
Event_Time,Device_IP,SysLog,Severity,Facility
"2024-03-29 10:15:23.456","192.168.1.100","MyApp: System started",6,1
"2024-03-29 10:15:24.789","192.168.1.101","DatabaseService: Connected",5,1
```

## Production Deployment

For production environments, consider using the provided systemd service:

```ini
[Unit]
Description=SysLog Server
After=network.target

[Service]
Type=simple
User=syslog
ExecStart=/usr/local/bin/syslog-server --port 514 --output /var/log/syslog.csv
Restart=always
LimitNOFILE=65535

[Install]
WantedBy=multi-user.target
```

## Contributing

Contributions are welcome! Please feel free to submit a Pull Request.

## License

MIT License

## Author

Anubhav Gain ([@mranv](https://github.com/mranv))

## Support

For support, please open an issue on the GitHub repository.