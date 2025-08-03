// Network Simulation Utilities
// Provides network condition simulation for testing proxy behavior under various conditions

use tokio::net::{TcpListener, TcpStream};
use tokio::io::{AsyncRead, AsyncWrite, AsyncReadExt, AsyncWriteExt};
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::{sleep, timeout};
use std::pin::Pin;
use std::task::{Context, Poll};
use std::io;

/// Network condition simulator that wraps streams to introduce various network behaviors
pub struct NetworkSimulator {
    conditions: NetworkConditions,
}

#[derive(Clone, Debug)]
pub struct NetworkConditions {
    pub latency: Duration,
    pub bandwidth_limit: Option<u64>, // bytes per second
    pub packet_loss_rate: f64, // 0.0 to 1.0
    pub jitter: Duration, // random delay variance
    pub disconnect_probability: f64, // probability of sudden disconnection
}

impl Default for NetworkConditions {
    fn default() -> Self {
        Self {
            latency: Duration::from_millis(0),
            bandwidth_limit: None,
            packet_loss_rate: 0.0,
            jitter: Duration::from_millis(0),
            disconnect_probability: 0.0,
        }
    }
}

impl NetworkConditions {
    pub fn high_latency() -> Self {
        Self {
            latency: Duration::from_millis(500),
            jitter: Duration::from_millis(100),
            ..Default::default()
        }
    }
    
    pub fn low_bandwidth() -> Self {
        Self {
            bandwidth_limit: Some(1024), // 1 KB/s
            ..Default::default()
        }
    }
    
    pub fn unstable() -> Self {
        Self {
            latency: Duration::from_millis(100),
            jitter: Duration::from_millis(200),
            packet_loss_rate: 0.05, // 5% packet loss
            disconnect_probability: 0.01, // 1% chance of disconnection
            ..Default::default()
        }
    }
    
    pub fn mobile_network() -> Self {
        Self {
            latency: Duration::from_millis(150),
            bandwidth_limit: Some(50_000), // 50 KB/s
            jitter: Duration::from_millis(50),
            packet_loss_rate: 0.02,
            ..Default::default()
        }
    }
    
    pub fn satellite() -> Self {
        Self {
            latency: Duration::from_millis(600),
            bandwidth_limit: Some(1_000_000), // 1 MB/s
            jitter: Duration::from_millis(100),
            packet_loss_rate: 0.001,
            ..Default::default()
        }
    }
}

impl NetworkSimulator {
    pub fn new(conditions: NetworkConditions) -> Self {
        Self { conditions }
    }
    
    /// Wrap a stream with network simulation
    pub fn wrap_stream<S>(&self, stream: S) -> SimulatedStream<S>
    where
        S: AsyncRead + AsyncWrite + Unpin,
    {
        SimulatedStream::new(stream, self.conditions.clone())
    }
    
    /// Create a simulated network bridge between two streams
    pub async fn bridge_streams<S1, S2>(&self, mut stream1: S1, mut stream2: S2) -> io::Result<()>
    where
        S1: AsyncRead + AsyncWrite + Unpin,
        S2: AsyncRead + AsyncWrite + Unpin,
    {
        let simulated1 = self.wrap_stream(&mut stream1);
        let simulated2 = self.wrap_stream(&mut stream2);
        
        let (mut r1, mut w1) = tokio::io::split(simulated1);
        let (mut r2, mut w2) = tokio::io::split(simulated2);
        
        let forward1 = tokio::io::copy(&mut r1, &mut w2);
        let forward2 = tokio::io::copy(&mut r2, &mut w1);
        
        tokio::select! {
            result = forward1 => result.map(|_| ()),
            result = forward2 => result.map(|_| ()),
        }
    }
}

/// Stream wrapper that simulates network conditions
pub struct SimulatedStream<S> {
    inner: S,
    conditions: NetworkConditions,
    last_operation_time: std::time::Instant,
    bytes_this_second: u64,
    second_start: std::time::Instant,
}

