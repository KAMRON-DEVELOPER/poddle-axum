use serde::de::DeserializeOwned;

pub fn parse_value<T>(raw: &str) -> Option<T>
where
    T: DeserializeOwned,
{
    let raw = raw.trim();

    // Try JSON as-is
    // This should always be the highest-priority attempt
    if let Ok(v) = serde_json::from_str::<T>(raw) {
        return Some(v);
    }

    // Try CSV → JSON array fallback
    // In case T: Vec<String>
    //     Invalid JSON (missing quotes) - ENTRYPOINTS=[web,websecure] -> ["web","websecure"]
    //     Not JSON - ENTRYPOINTS=web,websecure -> ["web","websecure"]
    //     Invalid JSON - ENTRYPOINTS=web -> ["web","websecure"]
    if raw.contains(',') {
        let as_json = format!(
            "[{}]",
            raw.split(',')
                .map(|s| format!("\"{}\"", s.trim()))
                .collect::<Vec<String>>()
                .join(",")
        );

        if let Ok(v) = serde_json::from_str::<T>(&as_json) {
            return Some(v);
        }
    }

    // Try scalar JSON (string, number, bool)
    // wraps it in JSON string quotes
    // In case T: String
    //     ENV=web would never deserialize into String
    let quoted = format!("\"{}\"", raw);
    if let Ok(v) = serde_json::from_str::<T>(&quoted) {
        return Some(v);
    }

    None
}
