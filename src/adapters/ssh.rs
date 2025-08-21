// Port target for trikeshed-channel-impl/.../SshProtocolAdapter.kt

pub fn ssh_adapter_name() -> &'static str {
    // placeholder for russh-based adapter
    "ssh::SshProtocolAdapter"
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ssh_name() {
        assert_eq!(ssh_adapter_name(), "ssh::SshProtocolAdapter");
    }
}
