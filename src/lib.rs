pub mod adapters;
pub mod channel;
pub mod quic;
pub mod reactor;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn smoke() {
        // Simple smoke test to confirm crate builds and modules link
        let _ = adapters::ssh::ssh_adapter_name();
    }
}
