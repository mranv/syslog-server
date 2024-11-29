# SysLog Server

A production-ready SysLog server implementation in Rust using the standard syslog crate. This server provides native syslog integration, metrics monitoring, and CSV logging capabilities.

## Features

The server implements comprehensive logging functionality:

- Native syslog protocol support
- Standard facility and severity level handling
- Prometheus metrics integration
- CSV logging with structured output
- Asynchronous processing using Tokio
- Production-grade monitoring

## Installation

Ensure you have Rust installed, then:

```bash
git clone https://github.com/mranv/syslog-server.git
cd syslog-server
cargo build --release
```

## Usage

Start the server with default settings:

```bash
./target/release/syslog-server
```

Configure custom ports and output:

```bash
./target/release/syslog-server --port 514 --output /var/log/syslog.csv --metrics-port 9000
```

## Testing

Send test messages using logger:

```bash
logger -p local0.info -t TestApp "Test message"
```

Using netcat:

```bash
echo "<13>TestApp: Test message" | nc -u localhost 514
```

## Monitoring

View Prometheus metrics:

```bash
curl http://localhost:9000/metrics
```

Example metrics output:
```
# HELP syslog_received_total Total number of logs received
syslog_received_total 42

# HELP syslog_written_total Total number of logs written
syslog_written_total 42
```

## Log Format

The CSV output includes:

```csv
Event_Time,Device_IP,Message,Severity,Facility
"2024-03-29 10:15:23.456","192.168.1.100","System started",INFO,LOCAL0
```

## Production Setup

For production deployment, use the provided systemd service:

```ini
[Unit]
Description=SysLog Server
After=network.target

[Service]
Type=simple
User=syslog
ExecStart=/usr/local/bin/syslog-server
Restart=always
LimitNOFILE=65535

[Install]
WantedBy=multi-user.target
```

## Author

Anubhav Gain ([@mranv](https://github.com/mranv))

## License

MIT License