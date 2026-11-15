# User stories

The requirements baseline for the bot and its web editor. The design, the ports, the schema and the code are derived from this file; when they disagree, fix this file first and then the code.

This is a living document: it always describes the current intended behaviour, and stories are rewritten or removed as that changes. Story ids (`US-<epic><n>`) are what the code and its tests cite, so a change here that renumbers stories updates the references there in the same commit.

## The bot in one paragraph
A group owner adds the bot to their [SimpleX Chat](https://simplex.chat) group with the moderator role or higher. The bot then checks every message a member posts, and everyone who joins, against the group's **rules**, which the owner sets up in a visual **web editor** reached through a link the bot sends. A rule has a **name**, a **trigger** — *someone sends a message* or *someone joins* —, a **condition** (a message contains a listed word, a link outside a list, the author posts too fast, the person's name advertises a website, …, or any combination of them) and the **actions** to take when it matches (delete the message, make the member an observer, kick them). Each rule has two switches: *Actions*, whether the bot carries out its actions or only checks it, and *Notifications*, whether it tells the owner when the rule matches — so a rule can be tried out, telling the owner what it would do, before it acts, and turned off without being deleted.

## Roles
- **Owner** — the person who invited the bot to a group. They configure it, and only they get its notifications. The owner of the group in the bot's sense need not be the group's owner in SimpleX.
- **Member** — anyone in the group whose SimpleX role is *member*. Only a member's messages are checked, and only someone who joins as a member or an observer; a moderator's, an admin's and an owner's messages and joining never are, and an observer cannot write.

## Conventions
- The owner talks to the bot in a direct chat, in English. The bot writes in standard Markdown, as SimpleX renders it (`**bold**`, `_italic_`, never a single `*` for bold); a bare `/command` in its text is tappable.
- The examples use the owner Alice, her group **Crypto Chat** (the bot's id for it: 7), and the member Bob. The editor's address is `https://ed-asriyan.github.io/simplex-chat-group-moderator`; a link to it carries the group's id, name and settings after `#`, shown here as `#bot_id=7&group=Crypto%20Chat&config=…`.

## A. Talking to the bot
- **US-A1.** As a person, when I connect to the bot, it sends me the welcome:

  ```text
  Hi! 👋 I'm a **moderation bot** for SimpleX groups. You set the rules, and I catch:

  - 🚫 **Banned words**, even disguised like b@d_w0rd
  - 🔗 **Links** to websites you don't allow
  - 🌊 **Floods**: too many messages, walls of text
  - 🚪 **Spammers joining**, by names that advertise
  - 🤖 **Anything you can describe**, checked by AI with your own key

  Then I 🗑 delete the message, 👁 make the member an observer, or 🚪 kick them.

  ### 🚀 Get started
  Invite me to your group as **Moderator** to delete messages, **Admin** to also make members observers, or **Owner** to also kick them. Then send /groups to set up the rules.

  ### 👀 Not ready to invite me?
  [Try the editor on a demo group](https://ed-asriyan.github.io/simplex-chat-group-moderator): see exactly what you would set up, without inviting me or giving me any role.

  Send /help for the commands. My code is open [on GitHub](https://github.com/ed-asriyan/simplex-chat-group-moderator).
  ```
- **US-A2.** As a person, when I send `/start`, I get the welcome of US-A1.
- **US-A3.** As a person, when I send `/help`, I get:

  ```text
  ### How to use this bot

  1. Invite me to your group as:
     - **Moderator** — to delete violating messages.
     - **Admin** — to also set violators as observers.
     - **Owner** — to also kick violators out of the group.
  2. Once I join, send /groups to list your groups and open the visual editor.
  3. I watch the chat and everyone who joins, and act according to your rules.

  To stop me moderating a group, just remove me from it.

  ### Each rule's switches

  Every rule has two, set in the same editor (send /groups to get the link):

  - **Actions** — on, I carry out its actions; off, I still check it but do nothing in the group.
  - **Notifications** — on, I DM you every time it matches, with what I did or would have done; off, I never DM you about it.

  ### Commands

  - /start — show the welcome guide.
  - /help — show this guide.
  - /groups — list and manage the groups I moderate for you.
  - /source — view my source code.
  - /issue — report a bug or unexpected moderation.
  - /feature — request a new moderation rule type or feature.
  ```
- **US-A4.** As a person, when I send `/source`, I get `My source code is [on GitHub](https://github.com/ed-asriyan/simplex-chat-group-moderator).`
- **US-A5.** As a person, when I send `/issue`, I get `🐞 Please report bugs and unexpected moderation [on GitHub](https://github.com/ed-asriyan/simplex-chat-group-moderator/issues/new?template=moderation-rule-bug.yml).`, a form that asks which condition and action the report is about.
- **US-A6.** As a person, when I send `/feature`, I get `💡 Please request new moderation rule types and features [on GitHub](https://github.com/ed-asriyan/simplex-chat-group-moderator/issues/new?template=feature-request.yml).`
- **US-A7.** As a person, when I send anything else — another command, plain text, an empty message — I get `Unknown command. Send /help to see what I can do.`

## B. Groups
- **US-B1.** As an owner, when I invite the bot to Crypto Chat with the moderator role or higher, it joins and I get two messages: `✅ I joined the group. Open the editor below to set up its rules.`, then the group's editor link (US-B4). I am the group's owner for the bot from then on. A new group has no rules.
- **US-B2.** As a person, when I invite the bot to a group with a role below moderator, it does not join, and I get `❌ I can't join this group: I need the **moderator** role or higher to delete messages. Invite me again with that role.`
- **US-B3.** As a person, when the bot cannot join a group I invited it to (the invitation expired, or the group is the bot's already), I get `❌ I couldn't join the group. Check that the invitation is still valid and send it again.`
- **US-B4.** As an owner, when I send `/groups`, I get one message per group I own:

  ```text
  **Crypto Chat**
  [View and Edit Settings](https://ed-asriyan.github.io/simplex-chat-group-moderator#bot_id=7&group=Crypto%20Chat&config=…)
  ```

  The link carries the group's name and its whole current configuration — its rules with their switches, those turned off included —, so the editor opens on exactly what the bot has. The group's name is its current one: the bot takes it from every message posted there.
- **US-B5.** As a person who owns no group, when I send `/groups`, I get `You don't have any groups yet. Invite me to your group as **moderator** or higher to get started — /help explains how. Want to look around first? [Try the editor on a demo group](https://ed-asriyan.github.io/simplex-chat-group-moderator).`
- **US-B6.** As an owner, when I remove the bot from Crypto Chat, it forgets the group and its rules: the group is no longer in `/groups`.

## C. Saving a group's configuration
- **US-C1.** As an owner, when I send the bot the link the editor copied (US-G8), alone or anywhere in a message, the group's whole configuration — all its rules with their switches — replaces the old one, and I get `✅ Group settings updated.` The bot moderates by the new configuration from the next message or join on.
- **US-C2.** As an owner, when the configuration I send is not valid, nothing of it is saved, the old one stays, and I get the reason quoted:

  ```text
  ❌ **Failed to update group settings:**

  > A rule has no actions. Add at least one action to it, or remove the rule.
  ```

  A configuration is not valid when, for instance, a rule has no action, a rule for when someone sends a message has no condition (`A rule for when someone sends a message has no condition. Add one, or remove the rule.`), a rule holds a condition or an action its trigger does not take (`'Moderate the message' does not work in a rule for when someone joins.`), a rule's name is longer than 60 characters (`A rule's name is longer than 60 characters.`), a composite condition is empty, a condition tree is deeper than 8 levels or has more than 64 conditions, a regex pattern does not compile, a number is outside its range, or *Author Hits Moderation Rate Limit* sits under a *Not* (`'Author Hits Moderation Rate Limit' cannot be placed under a 'Not' condition, because it already depends on what the other rules do with this message.`). A link the bot cannot decompress gets `Could not decompress the settings from the URL. Open the editor link again, make your changes, and send back the updated URL.` as the reason.
- **US-C3.** As a person, when I send a link for a group I do not own, nothing is saved and I get the refusal of US-C2 with the reason `User <my id> is not the owner of group 7`.
- **US-C4.** As an owner, when the configuration I send has an AI condition (US-E25, US-E26), the bot first checks each API key with its provider, and saves nothing if one does not work, giving the provider's verdict as the reason, e.g. `OpenRouter rejected the API key …a1b2. Check that it was copied whole and has not been deleted or disabled at openrouter.ai/settings/keys.` A key is only ever shown by its last characters.
- **US-C5.** As an owner, when I send a link to the editor without its settings (no `bot_id` or no `config` after `#`), the bot does not read it as settings, and I get the answer of US-A7.

## D. Moderation
- **US-D1.** As an owner, when a member posts in Crypto Chat, the bot checks the message against the group's rules for *💬 When someone sends a message*, in order. A rule with *Actions* on contributes actions the bot carries out; a rule with *Actions* off and *Notifications* on, actions it only tells me it would carry out (US-D4); a rule with both off is not checked. A rule whose every action is already covered is not checked: a rule that is on, by the rules that are on and matched before it; a rule that is off, by every rule that matched before it. So with two rules that are on and both moderate the message, only the first one that matches counts, and a rule that is off covers nothing for the rules that are on. The bot merges the actions (US-F4), carries out those of the rules that are on, and tells me according to each rule's *Notifications* (US-D2–US-D4). An edited message is checked again, as it is after the edit.
- **US-D2.** As an owner, when a member's message matches a rule with *Actions* and *Notifications* on, the bot carries out the actions and tells me:

  ```text
  🛡 I set the member as observer for 60 minutes and moderated the message in **Crypto Chat**.

  **The message:**
  > Buy cheap followers at https://spam.example

  **Reason:**
  - **Links from newcomers**: spam.example

  If it's a false positive, please [file an issue](https://github.com/ed-asriyan/simplex-chat-group-moderator/issues/new?template=moderation-rule-bug.yml).
  ```

  The actions are named in the order they were carried out — the observer first, then the message, then the kick (`set the member as observer`, `… for 1 minute`, `… for 60 minutes`, `moderated the message`, `kicked the member`, `kicked the member and deleted all their messages`) — the last joined by `and`. Each matched rule gives one reason line, in the order of the rules: its name in bold, then what matched; a rule without a name gives what matched alone (`- spam.example`). A rule that is not checked because the rules before it already cover its actions (US-D1) gives none, except as US-E20 says. The notification names the actions of the matched rules with *Notifications* on, merged, and gives their reason lines alone: a rule with *Notifications* off that matched the same message still has its actions carried out, but it is not named. The message is quoted with its line breaks, cut after 300 characters with `...`, and a message without text shows as `_(no text)_`.
- **US-D3.** As an owner, when a member's message matches only rules with *Notifications* off, the bot carries out the actions of those with *Actions* on and does not tell me.
- **US-D4.** As an owner, when a member's message matches a rule with *Actions* off and *Notifications* on, the bot does nothing for it in the group and tells me what it would have done, in the form of US-D2 with `🛡 I would moderate the message in **Crypto Chat**.` (`would … kick the member`, `would … kick the member and delete all their messages`; `set the member as observer` reads the same), and the reason line of the rule marked after its name: `- **Flood** _(off)_: author sent 12 messages in 1 min` (`- _(off)_ author sent 12 messages in 1 min` without a name). When the same message also matches a rule with both on, I get one notification, what was done first and then what would have been: `🛡 I moderated the message in **Crypto Chat**, and would also kick the member.`, with the reason lines of both rules in the order of the rules. An action the bot would carry out that one it carried out already covers (US-F4) is not named.
- **US-D5.** As an owner, a moderator's, an admin's or an owner's message in my group is never checked, nor their joining, whatever the rules say.
- **US-D6.** As an owner, when someone joins Crypto Chat as a member or an observer — by the group's link, or added by another member —, the bot checks them against the group's rules for *🚪 When someone joins*, in order, skipping the rules with both switches off and those whose every action is already covered, as in US-D1. Every rule that matches contributes its actions; the bot merges them (US-F4), carries out those of the rules with *Actions* on, and tells me as for a message (US-D2–US-D4). With a rule *Spam names* with both switches on, when Bob joins with the display name `Free Casino Bonus`, I get:

  ```text
  🛡 I kicked the member when they joined **Crypto Chat**.

  **The member:** Free Casino Bonus

  **Reason:**
  - **Spam names**: name contains word: 'casino'

  If it's a false positive, please [file an issue](https://github.com/ed-asriyan/simplex-chat-group-moderator/issues/new?template=moderation-rule-bug.yml).
  ```

  The actions are named as in US-D2 (`set the member as observer for 60 minutes`, `kicked the member`, `kicked the member and deleted all their messages`), and the reason lines too; a rule with no condition gives `everyone who joins` (`- **Quarantine newcomers**: everyone who joins`). With *Notifications* off, the bot acts and does not tell me; with *Actions* off and *Notifications* on, it does nothing in the group and tells me `🛡 I would kick the member when they joined **Crypto Chat**.`, with the reason line `- **Spam names** _(off)_: name contains word: 'casino'`.

## E. Conditions
A rule's condition is one of the conditions below, or a combination of them (US-E27–US-E30). The conditions work in rules for *💬 When someone sends a message*, and the editor offers no other there; those of *The member's name* (US-E31, US-E32) and the combinations work in rules for *🚪 When someone joins* too, and are the only ones offered there. Each story starts with what I see when I add the condition in the editor: its title, and its fields as the editor labels them, with their defaults and limits. A matched condition gives the reason line of my notification (US-D2, US-D6), shown here after it.

### Text
- **US-E1.** As an owner, when I add **💬 Message Contains Any of These Words**, I see *Words / Phrases*, a list of up to 10000 entries of up to 100 characters each. With `spam` and `blow job`, a member's message matches when it contains one of them as a whole word or phrase — never as a substring (`ass` does not match `classic`) — however it is disguised: capitals, leet-speak (`b@dw0rd`), Latin/Cyrillic look-alikes (`sраm`, with a Cyrillic `р` and `а`), styled or full-width letters, upside-down text, spaced-out letters (`s.p.a.m`), flooded letters (`spaaaam`), `@mentions`, English plurals and endings (`spams`), and, for a word of five letters or more, written split or joined (`blowjob`). Reason: `contains word: 'spam'`.
- **US-E2.** As an owner, when I add **💬 Message Exactly Matches Any of These Texts**, I see *Texts*, a list of up to 10000 entries of up to 1000 characters each, and *Case sensitive*, off. With `buy now`, a message matches when its whole text, without leading and trailing spaces, is one of the texts: `Buy Now ` matches, `buy now please` does not; with *Case sensitive* on, `Buy Now` does not either. Reason: the text it matched (`buy now`).
- **US-E3.** As an owner, when I add **💬 Message Matches Any of These Regex Patterns**, I see *Regex Patterns*, a list of up to 100 patterns of up to 200 characters each. With `\d{4,}`, a message matches when its raw text matches one of the patterns, in Rust `regex` syntax (`call 5551234`); nothing is undone for it as for words. Reason: `matches regex pattern: '\d{4,}'`.
- **US-E4.** As an owner, when I add **💬 Message Contains Repeated Sequence**, I see *Minimum Repeats in a Row*, 5 (at least 2), and *Minimum Sequence Length*, 1 (1 to 50 characters). A message matches when some sequence of at least that length repeats back-to-back that many times (`aaaaa`, `!!!!!`, `hahahahaha`), letter case aside. Reason: `"ha" repeated 5 times in a row`.

### Links
Each link condition sees through disguised links (`spam [dot] com`, `hxxp://`); a link with no `http://` or `www.` counts only when it ends in a common domain ending (`.com`, `.ru`, `.io`, `.me`, `.chat`, `.xyz`, …). A listed website covers its subdomains. The reason is the link's domain (`spam.example`).

- **US-E5.** As an owner, when I add **🔗 Message Contains a Link to Any of These Websites**, I see *Websites*, a list of up to 10000 domains of up to 100 characters each. With `spam.example`, a message matches when it links to `spam.example` or `www.spam.example`.
- **US-E6.** As an owner, when I add **🔗 Message Contains a Link to a Website Outside This List**, I see *Websites*. With `simplex.chat`, a message matches when it links to any other website; `https://simplex.chat/blog` does not match. With an empty list, any link matches.
- **US-E7.** As an owner, when I add **🔗 Message Contains a Link Outside Top 100 Websites**, I see *Extra Websites Added to Top 100*. A message matches when it links to a website neither among the top 100 (Google, GitHub, Wikipedia, YouTube, Reddit, …) nor in that list: `https://github.com/simplex-chat` does not match, `https://spam.example` does.

### Shape and size
- **US-E8.** As an owner, when I add **📏 Message Is Empty or Blank**, I see no field. A message matches when it is only whitespace, line breaks or invisible characters, and never when it carries an attachment. Reason: `empty message`.
- **US-E9.** As an owner, when I add **📏 Message Contains Invisible Characters**, I see no field. A message matches when it has even one zero-width, Braille-blank, bidi or variation character, emoji joined into one aside. Reason: `invisible characters`.
- **US-E10.** As an owner, when I add **📏 Message Exceeds Max Characters**, I see *Max Characters*, 1000 (at least 1). A message of 1203 characters matches. Reason: `1203 characters`.
- **US-E11.** As an owner, when I add **📏 Message Exceeds Max Words**, I see *Max Words*, 150 (at least 1). A message of 152 words matches. Reason: `152 words`.
- **US-E12.** As an owner, when I add **📏 Message Exceeds Max Lines**, I see *Max Lines*, 10 (at least 1), and *Characters per Line (Screen Wrap)*, 40 (0 or more). A line longer than 40 characters counts as several, as it wraps on a phone; with 0, only line breaks count. A message of 12 lines matches. Reason: `12 lines`.

### Attachments
A caption is the message's text for every other condition.

- **US-E13.** As an owner, when I add **📎 Message Contains an Image**, I see no field. A message carrying an image matches. Reason: `contains an image`.
- **US-E14.** As an owner, when I add **📎 Message Contains a Video**, I see no field. A message carrying a video matches. Reason: `contains a video`.
- **US-E15.** As an owner, when I add **📎 Message Contains a Voice Message**, I see no field. A voice message matches. Reason: `contains a voice message`.
- **US-E16.** As an owner, when I add **📎 Message Contains a File**, I see no field. A message carrying any other attachment matches. Reason: `contains a file`.

### The author
Every rate limit counts the message being checked, over a *Time Window (Minutes)* of 1 to 60.

- **US-E17.** As an owner, when I add **👤 Author Hits Message Rate Limit**, I see *Number of Messages*, 5 (at least 1), and *Time Window (Minutes)*, 1. A member's fifth message within a minute matches. Reason: `author sent 5 messages in 1 min`.
- **US-E18.** As an owner, when I add **👤 Author Hits Character Rate Limit**, I see *Number of Characters*, 2000 (at least 1), and *Time Window (Minutes)*, 1. A message that brings the member's characters within the window to 2000 or more matches. Reason: `author sent 2140 characters in 1 min`.
- **US-E19.** As an owner, when I add **👤 Author Hits Line Rate Limit**, I see *Number of Lines*, 30 (at least 1), *Time Window (Minutes)*, 1, and *Characters per Line (Screen Wrap)*, 40, lines counted as in US-E12. A message that brings the member's lines within the window to 30 or more matches. Reason: `author sent 31 lines in 1 min`.
- **US-E20.** As an owner, when I add **👤 Author Hits Moderation Rate Limit**, I see *Number of Moderated Messages*, 3 (at least 1), and *Time Window (Minutes)*, 60. It matches when that many of the author's messages were moderated in the window, the one being checked included when another rule that is on moderates it — a message a rule that is off would moderate does not count —, and then matches the author's next messages too until the window has room. It cannot sit under a *Not* (US-C2). Reason: `author had 3 messages moderated in 60 min`. While a rule has it, the notification always names the first rule that matched the message, after the other reasons, even when that rule was not checked because the rules before it already cover its actions (US-D1): with this limit first and *Contains Any of These Words* `spam` second, both moderating, the third `spam` lists `author had 3 messages moderated in 60 min`, then `contains word: 'spam'`.
- **US-E21.** As an owner, when I add **👤 Author Joined Recently**, I see *Joined Less Than (Minutes) Ago*, 60 (at least 1). A message from a member who joined less than that long ago matches; a member who was in the group before the bot joined never does. Reason: `author joined 12 min ago (less than 60 min)`.

### The member's name
A name is the display name and the full name of the person's SimpleX profile. In a rule for *💬 When someone sends a message*, these conditions read the name of the message's author; in a rule for *🚪 When someone joins*, the name of the person who joins.

- **US-E31.** As an owner, when I add **👤 Member's Name Contains Any of These Words**, I see *Words / Phrases*, a list of up to 10000 entries of up to 100 characters each. With `casino`, Bob joining as `Free Casino Bonus` matches, and so does every message he posts in a rule for messages. A word is found as in US-E1: as a whole word or phrase, however it is disguised. Reason: `name contains word: 'casino'`.
- **US-E32.** As an owner, when I add **👤 Member's Name Contains a Link Outside This List**, I see *Websites*, a list of up to 10000 domains of up to 100 characters each. A name matches when it holds a link to a website not in the list, seen through disguises as in *Links* (`Bob | spam [dot] example`); with an empty list, any link matches. With `simplex.chat`, `Bob (simplex.chat)` does not match. Reason: `name links to spam.example`.

### The group
The group's limits count every member's messages together, so they catch a raid by accounts that each stay under the author's limits. An edit does not count.

- **US-E22.** As an owner, when I add **👥 Group Hits Message Rate Limit**, I see *Number of Messages*, 30 (at least 1), and *Time Window (Minutes)*, 1. The group's 30th message within a minute matches. Reason: `group received 30 messages in 1 min`.
- **US-E23.** As an owner, when I add **👥 Group Hits Character Rate Limit**, I see *Number of Characters*, 10000 (at least 1), and *Time Window (Minutes)*, 1. Reason: `group received 10400 characters in 1 min`.
- **US-E24.** As an owner, when I add **👥 Group Hits Line Rate Limit**, I see *Number of Lines*, 150 (at least 1), *Time Window (Minutes)*, 1, and *Characters per Line (Screen Wrap)*, 40, lines counted as in US-E12. Reason: `group received 152 lines in 1 min`.

### AI
Each AI condition uses my own API key, shown masked with a button to show it. Its key is checked when I save (US-C4). A call that fails is tried again up to *Attempts* times (3, from 1 to 5), *Pause, s* apart (1, from 0 to 10 seconds); when the provider cannot be reached, the condition does not match. Only a message with text is checked.

- **US-E25.** As an owner, when I add **💬 Flagged by OpenAI Omni**, I see *OpenAI API Key*, then one field per category — *Sexual*, *Sexual (minors)*, *Harassment*, *Harassment (threatening)*, *Hate*, *Hate (threatening)*, *Illicit*, *Illicit (violent)*, *Self-harm*, *Self-harm (intent)*, *Self-harm (instructions)*, *Violence*, *Violence (graphic)* — each *OpenAI decides*, or *Off*, or a minimum score from 1 to 100 percent, then *Attempts* and *Pause, s*. Each message goes to OpenAI's Moderation API, and it matches when a category is past its trigger. Reason: `flagged by OpenAI Omni: harassment (OpenAI), hate 87% ≥ 80%`.
- **US-E26.** As an owner, when I add **💬 Flagged by OpenRouter Instruction**, I see *OpenRouter API Key*; *Model*, `google/gemini-2.5-flash-lite`, out of `google/gemini-2.5-flash-lite`, `mistralai/mistral-small-3.2-24b-instruct`, `meta-llama/llama-3.3-70b-instruct`, `meta-llama/llama-4-maverick`, `openai/gpt-4.1-nano`, `openai/gpt-4o-mini`, `openai/gpt-4.1-mini`, `anthropic/claude-haiku-4.5`, `openai/gpt-4o` and `openai/gpt-4.1`; *Instruction*, several lines of up to 4000 characters; *Context*, 0 (up to 10), the earlier messages sent as context; then *Attempts* and *Pause, s*. With the instruction "Match advertising: crypto promotions, casinos, unsolicited offers. Do not match normal conversation.", each message goes to the model with the instruction and, if I ask for them, that many earlier messages of the group as context, each with its author's display name, the checked message with its author's too. Messages the bot deleted are left out, and the bot keeps the earlier messages in memory only, for 24 hours at most, so there are fewer or none right after saving or a restart. It matches when the model answers yes; when it gives no clear answer, it does not. Only providers that do not keep the text to train on are used. Reason: the model's one-sentence reason after its name (`google/gemini-2.5-flash-lite: Promotes a crypto investment scheme.`), or `google/gemini-2.5-flash-lite says it matches the instruction` when it gives none.

### Combining conditions
- **US-E27.** As an owner, when I add **🧩 All of These Conditions Match (AND)**, I see *Conditions (all must match)*. It matches when every condition inside it does; the AI conditions inside it (US-E25, US-E26) are asked only when the others match.
- **US-E28.** As an owner, when I add **🔀 Any of These Conditions Match (OR)**, I see *Conditions (any may match)*. It matches when one condition inside it does.
- **US-E29.** As an owner, I put a condition under **✖️ This Condition Does Not Match (NOT)** with *Wrap in NOT* in the condition's ⋯ menu, and see it inside the NOT. A NOT picked from the type list starts empty, with nothing to add to it, and an empty one is refused when I save it (US-C2). It matches when that condition does not. Reason: `does not match: has more than 1000 characters`.
- **US-E30.** As an owner, I nest combinations up to 8 levels deep and 64 conditions per rule; past that, the rule is refused when I save it (US-C2) and the editor warns me before (US-G5).

## F. Actions
- **US-F1.** As an owner, when I add **🗑 Moderate the message** to a rule for *💬 When someone sends a message*, I see no field. When the rule matches, the bot marks the message as moderated in the group. It needs the moderator role or higher. A rule for *🚪 When someone joins* does not offer it.
- **US-F2.** As an owner, when I add **👁 Set the member as observer** to a rule, I see *Restore them to member after (minutes)*, 0 (up to 43200, 30 days). When the rule matches, the member — the message's author, or the person who joined — becomes an observer who can read but not write; with a duration, the bot makes them a member again when it is up, within a minute, and with 0 they stay an observer until I change their role. A message stays unless the rule also moderates it. It needs the admin role or higher. A restore that keeps failing for a day is given up.
- **US-F3.** As an owner, when I add **🚪 Kick the member** to a rule, I see *Also delete every message the member has ever sent in this group*, off. When the rule matches, the member — the message's author, or the person who joined — is removed from the group; with that field on, every message they have sent there goes too, the checked one included. Without it a message stays unless the rule also moderates it. It needs the owner role.
- **US-F4.** As an owner, when several matched rules, or one rule, call for actions that overlap, the bot does each thing once: an action another one already covers is dropped (a kick that deletes every message covers *Moderate the message*; a longer observer time covers a shorter one), and the rest run in the order observer, moderate, kick. A configuration I save is merged the same way.

## G. The web editor
The editor has three kinds of page: the group's overview, a rule's page and the *New rule* page. The bar at the top of a page holds what that page can do: the overview has *Undo*, *Redo* (US-G6), the ⋯ menu and *Apply changes* (US-G7), which apply to the whole configuration; a rule's page and *New rule* have only `‹ Back to rules`, and a rule's page *Undo* and *Redo* for what I changed on that page alone.

- **US-G1.** As an owner, when I open Crypto Chat's link (US-B4), the editor opens on the group's overview: the group's name, **Crypto Chat**, with `Moderation settings · Moderator Bot` under it. Below are the rules in two sections, *💬 When someone sends a message* and *🚪 When someone joins*, each under `Checked top to bottom.` and with its own *+ Add rule* (US-G14). Each rule is one line: its name — or, without one, what it does (`IF contains one of 2 words THEN 🗑 moderate`) —, under a name what it does in short (`👁 observer · 🗑 moderate`), its switch — its *Actions* (US-G13) — and its ⋯ menu (US-G12); a tap on the name opens its page (US-G3). Beside its name, a rule with *Actions* off reads `Notify only`, or `Off` when *Notifications* is off too; one with *Actions* on and *Notifications* off reads `Silent` (US-G19); one that is not finished, `Incomplete` (US-G14). An empty section reads `No rules yet. Nothing is moderated until you add one.` under *💬 When someone sends a message*, and `No rules yet. Nobody is checked when they join.` under *🚪 When someone joins*. The overview's ⋯ menu holds *Edit all with AI* (US-G9), *Import JSON* and *Export JSON* (US-G11).
  - A link without `group` shows `Group 7` as the title.
  - A link with no `bot_id` opens the demo (US-G18).
  - A link whose settings cannot be read shows `The settings in this link could not be read. Starting from scratch.`, and the editor opens empty.
- **US-G3.** As an owner, when I tap *Links from newcomers* on the overview, its page opens, with `‹ Back to rules` to go back to the overview, as the browser's Back does. From top to bottom:
  - *Name*, up to 60 characters. Left empty, it shows what the rule does as its placeholder, and the overview shows that in its place.
  - *Actions* (US-G13), and *Notifications* under it (US-G19).
  - *When*: `💬 Someone sends a message` or `🚪 Someone joins`. Changing it moves the rule to the end of the other section; a condition or action that does not work there stays where it is, marked `Doesn't work with this trigger`, until I change or remove it.
  - *If*: the condition. Each condition is a row titled by its type, with its fields under it; each field shows its default and limits (*👤 Author Hits Message Rate Limit* starts at 5 messages in 1 minute) and, under it, what it is for in its first sentence, always in view, with *More* for the rest (and *Less* to fold it back). *ⓘ* opens the type's full description; the row's ⋯ menu holds *Change type…* (US-G4), *Wrap in NOT* (US-E29), *Wrap in AND*, *Unwrap* and *Delete*. A folded condition shows its settings in one line (`spam, casino`). *+ Add condition* opens a list of the types the rule's trigger takes, with a search and the tabs *All* — every type, grouped by subject —, *Message text*, *Links*, *Shape and size*, *Attachments*, *Member*, *Group* and *Combine*, each showing its own subject. In a rule for *🚪 When someone joins*, *If* can stay empty: the rule then matches everyone who joins, and reads `Everyone who joins`.
  - *Then*: the actions, in the order the bot carries them out, under `The bot runs them in this order.`; each is a row titled by its type, with its fields under it and a ⋯ menu holding *Change type…* and *Delete*. *+ Add action* lists the actions the rule's trigger takes.
  - *✨ Edit with AI* (US-G17), and a ⋯ menu with *Duplicate* — which adds `Links from newcomers (copy)` right below the rule and opens its page — and *Delete*, which goes back to the overview.

  A rule's page has no way to move the rule: its place is set on the overview (US-G12). A list shows its entries one by one, each with × to remove it; I add one by typing it and pressing Enter, and one already there is refused with `That entry is already in the list.` An empty list shows its input alone. A list longer than 20 entries also gets a filter (`filter 150 entries`) that shows only the entries containing what I type. *bulk edit* turns any list into text, one entry per line (`One per line; applied when you click away — this is how a long list gets pasted in, or replaced wholesale. Up to 10000 entries, 100 characters each.`), and *chips* turns it back. A condition or action of a type this editor does not know shows its type as `FutureType (unknown)`, under ❓ and with no fields; its *ⓘ* reads `This type is not in this editor's schema. It is kept as it is — leave it alone unless you mean to replace it.`, and it is sent back as it was.
- **US-G4.** As an owner, when I change a condition's or an action's type with *Change type…* in its ⋯ menu, the list offers the types the rule's trigger takes, and every field the new type also has keeps its value, so *All* becomes *Any* with its whole subtree, and *🔗 Message Contains a Link to Any of These Websites* becomes *🔗 Message Contains a Link to a Website Outside This List* with its domains. A composite inside another can be unwrapped with *Unwrap*: its conditions move up into the parent.
- **US-G5.** As an owner, when a rule's tree passes 8 levels or 64 conditions, the rule's page shows `This rule has 70 conditions nested 9 levels deep. The bot accepts at most 64 conditions and 8 levels, so simplify it before applying — otherwise the rules will be rejected.` An action another one of the same rule already covers (*🗑 Moderate the message* beside *🚪 Kick the member* with *Also delete every message the member has ever sent in this group*) carries the badge *covered*, with `A stronger action on this rule already covers it, so the bot skips this one.` written under it.
- **US-G6.** As an owner, I undo and redo changes with *Undo* and *Redo* (or Ctrl/Cmd+Z, Ctrl/Cmd+Shift+Z, Ctrl+Y outside a text field), up to 20 steps; a button with nothing to do is hidden. On the overview they go through every change of the configuration; on a rule's page, only through what I changed on that page since I opened it, so a change to a rule that is not on the screen is never undone from there. Nothing asks me to confirm a change, since every change can be undone.
- **US-G7.** As an owner, *Apply changes* counts my changes (`Apply 3 changes`). When I press it, I see *Changes before you send them*, with the number of rules before and after (`2 → 3 rules`): every change since I opened the link, under its section, each rule as its name and its `IF … THEN …` line, marked *changed*, *added*, *removed*, *moved* (`was 2nd`), *renamed* (`was “Links”`), *actions turned off*, *actions turned on*, *notifications turned off* or *notifications turned on*, then `Unchanged: Banned words, Flood`. A rule whose *When* changed shows under its new section, marked *changed* (`was 💬 When someone sends a message`). A list that lost entries is shown in red. A reorder inside *All* or *Any* is not a change. With no change at all: `Nothing has changed since you opened this page. You can still copy the link and send it again.` While a rule is incomplete (US-G14), the dialog shows `“New rule” has no condition yet. Finish it or delete it before sending.` (`… has no action yet. …`) in the place of *Copy the link*.
  - Past 70% of what one SimpleX message carries (15000 bytes): `This link is 11,200 bytes, 75% of what a single chat message can carry. Once it passes 15,000, the bot will no longer be able to read it.`
  - Past all of it: `This link is 16,400 bytes — past the 15,000 a single chat message can carry. The bot will not be able to read it. Shorten a long list before sending.`
- **US-G8.** As an owner, when I confirm with *Copy the link*, the editor copies `[Settings for Crypto Chat](https://ed-asriyan.github.io/simplex-chat-group-moderator#bot_id=7&group=Crypto%20Chat&config=…)` (`[Settings for group 7](…)` when the link I opened had no `group`) and shows `Link copied. Paste it into your chat with the bot and send it — that is what applies the changes.` with *Close*; I send it to the bot (US-C1). From then on, the changes are counted from what I copied. When the browser does not let the editor copy, it shows `Could not reach the clipboard. Copy the whole address from the address bar and send that to the bot.`
- **US-G9.** As an owner, when I choose *Edit all with AI* in the overview's ⋯ menu, I see *Edit your settings with AI* with two steps and, in its footer, *Cancel* and *Load into the editor*.
  1. *Copy the question*: *Copy the prompt* copies a request describing every trigger, condition, action and setting the editor knows, with my configuration, and I see `Prompt copied. Paste it into your AI chat.`
  2. *Paste what it answers*: I paste the AI's whole reply and press *Load into the editor*, in the panel's footer, beside *Cancel*. There is no *Close*. It loads only if it is a valid configuration, as a change I can undo, and is sent only through *Apply changes*: `Loaded 3 rules — 1 changed, 1 added. Press Apply changes when you are ready; you will see exactly what changed before anything is sent.` A reply that is no configuration gets `No settings found in that text. Copy the AI's whole reply, or just its code block.`; one with mistakes gets `Not loaded — the AI's settings have problems:` and the list of them; nothing pasted gets `Paste the AI's reply first.`

  *Undo the load* goes back to what I had before pasting (`Reverted to the settings and rules you had before pasting.`); after a later edit it refuses with `You have edited the settings since. Use Undo in the header to step back.`
- **US-G10.** As an owner, a list longer than 100 entries goes into an AI prompt (US-G9, US-G16, US-G17) as a marker (`<<KEEP LIST 1: 347 ENTRIES>>`) unless I unfold `1 long list sent as a placeholder` (`2 long lists sent as a placeholder` with two) under *Copy the question* and tick it there, and comes back as it was; an answer that changes a marker's count is refused (`the AI wrote 340 where your list has 347 entries — it looks like it edited the list blind`). An API key never goes into the prompt: it is replaced by `<<KEEP SECRET 1>>`, and an answer that writes a key of its own is refused (`the AI wrote its own value into the secret field “api_key”. Secrets never go through the AI`).
- **US-G11.** As an owner, *Import JSON* in the overview's ⋯ menu opens *Import settings from JSON*: I paste a configuration or press *Choose JSON file*, then *Load settings*. It is checked as an AI answer is, and loads as a change I can undo: `Loaded the settings and 3 rules. Press Apply changes to send.` Something that is not JSON gets `That is not valid JSON.`; JSON without rules gets `The rules list is missing.`; nothing gets `Paste the JSON or choose a file first.` *Export JSON*, in the same menu, opens *Export settings as JSON*: *Copy JSON* copies the configuration (`Settings copied as JSON.`), *Download JSON* downloads `moderation-settings.json` (`Settings exported.`), with API keys emptied unless I untick *Clear secret values (API keys)*.
- **US-G12.** As an owner, on the overview, I move a rule within its section by dragging it by its handle ⠿, or with *Move up* and *Move down* in its ⋯ menu; the first rule has no *Move up*, the last no *Move down*. The same menu holds *Disable* (*Enable* when the rule is off, US-G13) and *Delete*, which removes the rule, as a change I can undo. A menu opens below its button, or above it when the screen ends first — on a phone, where its bar is, too —, and never goes past the screen. The order is the one the bot checks the rules in (US-D1, US-D6). A rule leaves its section only by its *When* (US-G3).
- **US-G13.** As an owner, on a rule's page, I see *Actions*, a choice of *On* and *Off*, with *Notifications* under it (US-G19), and under both one line that says what the rule does as they stand. With *Notifications* on, *On* reads `When the rule triggers, I notify you and execute actions`, *Off* `When the rule triggers, I notify you but do not execute actions` While *Actions* is off, *Then* reads `⚠ Not carried out: Actions is Off` beside its title, and its actions show dimmed; I can still change them. When I pick *Off* there, or turn the rule off with its switch on the overview, its line on the overview dims and reads `Notify only` (`Off` when *Notifications* is off too, US-G19). The rule keeps its place and its settings. Once I send the change (US-G7, *actions turned off*), the bot carries out none of its actions (US-D1, US-D4, US-D6) but keeps the rule, so the next link from `/groups` still has it, off. Picking *On*, the switch, or *Enable* in its ⋯ menu turns it back on; *Disable* in the menu turns it off as the switch does. A new rule — built myself, from a template or with AI (US-G14–US-G16) — is on.
- **US-G14.** As an owner, when I press *+ Add rule* under *💬 When someone sends a message*, the *New rule* page opens, with `‹ Back to rules`, `When someone sends a message`, and three ways to start: *Start from a template* (US-G15), *Create with AI* (US-G16) and *Build it myself*. Nothing is added until I pick one, and going back leaves no rule behind. *Build it myself* adds a rule with no name, condition or action at the end of the section and opens its page with the condition types listed to pick from. Until it has a condition and an action, its page shows `Pick a condition and at least one action — the bot takes no rule without them.`, the overview shows it as `New rule` marked `Incomplete`, and it cannot be sent (US-G7). Under *🚪 When someone joins*, the page reads `When someone joins`, and a rule needs only an action: `Pick at least one action — the bot takes no rule without one.`
- **US-G15.** As an owner, when I press *Start from a template* on the *New rule* page, a sheet *Start from a template* opens over the page, with the templates for the section's trigger, each with its name and what it does in one `IF … THEN …` line, and *Cancel*. Picking one closes the sheet, adds it at the end of the section, named after the template — with a number when a rule already has that name (`Banned words 2`) —, as a change I can undo, and opens its page. For *💬 When someone sends a message*:
  - *👻 Delete blank and invisible messages*: *📏 Message Is Empty or Blank* and *📏 Message Contains Invisible Characters* under *🔀 Any of These Conditions Match (OR)*, then *🗑 Moderate the message*.
  - *🔁 Delete messages with repeated text*: *💬 Message Contains Repeated Sequence* (a sequence of 5 characters repeated 4 times), then *🗑 Moderate the message*.
  - *🤬 Delete messages with bad words*: *💬 Message Contains Any of These Words* with a ready-made list of English profanity, sexual and hateful words, which I can edit, then *🗑 Moderate the message*; its page opens on the list.
  - *🔗 Delete links to unknown websites*: *🔗 Message Contains a Link Outside Top 100 Websites* with no website of my own added, then *🗑 Moderate the message*; its page opens on the list, where I can add the websites I also allow.
  - *🔗 Delete links from newcomers*: *👤 Author Joined Recently* (60 minutes) and *🔗 Message Contains a Link Outside Top 100 Websites* under *🧩 All of These Conditions Match (AND)*, then *🗑 Moderate the message*.
  - *📏 Delete walls of text*: *📏 Message Exceeds Max Lines* (10 lines, 40 characters per line), then *🗑 Moderate the message*.
  - *🌊 Kick members who flood*: *👤 Author Hits Message Rate Limit* (12 messages in 1 minute), then *🚪 Kick the member* with *Also delete every message the member has ever sent in this group* on.
  - *🚨 Kick newcomers who spam*: *👤 Author Joined Recently* (2880 minutes) and, under *🔀 Any of These Conditions Match (OR)*, *👤 Author Hits Moderation Rate Limit* (4 moderated messages in 60 minutes) and *👤 Author Hits Message Rate Limit* (6 messages in 1 minute), all under *🧩 All of These Conditions Match (AND)*, then *🚪 Kick the member* with *Also delete every message the member has ever sent in this group* on.
  - *🌪 Silence senders while the group floods*: *👥 Group Hits Message Rate Limit* (12 messages in 1 minute), then *👁 Set the member as observer* for 30 minutes. The observer is the author of each message sent while the group is over the limit, not everyone in the group.

  For *🚪 When someone joins*:
  - *👁 Quarantine newcomers*: no condition, then *👁 Set the member as observer* for 60 minutes.
- **US-G16.** As an owner, when I press *Create with AI* on the *New rule* page, I see *Create a rule with AI* with three steps and, in its footer, *Cancel* and *Add the rule*.
  1. *What should the rule do?*: I write it, e.g. `Kick anyone whose name advertises a casino`.
  2. *Copy the question*: *Copy the prompt* copies a request with what I wrote and the conditions and actions the section's trigger takes, asking for one rule with a name, and I see `Prompt copied. Paste it into your AI chat.` With nothing written, I get `Describe the rule first.`
  3. *Paste what it answers*: I paste the AI's whole reply and press *Add the rule*, beside *Cancel*. A valid rule is added at the end of the section, with the section's trigger, as a change I can undo, and its page opens with `Added “Casino names”. Press Apply changes when you are ready.` A reply with no rule gets `No rule found in that text. Copy the AI's whole reply, or just its code block.`; one with mistakes gets `Not added — the AI's rule has problems:` and the list of them; one with several rules gets `The answer has 3 rules — ask the AI for one.`; nothing pasted gets `Paste the AI's reply first.`
- **US-G17.** As an owner, when I press *✨ Edit with AI* on the page of *Links from newcomers*, I see *Edit “Links from newcomers” with AI* with two steps and, in its footer, *Cancel* and *Load into the rule*.
  1. *Copy the question*: *Copy the prompt* copies a request with this rule alone and the conditions and actions its trigger takes, and I see `Prompt copied. Paste it into your AI chat.` Nothing else of my configuration goes into it.
  2. *Paste what it answers*: I paste the AI's whole reply and press *Load into the rule*, beside *Cancel*. A valid rule replaces this one, and nothing else, as a change I can undo: `Updated “Links from newcomers”. Press Apply changes when you are ready.` A reply with several rules, or a whole configuration, gets `The answer has 3 rules — ask the AI for this one rule only.`; one with no rule gets `No rule found in that text. Copy the AI's whole reply, or just its code block.`; one with mistakes gets `Not loaded — the AI's rule has problems:` and the list of them; nothing pasted gets `Paste the AI's reply first.`

  *Undo the load* goes back to the rule I had before pasting (`Reverted to the rule you had before pasting.`); after a later edit it refuses with `You have edited the settings since. Use Undo in the header to step back.`
- **US-G18.** As a person who has not invited the bot, when I open the link of the welcome (US-A1), `https://ed-asriyan.github.io/simplex-chat-group-moderator`, the editor opens on the overview of a demo group, titled **Demo group**, with `Moderation settings · Moderator Bot` under it. Above every page I read `🧪 This is a demo. Everything works as it will for your own group: add rules, change them, try the AI. Nothing here reaches any group. To moderate yours, invite the bot to it as moderator or higher, and the link it sends you opens this editor on your group's rules.` The demo group has one rule for each template of US-G15 — all with *Actions* and *Notifications* on, named after their template, in the order US-G15 lists them, each under its section — so I see what a group's rules look like.

  Everything of US-G1–US-G17 works on it, *Apply changes* included: its dialog shows my changes as US-G7 says, but in the place of *Copy the link* it reads `This is a demo, so there is nothing to send. Invite the bot to your group, and the link it sends you opens this editor on your group's rules.`, and with no change at all, `Nothing has changed since you opened this page.` A link with settings but no `bot_id` opens the same demo on those settings.
- **US-G19.** As an owner, on a rule's page, under *Actions*, I see *Notifications*, a choice of *On* and *Off*. The one line under both switches says what the rule does by both choices, and changes as soon as I change either:
  - *Actions* on, *Notifications* on: `When the rule triggers, I notify you and execute actions`
  - *Actions* off, *Notifications* on: `When the rule triggers, I notify you but do not execute actions`
  - *Actions* on, *Notifications* off: `When the rule triggers, I execute actions but do not notify you`
  - *Actions* off, *Notifications* off: `When the rule triggers, I do nothing`

  When I pick *Off*, the rule's line on the overview reads `Silent` (`Off` when *Actions* is off too). Once I send the change (US-G7, *notifications turned off*), the bot no longer tells me about the rule (US-D2–US-D4, US-D6) and keeps the choice, so the next link from `/groups` still has it. A new rule — built myself, from a template or with AI (US-G14–US-G16) — has *Notifications* on.
