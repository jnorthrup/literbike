use literbike::rbcursive::PortHoppingConfig;

#[test]
fn select_port_within_allowed_set() {
    let cfg = PortHoppingConfig::default();

    // Run multiple selections to exercise randomness
    for _ in 0..100 {
        let p = cfg.select_port();
        let allowed: Vec<u16> = cfg.primary_ports.iter().chain(cfg.fallback_ports.iter()).cloned().collect();
        assert!(allowed.contains(&p), "selected port {} not in allowed set: {:?}", p, allowed);
    }
}
