// Port target for AbstractChannelProvider.kt

pub trait AbstractChannelProvider {
    fn open_channel(&self, name: &str) -> bool;
}

#[cfg(test)]
mod tests {
    use super::*;
    struct DummyProvider;
    impl AbstractChannelProvider for DummyProvider {
        fn open_channel(&self, _name: &str) -> bool { true }
    }

    #[test]
    fn provider_works() {
        let p = DummyProvider;
        assert!(p.open_channel("test"));
    }
}
