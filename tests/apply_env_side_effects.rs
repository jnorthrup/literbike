use std::env;

use literbike::config::Config;

#[test]
fn apply_env_side_effects_sets_egress_vars() {
    // Preserve existing env
    let orig_iface = env::var("EGRESS_INTERFACE").ok();
    let orig_ip = env::var("EGRESS_BIND_IP").ok();

    let mut cfg = Config::default();
    cfg.egress_interface = Some("rmnet0".to_string());
    cfg.egress_bind_ip = Some("10.0.0.5".parse().unwrap());

    // Ensure env not set before
    env::remove_var("EGRESS_INTERFACE");
    env::remove_var("EGRESS_BIND_IP");

    cfg.apply_env_side_effects();

    assert_eq!(env::var("EGRESS_INTERFACE").unwrap(), "rmnet0");
    assert_eq!(env::var("EGRESS_BIND_IP").unwrap(), "10.0.0.5");

    // Restore original env
    if let Some(v) = orig_iface { env::set_var("EGRESS_INTERFACE", v); } else { env::remove_var("EGRESS_INTERFACE"); }
    if let Some(v) = orig_ip { env::set_var("EGRESS_BIND_IP", v); } else { env::remove_var("EGRESS_BIND_IP"); }
}
