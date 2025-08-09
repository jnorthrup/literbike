import socket
import sys
import time

def debug_connect(host, port, payload):
    """
    Connects to a host/port, sends a payload, and prints any response.
    """
    print(f"--- Attempting to connect to {host}:{port} ---")
    try:
        # Create a socket and connect
        with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as s:
            s.settimeout(5)  # 5-second timeout for operations
            s.connect((host, port))
            print("--- Connection successful ---")

            # Send the payload
            print(f"--- Sending payload ---\n{payload.decode('utf-8', 'ignore').strip()}")
            s.sendall(payload)
            print("--- Payload sent ---")

            # Listen for a response
            s.settimeout(2) # Set a shorter timeout for receiving data
            response_data = b""
            try:
                while True:
                    chunk = s.recv(4096)
                    if not chunk:
                        print("--- Connection closed by server (no more data) ---")
                        break
                    response_data += chunk
            except socket.timeout:
                print("--- Socket timeout (no more data received) ---")
            
            if response_data:
                print(f"--- Received response ---\n{response_data.decode('utf-8', 'ignore')}")
            else:
                print("--- No response received ---")

    except socket.timeout:
        print("--- Connection timed out ---")
    except ConnectionRefusedError:
        print("--- Connection refused ---")
    except Exception as e:
        print(f"--- An error occurred: {e} ---")

if __name__ == "__main__":
    if len(sys.argv) != 2:
        print("Usage: python debug_client.py <http|socks5>")
        sys.exit(1)

    HOST = "192.168.225.152"
    PORT = 8888
    
    command = sys.argv[1]
    
    if command == "http":
        # A standard HTTP GET request
        payload = (
            b"GET / HTTP/1.1\r\n"
            b"Host: example.com\r\n"
            b"User-Agent: debug-client/1.0\r\n"
            b"Accept: */*\r\n"
            b"Connection: close\r\n\r\n"
        )
        debug_connect(HOST, PORT, payload)
    elif command == "socks5":
        # A SOCKS5 handshake and request to connect to example.com:80
        # 1. Client Greeting
        greeting = b"\x05\x01\x00" # Version 5, 1 auth method, 'No Authentication'
        # 2. Client Connection Request
        request = (
            b"\x05\x01\x00\x03"  # Version 5, CONNECT, RSV, Domain name
            b"\x0bexample.com"  # 11-byte domain name
            b"\x00\x50"          # Port 80
        )
        payload = greeting + request
        debug_connect(HOST, PORT, payload)
    else:
        print(f"Unknown command: {command}")
        sys.exit(1)
