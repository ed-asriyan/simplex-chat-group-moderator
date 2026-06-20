# Group Moderator Rules Configuration
The bot uses YAML configuration to let you configure multiple distinct moderation rules at once.
To update your group's rules, simply reply to the bot's configuration message with your new YAML text.

## Rule Structure
Every configuration is a list (`-`) of rules. Every rule **must** have:
- `type`: The kind of rule (e.g., `Keywords` or `Link`).
- `enabled`: `true` or `false` to toggle this specific rule without deleting it.

> Note: You can create multiple rules of the same `type` if you want to organize them differently!

---

## Available Rule Types

### 1. Keywords
Deletes messages that contain certain forbidden words. Matches are case-insensitive and attempt to bypass common obfuscation (like `b@dw0rd` instead of `badword`).

**Template:**
```yaml
- type: Keywords
  enabled: true
  keywords:
    - spam
    - cryptoscam
    - badword
```

**Fields:**
- `keywords` (List of Strings): The list of words or phrases to block. Max 100 characters per word. Max 10,000 words per group.

---

### 2. Link (Coming Soon)
A flexible rule to moderate URLs containing `http`, `https`, or `www.` in messages. 

**Template:**
```yaml
- type: Link
  enabled: true
  inclusive: false
  allow_top100: true
  allowed: 
    - github.com
    - wikipedia.org
  blocked: 
    - phishing-site.com
```

**Fields:**
- `inclusive` (Boolean): 
  - If `false` (Exclusive mode): The bot will **delete** any link **EXCEPT** those explicitly listed in `allowed` or `allow_top100`.
  - If `true` (Inclusive mode): The bot will only delete links that are explicitly listed in `blocked`.
- `allow_top100` (Boolean): If `true`, automatically allows the top 100 most popular internet domains (e.g. google.com, youtube.com) avoiding the need to manually whitelist them.
- `allowed` (List of Strings): A whitelist of domains to always allow. 
- `blocked` (List of Strings): A blacklist of domains to always delete.
