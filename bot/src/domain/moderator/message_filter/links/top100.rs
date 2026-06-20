/// Built-in preset allowlist of widely-used, legitimate domains.
/// Used by the `LinksWhitelistTop100` rule — no configuration needed.
///
/// Subdomains are automatically covered: allowing `github.com` also allows
/// `gist.github.com`, `docs.github.com`, etc.
///
/// To extend or override this list use `LinksWhitelist` instead.
pub static DOMAINS: &[&str] = &[
    // Search & portals
    "google.com",
    "bing.com",
    "yahoo.com",
    "duckduckgo.com",
    "yandex.ru",
    "baidu.com",
    // Social networks
    "youtube.com",
    "facebook.com",
    "twitter.com",
    "x.com",
    "instagram.com",
    "linkedin.com",
    "reddit.com",
    "tiktok.com",
    "snapchat.com",
    "pinterest.com",
    "tumblr.com",
    "vk.com",
    "ok.ru",
    "twitch.tv",
    "discord.com",
    // Messaging
    "telegram.org",
    "whatsapp.com",
    "signal.org",
    // Reference
    "wikipedia.org",
    "wikimedia.org",
    "archive.org",
    "mozilla.org",
    // Tech & dev
    "github.com",
    "gitlab.com",
    "bitbucket.org",
    "stackoverflow.com",
    "npmjs.com",
    "pypi.org",
    "crates.io",
    "docker.com",
    "rust-lang.org",
    "python.org",
    "nodejs.org",
    // Hosting & cloud
    "cloudflare.com",
    "netlify.com",
    "vercel.com",
    "heroku.com",
    "aws.amazon.com",
    // Productivity
    "notion.so",
    "slack.com",
    "zoom.us",
    "dropbox.com",
    "trello.com",
    "atlassian.com",
    "figma.com",
    "canva.com",
    // Microsoft
    "microsoft.com",
    "office.com",
    "outlook.com",
    "skype.com",
    // Apple
    "apple.com",
    "icloud.com",
    // Media & entertainment
    "spotify.com",
    "soundcloud.com",
    "netflix.com",
    "vimeo.com",
    "dailymotion.com",
    "imgur.com",
    "giphy.com",
    // News
    "bbc.com",
    "bbc.co.uk",
    "cnn.com",
    "nytimes.com",
    "theguardian.com",
    "reuters.com",
    "techcrunch.com",
    "theverge.com",
    "arstechnica.com",
    "wired.com",
    // Shopping & payments
    "amazon.com",
    "ebay.com",
    "etsy.com",
    "paypal.com",
    "stripe.com",
    // Publishing & blogs
    "medium.com",
    "substack.com",
    "wordpress.com",
    "blogger.com",
    "hashnode.com",
    "dev.to",
    // Design
    "dribbble.com",
    "behance.net",
    "adobe.com",
    "unsplash.com",
    "freepik.com",
    // Privacy & email
    "proton.me",
    "protonmail.com",
    "simplex.chat",
    "asriyan.me",
    "runonflux.com",
];
