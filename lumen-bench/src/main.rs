use std::time::Instant;
use tonic::transport::Channel;
use hdrhistogram::Histogram;

// Import generated proto code
pub mod pb {
    // MUST match "package kv;" from your proto file
    tonic::include_proto!("kv");
}

// Service name is "KeyValueStore", so client is "KeyValueStoreClient"
use pb::key_value_store_client::KeyValueStoreClient;
// Message is "PutRequest"
use pb::PutRequest;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 1. Connect to the Server
    let channel = Channel::from_static("http://0.0.0.0:50051")
        .connect()
        .await
        .expect("Failed to connect to LumenKV. Is the server running?");

    let mut client = KeyValueStoreClient::new(channel);
    let total_requests = 10_000;
    
    println!("🚀 Starting Benchmark: {} requests...", total_requests);
    let start = Instant::now();
    let mut hist = Histogram::<u64>::new(3).unwrap();

    // 2. The Attack Loop
    for i in 0..total_requests {
        let key = format!("bench-key-{}", i);
        let value = vec![0u8; 128]; // 128-byte payload

        let request = tonic::Request::new(PutRequest { 
            key, 
            value 
        });

        let op_start = Instant::now();
        
        // Changed from .set() to .put() to match your Proto
        client.put(request).await?; 
        
        hist.record(op_start.elapsed().as_micros() as u64).unwrap();
    }

    let duration = start.elapsed();
    let ops = total_requests as f64 / duration.as_secs_f64();

    // 3. The Report
    println!("\n✅ Benchmark Complete!");
    println!("⏱️  Total Time: {:.2?}", duration);
    println!("⚡ Throughput: {:.2} ops/sec", ops);
    println!("📊 Latency (P50): {} µs", hist.value_at_quantile(0.50));
    println!("📊 Latency (P99): {} µs", hist.value_at_quantile(0.99));

    Ok(())
}
