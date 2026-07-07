use std::collections::HashMap;

const URL: &str = "https://puffin-validator.xyz/inclusion_delays";
pub const MAX_DELAY_MS: u64 = 150;

pub struct Validator { pub ms: u64, pub client: String, pub order: u8 }

pub async fn load() -> HashMap<String, Validator> {
    let html = match reqwest::get(URL).await {
        Ok(r) => r.text().await.unwrap_or_default(),
        Err(_) => return HashMap::new(),
    };
    let mut out = HashMap::new();
    let row = regex::Regex::new(r"<tr[^>]*>(.*?)</tr>").unwrap();
    let pk  = regex::Regex::new(r"<!--node-id\d+-->([1-9A-HJ-NP-Za-km-z]{32,44})<!--#-->").unwrap();
    let num = regex::Regex::new(r"<!--node-id\d+-->(\d+)<!--#-->").unwrap();
    let ord = regex::Regex::new(r"<!--node-id\d+-->(\d+)\s*%<!--#-->").unwrap();
    let cli = regex::Regex::new(
        r"<!--node-id\d+-->(Jito|Agave|Firedancer|Frankendancer|Rakurai|BAM|Harmonic[^<]*)<!--#-->",
    ).unwrap();

    for cap in row.captures_iter(&html) {
        let r = &cap[1];
        let pks: Vec<&str> = pk.captures_iter(r).map(|c| c.get(1).unwrap().as_str()).collect();
        if pks.is_empty() { continue; }
        let nums: Vec<u64> = num.captures_iter(r).filter_map(|c| c[1].parse().ok()).collect();
        if nums.len() < 2 { continue; }
        out.insert(pks[0].to_string(), Validator {
            ms:     nums[1],
            client: cli.captures(r).map(|c| c[1].to_string()).unwrap_or_default(),
            order:  ord.captures(r).and_then(|c| c[1].parse().ok()).unwrap_or(0),
        });
    }
    eprintln!("puffin: {} validators", out.len());
    out
}

pub fn check(leaders: &[String], table: &HashMap<String, Validator>) -> (bool, String) {
    for l in leaders.iter().take(4) {
        return match table.get(l) {
            Some(v) if v.ms <= MAX_DELAY_MS => (true,  format!("{}ms {} {}%", v.ms, v.client, v.order)),
            Some(v)                          => (false, format!("slow {}ms", v.ms)),
            None                             => (true,  "unknown".into()),
        };
    }
    (false, "no leaders".into())
}
