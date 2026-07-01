import json, base64, urllib.request
from collections import Counter

HELIUS = "https://mainnet.helius-rpc.com/?api-key=YOUR_KEY"

def rpc(method, params=[]):
    payload = json.dumps({"jsonrpc":"2.0","id":1,"method":method,"params":params}).encode()
    req = urllib.request.Request(HELIUS, data=payload, headers={"Content-Type":"application/json"})
    return json.loads(urllib.request.urlopen(req).read())["result"]

accounts = rpc("getProgramAccounts", [
    "KLend2g3cP87fffoy8q1mQqGKjrxjC8boSyAYavgmjD",
    {"encoding":"base64","dataSlice":{"offset":0,"length":8}}
])

print(f"total: {len(accounts)}")

disc = Counter()
for acc in accounts[:5000]:
    data = base64.b64decode(acc["account"]["data"][0])
    disc[data[:8].hex()] += 1

for d, count in disc.most_common(10):
    print(d, count)