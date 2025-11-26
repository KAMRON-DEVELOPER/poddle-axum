use rand::{Rng, distr::Alphanumeric};

pub fn generate_username() -> String {
    // A small list of fun adjectives and nouns to make usernames memorable
    const ADJECTIVES: &[&str] = &[
        "fast", "silent", "brave", "clever", "bright", "fuzzy", "calm", "happy", "wise", "bold",
        "swift", "chill", "tiny", "lucky", "cool",
    ];
    const NOUNS: &[&str] = &[
        "lion", "panda", "eagle", "wolf", "otter", "fox", "tiger", "whale", "owl", "bear", "hawk",
        "cat", "dog", "crab", "sloth",
    ];

    let mut rng = rand::rng();

    let adj = ADJECTIVES[rng.random_range(0..ADJECTIVES.len())];
    let noun = NOUNS[rng.random_range(0..NOUNS.len())];

    // Append 4 random alphanumeric characters for uniqueness
    let rand_suffix: String = rng
        .sample_iter(&Alphanumeric)
        .take(4)
        .map(char::from)
        .collect::<String>()
        .to_lowercase();

    format!("{}_{}_{rand_suffix}", adj, noun)
}
