#[derive(Clone, Debug, Default)]
pub struct CcekCadence {
    pub burst_ms: u32,
    pub idle_ms: u32,
    pub jitter_ms: u32,
}

#[derive(Clone, Debug, Default)]
pub struct CcekPolicy {
    pub enable_cover: bool,
    pub cadence: CcekCadence,
}
