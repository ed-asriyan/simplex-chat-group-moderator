# Group Moderator Rules Configuration
The bot uses YAML configuration to let you configure multiple distinct moderation rules at once.
To update your group's rules, simply reply to the bot's configuration message with your new YAML text.

> **Tip:** Use an online YAML editor such as [The Tool App — Edit YAML](https://thetoolapp.com/code-tools/edit-yaml) to compose or validate your config before sending it.

## Rule Structure
Every configuration is a list (`-`) of rules. Every rule **must** have exactly two root fields:
- `type`: The kind of rule (e.g., `WordsBlacklist`, `LinksBlacklist`, `LinksWhitelist`, `LinksWhitelistTop100`).
- `parameters`: An object containing the configuration for the rule.

Order of rules does not matter.

> Note: You can create multiple rules of the same `type` if you want to organize them differently!

## Removing All Rules
To disable all moderation and remove every rule, reply with an empty list:
```yaml
[]
```

---

## Quick Example
Not sure where to start? Here is a ready-to-use config that covers the most common cases — block spam words, block known bad domains, and only allow links from trusted sites:

```yaml
# Block messages containing spam words
- type: WordsBlacklist
  parameters:
    keywords:
      - spam
      - crypto scam
      - buy followers

# Block links to known bad domains
- type: LinksBlacklist
  parameters:
    blocked:
      - t.me
      - bit.ly
```

Copy it, adjust the lists to your needs, and reply to the bot's configuration message with the result.

---

## Available Rule Types
### 1. WordsBlacklist
Deletes messages that contain certain forbidden words. Matches are case-insensitive and attempt to bypass common obfuscation (like `b@dw0rd` instead of `badword`).

**Template:**
```yaml
- type: WordsBlacklist
  parameters:
    keywords:
      - spam
      - cryptoscam
      - badword
```

**Fields:**
- `keywords` (List of Strings): The list of words or phrases to block. Max 100 characters per word. Max 10,000 words per group.

**Resources with bad words:**
- **🔞 List of Dirty, Naughty, Obscene, and Otherwise Bad Words** — multilanguage, ~400 EN / ~1700 total  
  https://github.com/LDNOOBW/List-of-Dirty-Naughty-Obscene-and-Otherwise-Bad-Words
- **☕ Google Profanity Words** — multilanguage, ~1k EN / ~1600 total  
  https://github.com/coffee-and-fun/google-profanity-words/tree/main/data
- **💬 Comment Blocklist for WordPress** — multilanguage, ~64k total  
  https://github.com/splorp/wordpress-comment-blocklist

---

### 2. LinksBlacklist
Deletes messages that contain URLs with domains matching the blocked list.

**Template:**
```yaml
- type: LinksBlacklist
  parameters:
    blocked:
      - phishing-site.com
      - evil.com
```

**Fields:**
- `blocked` (List of Strings): A blacklist of domains. If a message contains a link to any of these domains, it will be deleted.

> **Note:** `LinksBlacklist` and `LinksWhitelist` contradict each other — using both at the same time is not recommended. If you use `LinksWhitelist`, it already blocks every domain not on the allowed list, making a separate `LinksBlacklist` redundant.

---

### 3. LinksWhitelist
Deletes messages that contain URLs, *unless* they only point to domains allowed in the whitelist.

**Template:**
```yaml
- type: LinksWhitelist
  parameters:
    allowed:
      - github.com
      - wikipedia.org
```

**Fields:**
- `allowed` (List of Strings): A whitelist of domains. If a message contains a link to ANY domain not on this list, it will be deleted.

> **Note:** `LinksWhitelist` and `LinksBlacklist` contradict each other — using both at the same time is not recommended. `LinksWhitelist` already blocks every unlisted domain, so adding a `LinksBlacklist` on top has no practical effect.

To prohibit all links, use this template:
```yaml
- type: LinksWhitelist
  parameters:
    allowed: []
```

---

### 4. LinksWhitelistTop100
Deletes messages that contain URLs, *unless* they only point to domains from [a built-in preset list of ~100 widely-used](https://github.com/simplex-chat/group-moderator/blob/master/bot/src/domain/moderator/message_filter/links/top100.rs), legitimate sites (Google, GitHub, Wikipedia, YouTube, Reddit, etc.).

No configuration is needed — the list is maintained inside the bot.

**Template:**
```yaml
- type: LinksWhitelistTop100
  parameters: {}
```

> **Note:** `LinksWhitelistTop100` and `LinksBlacklist` contradict each other for the same reason as `LinksWhitelist` — using both is not recommended.

> **Tip:** If the built-in list doesn't suit your group, use `LinksWhitelist` instead and specify your own domains.

---