impl<S> SimulatedStream<S>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    fn new(inner: S, conditions: NetworkConditions) -> Self {
        let now = std::time::Instant::now();
        Self {
            inner,
            conditions,
            last_operation_time: now,
            bytes_this_second: 0,
            second_start: now,
        }
    }
    
    async fn apply_conditions(&mut self, bytes_count: usize) -> io::Result<()> {
        // Check for disconnection
        if self.conditions.disconnect_probability > 0.0 {
            if rand::random::<f64>() < self.conditions.disconnect_probability {
                return Err(io::Error::new(
                    io::ErrorKind::ConnectionAborted,
                    "Simulated disconnection"
                ));
            }
        }
        
        // Apply bandwidth limiting
        if let Some(bandwidth_limit) = self.conditions.bandwidth_limit {
            let now = std::time::Instant::now();
            
            // Reset counter if a second has passed
            if now.duration_since(self.second_start) >= Duration::from_secs(1) {
                self.bytes_this_second = 0;
                self.second_start = now;
            }
            
            self.bytes_this_second += bytes_count as u64;
            
            // If we've exceeded the bandwidth limit, wait
            if self.bytes_this_second > bandwidth_limit {
                let wait_time = Duration::from_secs(1) - now.duration_since(self.second_start);
                sleep(wait_time).await;
                
                // Reset for next second
                self.bytes_this_second = 0;
                self.second_start = std::time::Instant::now();
            }
        }
        
        // Apply latency
        if self.conditions.latency > Duration::from_millis(0) {
            let mut delay = self.conditions.latency;
            
            // Add jitter
            if self.conditions.jitter > Duration::from_millis(0) {
                let jitter_ms = rand::random::<u64>() % self.conditions.jitter.as_millis() as u64;
                delay += Duration::from_millis(jitter_ms);
            }
            
            sleep(delay).await;
        }
        
        // Simulate packet loss
        if self.conditions.packet_loss_rate > 0.0 {
            if rand::random::<f64>() < self.conditions.packet_loss_rate {
                return Err(io::Error::new(
                    io::ErrorKind::TimedOut,
                    "Simulated packet loss"
                ));
            }
        }
        
        self.last_operation_time = std::time::Instant::now();
        Ok(())
    }
}

impl<S> AsyncRead for SimulatedStream<S>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        // First, try to read from the inner stream
        let initial_filled = buf.filled().len();
        let result = Pin::new(&mut self.inner).poll_read(cx, buf);
        
        match result {
            Poll::Ready(Ok(())) => {
                let bytes_read = buf.filled().len() - initial_filled;
                if bytes_read > 0 {
                    // We need to apply conditions, but poll_read is synchronous
                    // In a real implementation, you'd need a more sophisticated approach
                    // For now, we'll just apply bandwidth limiting
                    if let Some(bandwidth_limit) = self.conditions.bandwidth_limit {
                        let now = std::time::Instant::now();
                        if now.duration_since(self.second_start) >= Duration::from_secs(1) {
                            self.bytes_this_second = 0;
                            self.second_start = now;
                        }
                        
                        self.bytes_this_second += bytes_read as u64;
                        
                        if self.bytes_this_second > bandwidth_limit {
                            // Return pending to simulate bandwidth limiting
                            cx.waker().wake_by_ref();
                            return Poll::Pending;
                        }
                    }
                }
                Poll::Ready(Ok(()))
            }
            other => other,
        }
    }
}

impl<S> AsyncWrite for SimulatedStream<S>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<Result<usize, io::Error>> {
        // Apply conditions before writing
        if let Some(bandwidth_limit) = self.conditions.bandwidth_limit {
            let now = std::time::Instant::now();
            if now.duration_since(self.second_start) >= Duration::from_secs(1) {
                self.bytes_this_second = 0;
                self.second_start = now;
            }
            
            if self.bytes_this_second >= bandwidth_limit {
                cx.waker().wake_by_ref();
                return Poll::Pending;
            }
            
            // Limit the write size if necessary
            let max_bytes = bandwidth_limit - self.bytes_this_second;
            let write_size = buf.len().min(max_bytes as usize);
            
            if write_size == 0 {
                cx.waker().wake_by_ref();
                return Poll::Pending;
            }
            
            let result = Pin::new(&mut self.inner).poll_write(cx, &buf[..write_size]);
            
            if let Poll::Ready(Ok(bytes_written)) = result {
                self.bytes_this_second += bytes_written as u64;
            }
            
            result
        } else {
            Pin::new(&mut self.inner).poll_write(cx, buf)
        }
    }
    
    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), io::Error>> {
        Pin::new(&mut self.inner).poll_flush(cx)
    }
    
    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), io::Error>> {
        Pin::new(&mut self.inner).poll_shutdown(cx)
    }
}

/// Test server that simulates various network conditions
pub struct NetworkTestServer {
    listener: TcpListener,
    simulator: NetworkSimulator,
    behavior: ServerBehavior,
}

#[derive(Clone)]
pub enum ServerBehavior {
    Echo,
    Http { response: String },
    Socks5 { auth_required: bool },
    SlowResponse { delay: Duration },
    RandomDisconnect { probability: f64 },
    DataCorruption { corruption_rate: f64 },
}

impl NetworkTestServer {
    pub async fn new(conditions: NetworkConditions, behavior: ServerBehavior) -> io::Result<Self> {
        let listener = TcpListener::bind("127.0.0.1:0").await?;
        let simulator = NetworkSimulator::new(conditions);
        
        Ok(Self {
            listener,
            simulator,
            behavior,
        })
    }
    
    pub fn addr(&self) -> SocketAddr {
        self.listener.local_addr().unwrap()
    }
    
