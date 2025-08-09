
import socket
import sys

def check_port(port):
    sock = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
    try:
        sock.bind(('0.0.0.0', port))
        print(f"Port {port} is AVAILABLE")
    except socket.error as e:
        if e.errno in [98, 48]:  # EADDRINUSE on Linux (98) and macOS (48)
            print(f"Port {port} is IN_USE")
        else:
            print(f"Port {port} error: {e}")
    finally:
        sock.close()

if __name__ == "__main__":
    if len(sys.argv) != 2:
        print("Usage: python check_port.py <port_number>")
        sys.exit(1)
    
    try:
        port = int(sys.argv[1])
        check_port(port)
    except ValueError:
        print("Invalid port number.")
        sys.exit(1)
