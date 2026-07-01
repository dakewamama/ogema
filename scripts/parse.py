import json, base64, struct, base58, sys, urllib.request

HELIUS_URL = "https://mainnet.helius-rpc.com/?api-key=YOUR_KEY"

def get_account(pubkey):
    payload = json.dumps({
        "jsonrpc": "2.0", "id": 1,
        "method": "getAccountInfo",
        "params": [pubkey, {"encoding": "base64"}]
    }).encode()
    req = urllib.request.Request(HELIUS_URL, data=payload, headers={"Content-Type": "application/json"})
    resp = urllib.request.urlopen(req)
    return json.loads(resp.read())["result"]["value"]

def parse_obligation_header(raw: bytes):
    off = 8
    tag = struct.unpack('<Q', raw[off:off+8])[0]; off += 8
    slot = struct.unpack('<Q', raw[off:off+8])[0]; off += 8
    stale = raw[off]; off += 1
    lending_market = raw[off:off+32]; off += 32
    owner = raw[off:off+32]; off += 32
    return {
        "tag": tag,
        "last_update_slot": slot,
        "stale": stale,
        "lending_market": base58.b58encode(lending_market).decode(),
        "owner": base58.b58encode(owner).decode(),
        "header_end_offset": off,
        "remaining_bytes": len(raw) - off,
    }

if __name__ == "__main__":
    pubkey = sys.argv[1] if len(sys.argv) > 1 else "JEJvqMqwyk2XGonDNfX4pPLNtKZCgxDJZP83YDWrxmjM"
    acc = get_account(pubkey)
    raw = base64.b64decode(acc["data"][0])
    print("owner program:", acc["owner"])
    print("size:", len(raw))
    info = parse_obligation_header(raw)
    for k, v in info.items():
        print(k, ":", v)