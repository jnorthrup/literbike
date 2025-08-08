use std::env;
use std::path::Path;
use std::process::Command;

// Import the syscall functionality from our library crate.
use litebike::syscall_net;

fn main() {
    let arg0 = env::args().next().unwrap_or_default();
    let command = Path::new(&arg0)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("");

    match command {
        "ifconfig" => ifconfig_main(),
        "ip" => ip_main(),
        "route" => route_main(),
        "netstat" => netstat_main(),
        "lsof" => lsof_main(),
        "setup-remote" => setup_remote_main(),
        _ => litebike_main(),
    };
}

/// Main function for `ifconfig` compatibility.
fn ifconfig_main() {
    println!("ifconfig command called");
    // Implementation will go here.
    match syscall_net::list_interfaces() {
        Ok(interfaces) => {
            for (_, iface) in interfaces {
                println!("Interface: {}", iface.name);
                for addr in iface.addrs {
                    println!("  {:?}", addr);
                }
            }
        }
        Err(e) => eprintln!("Error: {}", e),
    }
}

/// Main function for `ip` compatibility.
fn ip_main() {
    println!("ip command called");
    // Implementation will go here.
}

/// Main function for `route` compatibility.
fn route_main() {
    println!("route command called");
    // Implementation will go here.
}

/// Main function for `netstat` compatibility.
fn netstat_main() {
    println!("netstat command called");
    // Implementation will go here.
}

/// Main function for `lsof` compatibility.
fn lsof_main() {
    println!("lsof command called");
    // Implementation will go here.
}

/// Main function for `setup-remote` command.
fn setup_remote_main() {
    println!("Setting up remote repository...");

    let gateway_ip = match syscall_net::get_default_gateway() {
        Ok(ip) => ip,
        Err(e) => {
            eprintln!("Error getting default gateway: {}", e);
            return;
        }
    };
    println!("Default Gateway IP: {}", gateway_ip);

    let cwd_name = match get_cwd_name() {
        Ok(name) => name,
        Err(e) => {
            eprintln!("Error getting current directory name: {}", e);
            return;
        }
    };
    println!("Current directory name: {}", cwd_name);

    let remote_url = format!("ssh://jim@{}/~/{}", gateway_ip, cwd_name);
    println!("Remote URL: {}", remote_url);

    // 1. Ensure bare repo exists on remote
    let ssh_command = format!("mkdir -p ~/{0} && git init --bare ~/{0}", cwd_name);
    println!("Executing remote command: ssh jim@{} '{}'", gateway_ip, ssh_command);

    let output = Command::new("ssh")
        .arg("-p").arg("8022")
        .arg(format!("jim@{}", gateway_ip))
        .arg(&ssh_command)
        .output();

    match output {
        Ok(output) => {
            if output.status.success() {
                println!("Remote repository initialized successfully.");
            } else {
                eprintln!("Error initializing remote repository:");
                eprintln!("Stdout: {}", String::from_utf8_lossy(&output.stdout));
                eprintln!("Stderr: {}", String::from_utf8_lossy(&output.stderr));
                return;
            }
        }
        Err(e) => {
            eprintln!("Failed to execute SSH command: {}", e);
            return;
        }
    }

    // 2. Remove existing remote if it exists
    let _ = Command::new("git").arg("remote").arg("remove").arg("temp_upstream").output();

    // 3. Add new remote
    let output = Command::new("git").arg("remote").arg("add").arg("temp_upstream").arg(&remote_url).output();

    match output {
        Ok(output) => {
            if output.status.success() {
                println!("Git remote 'temp_upstream' added successfully.");
            } else {
                eprintln!("Error adding git remote:");
                eprintln!("Stdout: {}", String::from_utf8_lossy(&output.stdout));
                eprintln!("Stderr: {}", String::from_utf8_lossy(&output.stderr));
            }
        }
        Err(e) => {
            eprintln!("Failed to execute git remote add command: {}", e);
        }
    }
}

/// Gets the name of the current working directory.
fn get_cwd_name() -> Result<String, std::io::Error> {
    let current_dir = env::current_dir()?;
    let name = current_dir.file_name()
        .and_then(|s| s.to_str())
        .map(|s| s.to_string())
        .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::InvalidData, "Could not get current directory name"))?;
    Ok(name)
}

/// Default main function for `litebike`.
fn litebike_main() {
    println!("LiteBike: A resilient, dependency-free network toolkit.");
    println!("Usage: Call this binary via hardlinks named ifconfig, ip, route, netstat, lsof, or setup-remote.");
}
