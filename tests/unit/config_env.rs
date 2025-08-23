use std::env;

use literbike::config::Config;

#[test]
fn from_env_applies_bind_addr_and_interface_independently() {
    // Preserve existing env
    let orig_interface = env::var("LITEBIKE_INTERFACE").ok();
    let orig_bind_addr = env::var("LITEBIKE_BIND_ADDR").ok();

    env::set_var("LITEBIKE_INTERFACE", "eth0");
    env::set_var("LITEBIKE_BIND_ADDR", "127.0.0.2");

    let cfg = Config::from_env();

    // Expect both to be applied independently
    assert_eq!(cfg.interface, "eth0");
    assert_eq!(cfg.bind_addr.to_string(), "127.0.0.2");

    // Restore original env
    if let Some(v) = orig_interface {
        env::set_var("LITEBIKE_INTERFACE", v);
    } else {
        env::remove_var("LITEBIKE_INTERFACE");
    }
    if let Some(v) = orig_bind_addr {
        env::set_var("LITEBIKE_BIND_ADDR", v);
    } else {
        env::remove_var("LITEBIKE_BIND_ADDR");
    }
}
