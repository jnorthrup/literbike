/// Minimal TrafficMirror stub used by `rbcursive::tunnel_config`.
/// The real project provides richer TLS mirroring; tests only need a
/// placeholder that constructs a default mirror.
#[derive(Clone)]
pub struct TrafficMirror {
    pub template: &'static str,
}

impl TrafficMirror {
    pub fn chrome_stable() -> Self {
        TrafficMirror { template: "chrome_stable" }
    }
}
