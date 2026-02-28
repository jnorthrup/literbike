import hashlib
import hmac

def hkdf_extract(salt: bytes, input_key_material: bytes) -> bytes:
    if not salt:
        salt = bytes([0] * hashlib.sha256().digest_size)
    return hmac.new(salt, input_key_material, hashlib.sha256).digest()

def hkdf_expand_label(secret: bytes, label: bytes, context: bytes, length: int) -> bytes:
    hkdf_label = length.to_bytes(2, "big")
    full_label = b"tls13 " + label
    hkdf_label += len(full_label).to_bytes(1, "big") + full_label
    hkdf_label += len(context).to_bytes(1, "big") + context
    
    # Expand
    prk = secret
    t = b""
    okm = b""
    i = 1
    while len(okm) < length:
        t = hmac.new(prk, t + hkdf_label + bytes([i]), hashlib.sha256).digest()
        okm += t
        i += 1
    return okm[:length]

cid = bytes.fromhex("8394c8f03e515708")
salt = bytes([0x38, 0x76, 0x2c, 0xf7, 0xf5, 0x59, 0x34, 0xb3, 0x4d, 0x17, 0x9a, 0xe6, 0xa4, 0xc8, 0x0c, 0xad, 0xcc, 0xbb, 0x7f, 0x0a])

initial_secret = hkdf_extract(salt, cid)
client_initial_secret = hkdf_expand_label(initial_secret, b"client in", b"", 32)
client_key = hkdf_expand_label(client_initial_secret, b"quic key", b"", 16)
client_iv = hkdf_expand_label(client_initial_secret, b"quic iv", b"", 12)
client_hp = hkdf_expand_label(client_initial_secret, b"quic hp", b"", 16)

print("computed client key: " + client_key.hex())
print("expected client key: 1f369613dd76d5467730efcbe3b1a22d")

