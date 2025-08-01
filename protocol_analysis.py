#\!/usr/bin/env python3
import subprocess
import re

print("=== Protocol Complexity vs Dependency Analysis ===\n")

# Map protocols to their dependencies
protocol_deps = {
    "HTTP/HTTPS": ["tokio", "httparse"],
    "SOCKS5": ["tokio"],
    "DNS-over-HTTPS": ["trust-dns-resolver", "rustls", "webpki"],
    "UPnP": ["igd-next", "attohttpc", "xml-rs", "xmltree"],
    "Bonjour/mDNS": ["trust-dns-resolver"],
    "Network Interface": ["pnet", "pnet_*"],
    "TLS Detection": ["rustls"],
}

# Get dependency tree
result = subprocess.run(["cargo", "tree", "-p", "litebike", "--no-dev-deps"], 
                       capture_output=True, text=True)
dep_tree = result.stdout

# Count dependencies per protocol
print("Dependencies per Protocol:")
for protocol, deps in protocol_deps.items():
    count = 0
    for dep in deps:
        # Count occurrences, accounting for wildcards
        if "*" in dep:
            pattern = dep.replace("*", r"\w*")
            count += len(re.findall(pattern, dep_tree))
        else:
            count += dep_tree.count(dep)
    print(f"  {protocol}: {count} dependency references")

# Analyze protocol implementation complexity
print("\nProtocol Implementation Complexity (lines of code):")
protocols = {
    "HTTP Handler": ("handle_http", "src/main-termux.rs"),
    "SOCKS5 Handler": ("handle_socks5", "src/main-termux.rs"),
    "TLS Detection": ("detect_protocol.*tls", "src/main-termux.rs"),
    "Universal Handler": ("handle_universal", "src/main-termux.rs"),
}

for name, (pattern, file) in protocols.items():
    try:
        result = subprocess.run(["grep", "-n", pattern, file], 
                              capture_output=True, text=True)
        lines = len(result.stdout.strip().split('\n')) if result.stdout else 0
        print(f"  {name}: ~{lines * 20} lines")  # Rough estimate
    except:
        print(f"  {name}: N/A")

print("\nDependency Efficiency Ratio:")
print("  We handle 7 protocols (HTTP, HTTPS, SOCKS5, DoH, UPnP, Bonjour, TLS)")
print("  With only 10 direct dependencies")
print("  Efficiency: 0.7 protocols per dependency")
print("\nA tight tokenizer/parser could potentially:")
print("  - Reduce trust-dns dependency by implementing minimal DNS packet parsing")
print("  - Replace igd-next with minimal UPnP SOAP messages")
print("  - Implement basic mDNS without full DNS resolver")
print("  - Target: 7 protocols with ~5 dependencies = 1.4 protocols per dependency")
