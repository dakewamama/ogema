import json, base64, struct, base58, sys, urllib.request

HELIUS = "https://mainnet.helius-rpc.com/?api-key=YOUR_KEY"
DEPOSIT_SLOT_SIZE = 72
BORROW_SLOT_SIZE = 144
NUM_SLOTS = 8

def get_account(pubkey):
    payload = json.dumps({"jsonrpc":"2.0","id":1,"method":"getAccountInfo","params":[pubkey,{"encoding":"base64"}]}).encode()
    req = urllib.request.Request(HELIUS, data=payload, headers={"Content-Type":"application/json"})
    return json.loads(urllib.request.urlopen(req).read())["result"]["value"]

def sf_to_float(raw: bytes) -> float:
    return int.from_bytes(raw, "little") / (2**60)

def parse_header(raw: bytes):
    off = 8
    tag = struct.unpack('<Q', raw[off:off+8])[0]; off += 8
    slot = struct.unpack('<Q', raw[off:off+8])[0]; off += 8
    stale = raw[off]; off += 1
    market = raw[off:off+32]; off += 32
    owner = raw[off:off+32]; off += 32
    return {"tag":tag,"slot":slot,"stale":stale,"market":base58.b58encode(market).decode(),"owner":base58.b58encode(owner).decode(),"off":off}

def parse_deposits(raw: bytes, off: int):
    deposits = []
    for i in range(NUM_SLOTS):
        reserve = raw[off:off+32]
        amount = struct.unpack('<Q', raw[off+32:off+40])[0]
        mv = raw[off+40:off+56]
        if amount > 0:
            deposits.append({"slot":i,"reserve":base58.b58encode(reserve).decode(),"amount":amount,"market_value":sf_to_float(mv)})
        off += DEPOSIT_SLOT_SIZE
    return deposits, off

def parse_borrows(raw: bytes, off: int):
    borrows = []
    for i in range(NUM_SLOTS):
        reserve = raw[off:off+32]
        borrowed_sf = raw[off+88:off+104]
        mv_sf = raw[off+104:off+120]
        bfv_sf = raw[off+120:off+136]
        amount = sf_to_float(borrowed_sf)
        if amount > 0:
            borrows.append({"slot":i,"reserve":base58.b58encode(reserve).decode(),"amount":amount,"market_value":sf_to_float(mv_sf),"borrow_factor_value":sf_to_float(bfv_sf)})
        off += BORROW_SLOT_SIZE
    return borrows, off

def parse(raw: bytes):
    h = parse_header(raw)
    deps, off = parse_deposits(raw, h["off"])
    borrs, off = parse_borrows(raw, off)
    dep_val = sum(d["market_value"] for d in deps)
    bor_val = sum(b["borrow_factor_value"] for b in borrs)
    hf = dep_val / bor_val if bor_val > 0 else float('inf')
    return {**h,"deposits":deps,"borrows":borrs,"deposit_value":dep_val,"borrow_value":bor_val,"health_factor":hf,"liquidatable":hf<1.0}

if __name__ == "__main__":
    pubkey = sys.argv[1] if len(sys.argv) > 1 else "JEJvqMqwyk2XGonDNfX4pPLNtKZCgxDJZP83YDWrxmjM"
    acc = get_account(pubkey)
    raw = base64.b64decode(acc["data"][0])
    r = parse(raw)
    print("owner:", r["owner"])
    print("health_factor:", r["health_factor"])
    print("liquidatable:", r["liquidatable"])
    print("deposits:", r["deposits"])
    print("borrows:", r["borrows"])