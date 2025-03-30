# Internet Monitor

A Rust application that tracks:
- Latency to Google.com
- Download speed from a test file
- Upload speed to httpbin.org

## Features

- Measures and records internet performance metrics
- Stores metrics in InfluxDB
- Visualizes data in Grafana
- Fully dockerized for easy deployment

## Quick Start

1. Clone this repository
2. Run the application with Docker Compose:

```bash
docker-compose up -d
```

3. Access Grafana at http://localhost:3000
    - Username: admin
    - Password: admin123

## Configuration

You can adjust the monitoring parameters by editing the `docker-compose.yml`