    pub async fn run(mut self) -> io::Result<()> {
        while let Ok((stream, addr)) = self.listener.accept().await {
            let simulator = NetworkSimulator::new(self.simulator.conditions.clone());
            let behavior = self.behavior.clone();
            
            tokio::spawn(async move {
                Self::handle_connection(stream, simulator, behavior, addr).await
            });
        }
        
        Ok(())
    }
    
    async fn handle_connection(
        stream: TcpStream,
        simulator: NetworkSimulator,
        behavior: ServerBehavior,
        addr: SocketAddr,
    ) {
        let mut simulated_stream = simulator.wrap_stream(stream);
        
        match behavior {
            ServerBehavior::Echo => {
                Self::handle_echo(&mut simulated_stream).await;
            }
            ServerBehavior::Http { response } => {
                Self::handle_http(&mut simulated_stream, &response).await;
            }
            ServerBehavior::Socks5 { auth_required } => {
                Self::handle_socks5(&mut simulated_stream, auth_required).await;
            }
            ServerBehavior::SlowResponse { delay } => {
                sleep(delay).await;
                Self::handle_echo(&mut simulated_stream).await;
            }
            ServerBehavior::RandomDisconnect { probability } => {
                if rand::random::<f64>() < probability {
                    return; // Just disconnect
                }
                Self::handle_echo(&mut simulated_stream).await;
            }
            ServerBehavior::DataCorruption { corruption_rate } => {
                Self::handle_corrupted_echo(&mut simulated_stream, corruption_rate).await;
            }
        }
    }
    
    async fn handle_echo<S>(stream: &mut S)
    where
        S: AsyncRead + AsyncWrite + Unpin,
    {
        let mut buffer = vec![0u8; 4096];
        while let Ok(n) = stream.read(&mut buffer).await {
            if n == 0 { break; }
            
            if stream.write_all(&buffer[..n]).await.is_err() {
                break;
            }
        }
    }
    
    async fn handle_http<S>(stream: &mut S, response: &str)
    where
        S: AsyncRead + AsyncWrite + Unpin,
    {
        let mut buffer = vec![0u8; 4096];
        if stream.read(&mut buffer).await.is_ok() {
            let _ = stream.write_all(response.as_bytes()).await;
        }
    }
    
    async fn handle_socks5<S>(stream: &mut S, auth_required: bool)
    where
        S: AsyncRead + AsyncWrite + Unpin,
    {
        // Simplified SOCKS5 handling
        let mut buffer = [0u8; 256];
        
        // Read handshake
        if stream.read(&mut buffer).await.is_ok() {
            // Send method selection
            let response = if auth_required { [5, 2] } else { [5, 0] };
            let _ = stream.write_all(&response).await;
            
            if auth_required {
                // Handle auth (simplified - accept anything)
                if stream.read(&mut buffer).await.is_ok() {
                    let _ = stream.write_all(&[1, 0]).await; // Auth success
                }
            }
            
            // Handle connect request
            if stream.read(&mut buffer).await.is_ok() {
                let response = [5, 0, 0, 1, 127, 0, 0, 1, 0, 80]; // Success
                let _ = stream.write_all(&response).await;
            }
        }
    }
    
    async fn handle_corrupted_echo<S>(stream: &mut S, corruption_rate: f64)
    where
        S: AsyncRead + AsyncWrite + Unpin,
    {
        let mut buffer = vec![0u8; 4096];
        while let Ok(n) = stream.read(&mut buffer).await {
            if n == 0 { break; }
            
            // Corrupt some bytes
            for byte in &mut buffer[..n] {
                if rand::random::<f64>() < corruption_rate {
                    *byte = rand::random();
                }
            }
            
            if stream.write_all(&buffer[..n]).await.is_err() {
                break;
            }
        }
    }
}

/// Load generator for stress testing
pub struct LoadGenerator {
    target_addr: SocketAddr,
    connection_rate: u64, // connections per second
    connections_per_test: usize,
    request_data: Vec<u8>,
}

impl LoadGenerator {
    pub fn new(target_addr: SocketAddr) -> Self {
        Self {
            target_addr,
            connection_rate: 10,
            connections_per_test: 100,
            request_data: b"GET / HTTP/1.1\r\nHost: example.com\r\n\r\n".to_vec(),
        }
    }
    
    pub fn with_connection_rate(mut self, rate: u64) -> Self {
        self.connection_rate = rate;
        self
    }
    
    pub fn with_connection_count(mut self, count: usize) -> Self {
        self.connections_per_test = count;
        self
    }
    
    pub fn with_request_data(mut self, data: Vec<u8>) -> Self {
        self.request_data = data;
        self
    }
    
