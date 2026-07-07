const SOL_FEED: &str =
    "https://hermes.pyth.network/v2/updates/price/latest\
     ?ids[]=0xef0d8b6fda2ceba41da15d4095d1da392a0d2f8ed0c6c7bc0f4cfac8c280b56d\
     &ids[]=0xeaa020c61cc479712813461ce153894a96a6c00b21ed0cfc2798d1f9a9e9c94a\
     &ids[]=0x2b89b9dc8fdf9f34709a5b106b472f0f39bb6ca9ce04b0fd7f2e971688e2e53b";

pub async fn prices() -> Vec<(String, f64)> {
    let Ok(resp) = reqwest::get(SOL_FEED).await else { return vec![] };
    let Ok(data) = resp.json::<serde_json::Value>().await else { return vec![] };
    data["parsed"].as_array().unwrap_or(&vec![]).iter().map(|p| {
        let raw  = p["price"]["price"].as_str().unwrap_or("0").parse::<i64>().unwrap_or(0);
        let expo = p["price"]["expo"].as_i64().unwrap_or(0);
        let id   = format!("0x{}", p["id"].as_str().unwrap_or(""));
        (id, raw as f64 * 10f64.powi(expo as i32))
    }).collect()
}
