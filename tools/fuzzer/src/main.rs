use std::fs;
use std::net::SocketAddr;
use std::time::Duration;

use anyhow::Result;
use clap::Parser;
use rand::Rng;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

/// Minimal extensible client->host fuzzer for litebike
#[derive(Parser, Debug)]
struct Args {
    /// Target host:port (e.g. 127.0.0.1:8080)
    #[arg(long)]
    target: String,

    /// Seed file or directory with seeds
    #[arg(long, default_value = "seeds")]
    seeds: String,

    /// Number of iterations
    #[arg(long, default_value_t = 1000)]
    iterations: usize,

    /// Delay between iterations (ms)
    #[arg(long, default_value_t = 10)]
    delay_ms: u64,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();
    let seeds = collect_seeds(&args.seeds)?;

    println!("Litebike fuzzer -> target={} seeds={} iterations={}", args.target, seeds.len(), args.iterations);

    for i in 0..args.iterations {
        let seed = seeds.get(i % seeds.len()).unwrap().clone();
        let mut mutated = mutate(seed);

        if let Err(e) = fuzz_once(&args.target, &mut mutated).await {
            eprintln!("iteration {} error: {}", i, e);
        }

        tokio::time::sleep(Duration::from_millis(args.delay_ms)).await;
    }

    Ok(())
}

fn collect_seeds(path: &str) -> Result<Vec<Vec<u8>>, anyhow::Error> {
    let metadata = fs::metadata(path)?;
    let mut seeds = Vec::new();

    if metadata.is_dir() {
        for entry in fs::read_dir(path)? {
            let p = entry?.path();
            if p.is_file() {
                seeds.push(fs::read(p)?);
            }
        }
    } else {
        seeds.push(fs::read(path)?);
    }

    if seeds.is_empty() {
        seeds.push(b"GET / HTTP/1.1\r\nHost: example\r\n\r\n".to_vec());
    }

    Ok(seeds)
}

fn mutate(mut seed: Vec<u8>) -> Vec<u8> {
    let mut rng = rand::thread_rng();
    let ops = rng.gen_range(1..4);
    for _ in 0..ops {
        match rng.gen_range(0..4) {
            0 => { // flip byte
                let idx = rng.gen_range(0..seed.len());
                seed[idx] = seed[idx].wrapping_add(rng.gen::<u8>());
            }
            1 => { // insert random byte
                let idx = rng.gen_range(0..=seed.len());
                seed.insert(idx, rng.gen());
            }
            2 => { // delete byte
                if seed.len() > 1 {
                    let idx = rng.gen_range(0..seed.len());
                    seed.remove(idx);
                }
            }
            3 => { // duplicate slice
                let start = rng.gen_range(0..seed.len());
                let len = rng.gen_range(1..=(seed.len()-start));
                let slice = seed[start..start+len].to_vec();
                let idx = rng.gen_range(0..=seed.len());
                seed.splice(idx..idx, slice);
            }
            _ => {}
        }
    }
    seed
}

async fn fuzz_once(target: &str, data: &[u8]) -> Result<(), anyhow::Error> {
    let addr: SocketAddr = target.parse()?;
    let mut stream = TcpStream::connect(addr).await?;
    stream.write_all(data).await?;

    let mut buf = [0u8; 1024];
    let _ = stream.read(&mut buf).await;
    Ok(())
}