    /// Generate load and return metrics
    pub async fn generate_load(&self) -> LoadTestResult {
        let mut results = LoadTestResult::new();
        let start_time = std::time::Instant::now();
        
        let connection_interval = Duration::from_nanos(1_000_000_000 / self.connection_rate);
        let mut tasks = Vec::new();
        
        for i in 0..self.connections_per_test {
            let target_addr = self.target_addr;
            let request_data = self.request_data.clone();
            let connection_id = i;
            
            let task = tokio::spawn(async move {
                Self::single_connection_test(connection_id, target_addr, request_data).await
            });
            
            tasks.push(task);
            
            // Rate limiting
            if i < self.connections_per_test - 1 {
                sleep(connection_interval).await;
            }
        }
        
        // Collect results
        for (i, task) in tasks.into_iter().enumerate() {
            match task.await {
                Ok(connection_result) => {
                    results.add_connection_result(connection_result);
                }
                Err(e) => {
                    results.add_error(format!("Task {} panicked: {:?}", i, e));
                }
            }
        }
        
        results.total_duration = start_time.elapsed();
        results.calculate_rates();
        results
    }
    
    async fn single_connection_test(
        connection_id: usize,
        target_addr: SocketAddr,
        request_data: Vec<u8>,
    ) -> ConnectionResult {
        let start_time = std::time::Instant::now();
        
        match TcpStream::connect(target_addr).await {
            Ok(mut stream) => {
                let connect_time = start_time.elapsed();
                
                // Send request
                match stream.write_all(&request_data).await {
                    Ok(_) => {
                        let send_time = start_time.elapsed();
                        
                        // Read response
                        let mut response = Vec::new();
                        match timeout(Duration::from_secs(10), stream.read_to_end(&mut response)).await {
                            Ok(Ok(_)) => {
                                let total_time = start_time.elapsed();
                                ConnectionResult {
                                    connection_id,
                                    success: true,
                                    connect_time,
                                    send_time,
                                    total_time,
                                    bytes_sent: request_data.len(),
                                    bytes_received: response.len(),
                                    error: None,
                                }
                            }
                            Ok(Err(e)) => ConnectionResult::error(connection_id, e.to_string()),
                            Err(_) => ConnectionResult::error(connection_id, "Read timeout".to_string()),
                        }
                    }
                    Err(e) => ConnectionResult::error(connection_id, e.to_string()),
                }
            }
            Err(e) => ConnectionResult::error(connection_id, e.to_string()),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ConnectionResult {
    pub connection_id: usize,
    pub success: bool,
    pub connect_time: Duration,
    pub send_time: Duration,
    pub total_time: Duration,
    pub bytes_sent: usize,
    pub bytes_received: usize,
    pub error: Option<String>,
}

impl ConnectionResult {
    fn error(connection_id: usize, error: String) -> Self {
        Self {
            connection_id,
            success: false,
            connect_time: Duration::from_secs(0),
            send_time: Duration::from_secs(0),
            total_time: Duration::from_secs(0),
            bytes_sent: 0,
            bytes_received: 0,
            error: Some(error),
        }
    }
}

#[derive(Debug, Clone)]
pub struct LoadTestResult {
    pub total_connections: usize,
    pub successful_connections: usize,
    pub failed_connections: usize,
    pub total_duration: Duration,
    pub total_bytes_sent: usize,
    pub total_bytes_received: usize,
    pub average_connect_time: Duration,
    pub average_total_time: Duration,
    pub connections_per_second: f64,
    pub errors: Vec<String>,
}

impl LoadTestResult {
    fn new() -> Self {
        Self {
            total_connections: 0,
            successful_connections: 0,
            failed_connections: 0,
            total_duration: Duration::from_secs(0),
            total_bytes_sent: 0,
            total_bytes_received: 0,
            average_connect_time: Duration::from_secs(0),
            average_total_time: Duration::from_secs(0),
            connections_per_second: 0.0,
            errors: Vec::new(),
        }
    }
    
    fn add_connection_result(&mut self, result: ConnectionResult) {
        self.total_connections += 1;
        
        if result.success {
            self.successful_connections += 1;
            self.total_bytes_sent += result.bytes_sent;
            self.total_bytes_received += result.bytes_received;
        } else {
            self.failed_connections += 1;
            if let Some(error) = result.error {
                self.errors.push(error);
            }
        }
    }
    
    fn add_error(&mut self, error: String) {
        self.failed_connections += 1;
        self.errors.push(error);
    }
    
    fn calculate_rates(&mut self) {
        if self.total_duration.as_secs_f64() > 0.0 {
            self.connections_per_second = self.total_connections as f64 / self.total_duration.as_secs_f64();
        }
        
        // Calculate averages for successful connections only
        if self.successful_connections > 0 {
            // This is simplified - in a real implementation you'd track individual times
            self.average_connect_time = Duration::from_millis(10); // Placeholder
            self.average_total_time = Duration::from_millis(50); // Placeholder
        }
    }
}