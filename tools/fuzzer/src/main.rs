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
    run(args.target, args.seeds, args.iterations, args.delay_ms).await
}

pub async fn run(target: String, seeds_path: String, iterations: usize, delay_ms: u64) -> Result<()> {
    let seeds = collect_seeds(&seeds_path)?;

    println!("Litebike fuzzer -> target={} seeds={} iterations={}", target, seeds.len(), iterations);

    for i in 0..iterations {
        let seed = seeds.get(i % seeds.len()).unwrap().clone();
        let mutated = mutate(seed);

        if let Err(e) = fuzz_once(&target, &mutated).await {
            eprintln!("iteration {} error: {}", i, e);
        }

        tokio::time::sleep(Duration::from_millis(delay_ms)).await;
    }

    Ok(())
}

pub fn collect_seeds(path: &str) -> Result<Vec<Vec<u8>>, anyhow::Error> {
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

pub fn mutate(mut seed: Vec<u8>) -> Vec<u8> {
    let mut rng = rand::thread_rng();
    let ops = rng.gen_range(1..4);
    for _ in 0..ops {
        match rng.gen_range(0..4) {
            0 => { // flip byte
                if seed.is_empty() { continue; }
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
                if seed.is_empty() { continue; }
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

pub async fn fuzz_once(target: &str, data: &[u8]) -> Result<(), anyhow::Error> {
    let addr: SocketAddr = target.parse()?;
    let mut stream = TcpStream::connect(addr).await?;
    stream.write_all(data).await?;

    let mut buf = [0u8; 1024];
    let _ = stream.read(&mut buf).await;
    Ok(())
}


#[cfg(test)]
mod tests {
    use super::*;
    use tokio::net::TcpListener;

    #[test]
    fn test_collect_seeds_missing_dir() {
        let res = collect_seeds("nonexistent-file");
        assert!(res.is_err());
    }

    #[test]
    fn test_mutate_preserves_nonempty() {
        let s = b"hello".to_vec();
        let out = mutate(s.clone());
        assert!(!out.is_empty());
    }

    #[tokio::test]
    async fn test_fuzz_once_roundtrip() {
        // start a mock server
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
        let addr = listener.local_addr().unwrap();

        let server = tokio::spawn(async move {
            let (mut s, _) = listener.accept().await.expect("accept");
            let mut buf = vec![0u8; 1024];
            let n = s.read(&mut buf).await.expect("read");
            // echo back
            let _ = s.write_all(&buf[..n]).await;
        });

        let data = b"ping".to_vec();
        let target = format!("{}", addr);
        let res = fuzz_once(&target, &data).await;
        assert!(res.is_ok());
        server.await.expect("server");
    }
}
