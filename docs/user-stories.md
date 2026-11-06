# User stories

The requirements baseline for the bot and its web editor. The design, the ports, the schema and the code are derived from this file; when they disagree, fix this file first and then the code.

This is a living document: it always describes the current intended behaviour, and stories are rewritten or removed as that changes. Story ids (`US-<epic><n>`) are what the code and its tests cite, so a change here that renumbers stories updates the references there in the same commit.

## The bot in one paragraph
A group owner adds the bot to their [SimpleX Chat](https://simplex.chat) group with the moderator role or higher. The bot then checks every message a member posts against the group's **rules**, which the owner sets up in a visual **web editor** reached through a link the bot sends. A rule is a **condition** (a message contains a listed word, a link outside a list, the author posts too fast, …, or any combination of them) and the **actions** to take when it matches (delete the message, make the author an observer, kick them). The group's **mode** decides whether the bot acts, tells the owner, or both.

## Roles
- **Owner** — the person who invited the bot to a group. They configure it, and only they get its notifications. The owner of the group in the bot's sense need not be the group's owner in SimpleX.
- **Member** — anyone in the group whose SimpleX role is *member*. Only a member's messages are checked; a moderator's, an admin's and an owner's never are, and an observer cannot write.

## Conventions
- The owner talks to the bot in a direct chat, in English. The bot writes in SimpleX Markdown; a bare `/command` in its text is tappable.
- The examples use the owner Alice, her group **Crypto Chat** (the bot's id for it: 7), and the member Bob. The editor's address is `https://ed-asriyan.github.io/simplex-chat-group-moderator`; a link to it carries the group's settings after `#`, shown here as `#bot_id=7&config=…`.

## A. Talking to the bot
- **US-A1.** As a person, when I connect to the bot, it sends me the welcome:

  ```text
  Hi! I'm an **automated moderation** bot for SimpleX groups.

  ### What I can detect

  - 🚫 **Banned words and exact phrases**, even obfuscated like b@d_w0rd.
  - 🔗 **Links** — block specific sites, allow only a list, or allow just the Top 100 sites.
  - ⏱ **Rate limits** — too many messages, or repeated violations, in a time window.
  - 🌊 **Screen flooding** — long messages, empty or invisible text, line floods.

  …and much more — this is just a glimpse.

  ### What I can do to violators

  - 🗑 **Delete their messages**
  - 👁 **Make them observers**
  - 🚪 **Kick them out of the group**

  ### How to get started

  1. Invite me to your group as:
     - **Moderator** — if you only want me to 🗑 delete messages.
     - **Admin** — if you also want me to 👁 make members observers.
     - **Owner** — if you also want me to 🚪 kick members.
  2. Once I join, send /groups to set up your rules in the visual editor.

  Send /help any time for the commands and modes (like dry mode).

  My code is [available on GitHub](https://github.com/ed-asriyan/simplex-chat-group-moderator).
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
  3. I watch the chat and act according to your rules.

  To stop me moderating a group, just remove me from it.

  ### Modes

  Each group has a mode, set in the same editor as its rules (send /groups to get the link):

  - **Notifications** — I act and DM you every time I do.
  - **Silent** — I act without telling you.
  - **Dry** — I run every check and DM you what I would have done, but delete and kick nothing.

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
- **US-B1.** As an owner, when I invite the bot to Crypto Chat with the moderator role or higher, it joins and I get two messages: `✅ I joined the group. Open the editor below to set up its rules.`, then the group's editor link (US-B4). I am the group's owner for the bot from then on. A new group has no rules and the mode *Notifications*.
- **US-B2.** As a person, when I invite the bot to a group with a role below moderator, it does not join, and I get `❌ I can't join this group: I need the **moderator** role or higher to delete messages. Invite me again with that role.`
- **US-B3.** As a person, when the bot cannot join a group I invited it to (the invitation expired, or the group is the bot's already), I get `❌ I couldn't join the group. Check that the invitation is still valid and send it again.`
- **US-B4.** As an owner, when I send `/groups`, I get one message per group I own:

  ```text
  *Crypto Chat*
  [View and Edit Settings](https://ed-asriyan.github.io/simplex-chat-group-moderator#bot_id=7&config=…)
  ```

  The link carries the group's whole current configuration, its mode and its rules, so the editor opens on exactly what the bot has. The group's name is its current one: the bot takes it from every message posted there.
- **US-B5.** As a person who owns no group, when I send `/groups`, I get `You don't have any groups yet. Invite me to your group as **moderator** or higher to get started — /help explains how.`
- **US-B6.** As an owner, when I remove the bot from Crypto Chat, it forgets the group, its mode and its rules: the group is no longer in `/groups`.

## C. Saving a group's configuration
- **US-C1.** As an owner, when I send the bot the link the editor copied (US-G8), alone or anywhere in a message, the group's whole configuration — its mode and all its rules — replaces the old one, and I get `✅ Group settings updated.` The bot moderates by the new configuration from the next message on.
- **US-C2.** As an owner, when the configuration I send is not valid, nothing of it is saved, the old one stays, and I get the reason quoted:

  ```text
  ❌ **Failed to update group settings:**

  > A rule has no actions. Add at least one action to it, or remove the rule.
  ```

  A configuration is not valid when, for instance, a rule has no action, a composite condition is empty, a condition tree is deeper than 8 levels or has more than 64 conditions, a regex pattern does not compile, a number is outside its range, or *Author Hits Moderation Rate Limit* sits under a *Not* (`'Author Hits Moderation Rate Limit' cannot be placed under a 'Not' condition, because it already depends on what the other rules do with this message.`). A link the bot cannot decompress gets `Could not decompress the settings from the URL. Open the editor link again, make your changes, and send back the updated URL.` as the reason.
- **US-C3.** As a person, when I send a link for a group I do not own, nothing is saved and I get the refusal of US-C2 with the reason `User <my id> is not the owner of group 7`.
- **US-C4.** As an owner, when the configuration I send has an AI condition (US-E25, US-E26), the bot first checks each API key with its provider, and saves nothing if one does not work, giving the provider's verdict as the reason, e.g. `OpenRouter rejected the API key …a1b2. Check that it was copied whole and has not been deleted or disabled at openrouter.ai/settings/keys.` A key is only ever shown by its last characters.
- **US-C5.** As an owner, when I send a link to the editor without its settings (no `bot_id` or no `config` after `#`), the bot does not read it as settings, and I get the answer of US-A7.

## D. Moderation
- **US-D1.** As an owner, when a member posts in Crypto Chat, the bot checks the message against the rules of the group, in order. A rule whose every action the rules matched before it already cover is not checked: with two rules that both moderate the message, only the first one that matches counts. Every rule that matches contributes its actions; the bot merges them (US-F4) and carries them out according to the group's mode. An edited message is checked again, as it is after the edit.
- **US-D2.** As an owner of a group in the mode *Notifications*, when a member's message matches, the bot carries out the actions and tells me:

  ```text
  🛡 I set the author as observer for 60 minutes and moderated the message in **Crypto Chat**.

  **The message:**
  > Buy cheap followers at https://spam.example

  **Reason:**
  - spam.example

  If it's a false positive, please [file an issue](https://github.com/ed-asriyan/simplex-chat-group-moderator/issues/new?template=moderation-rule-bug.yml).
  ```

  The actions are named in the order they were carried out — the observer first, then the message, then the kick (`set the author as observer`, `… for 1 minute`, `… for 60 minutes`, `moderated the message`, `kicked the author`, `kicked the author and deleted all their messages`) — the last joined by `and`. Each matched rule gives one reason line, in the order of the rules; a rule that is not checked because the rules before it already cover its actions (US-D1) gives none, except as US-E20 says. The message is quoted with its line breaks, cut after 300 characters with `...`, and a message without text shows as `_(no text)_`.
- **US-D3.** As an owner of a group in the mode *Silent*, when a member's message matches, the bot carries out the actions and does not tell me.
- **US-D4.** As an owner of a group in the mode *Dry*, when a member's message matches, the bot does nothing in the group and tells me what it would have done, in the form of US-D2 with `🛡 I would moderate the message …` (`would … kick the author`, `would … kick the author and delete all their messages`; `set the author as observer` reads the same).
- **US-D5.** As an owner, a moderator's, an admin's or an owner's message in my group is never checked, whatever the rules say.

## E. Conditions
A rule's condition is one of the conditions below, or a combination of them (US-E27–US-E30). Each story starts with what I see when I add the condition in the editor: its title, and its fields as the editor labels them, with their defaults and limits. A matched condition gives the reason line of my notification (US-D2), shown here after it.

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
- **US-E20.** As an owner, when I add **👤 Author Hits Moderation Rate Limit**, I see *Number of Moderated Messages*, 3 (at least 1), and *Time Window (Minutes)*, 60. It matches when that many of the author's messages were moderated in the window, the one being checked included when another rule moderates it, and then matches the author's next messages too until the window has room. It cannot sit under a *Not* (US-C2). Reason: `author had 3 messages moderated in 60 min`. While a rule has it, the notification always names the first rule that matched the message, after the other reasons, even when that rule was not checked because the rules before it already cover its actions (US-D1): with this limit first and *Contains Any of These Words* `spam` second, both moderating, the third `spam` lists `author had 3 messages moderated in 60 min`, then `contains word: 'spam'`.
- **US-E21.** As an owner, when I add **👤 Author Joined Recently**, I see *Joined Less Than (Minutes) Ago*, 60 (at least 1). A message from a member who joined less than that long ago matches; a member who was in the group before the bot joined never does. Reason: `author joined 12 min ago (less than 60 min)`.

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
- **US-E29.** As an owner, I put a condition under **✖️ This Condition Does Not Match (NOT)** with *Wrap in NOT* in the condition's ⋮ menu, and see it inside the NOT. A NOT picked from the type list starts empty, with nothing to add to it, and an empty one is refused when I save it (US-C2). It matches when that condition does not. Reason: `does not match: has more than 1000 characters`.
- **US-E30.** As an owner, I nest combinations up to 8 levels deep and 64 conditions per rule; past that, the rule is refused when I save it (US-C2) and the editor warns me before (US-G5).

## F. Actions
- **US-F1.** As an owner, when I add **🗑 Moderate the message** to a rule, I see no field. When the rule matches, the bot marks the message as moderated in the group. It needs the moderator role or higher.
- **US-F2.** As an owner, when I add **👁 Set author as observer** to a rule, I see *Restore author to member after (minutes)*, 0 (up to 43200, 30 days). When the rule matches, the author becomes an observer who can read but not write; with a duration, the bot makes them a member again when it is up, within a minute, and with 0 they stay an observer until I change their role. The message itself stays unless the rule also moderates it. It needs the admin role or higher. A restore that keeps failing for a day is given up.
- **US-F3.** As an owner, when I add **🚪 Kick the author** to a rule, I see *Also delete every message the author has ever sent in this group*, off. When the rule matches, the author is removed from the group; with that field on, their whole history goes too, the message included. Without it the message stays unless the rule also moderates it. It needs the owner role.
- **US-F4.** As an owner, when several matched rules, or one rule, call for actions that overlap, the bot does each thing once: an action another one already covers is dropped (a kick that deletes every message covers *Moderate the message*; a longer observer time covers a shorter one), and the rest run in the order observer, moderate, kick. A configuration I save is merged the same way.

## G. The web editor
- **US-G1.** As an owner, when I open Crypto Chat's link (US-B4), the editor shows *Moderation settings for Moderator Bot*, then its three steps: "1️⃣ Choose the group's mode. 2️⃣ Pick a rule in the list, or add a new one, and for each rule set the 🔎 Condition — what to detect — and the ⚡ Actions — what the bot does when a message matches. 3️⃣ When you're done, press "Apply changes" and send the copied link back to the bot." Below them are the *Group settings* panel with the group's mode, and *Rules* with their number (`3 total`), each rule as its condition tree and its actions in plain words; one tap opens a rule. With no rules, the list reads `No rules yet. Nothing is moderated until you add one.` A rule of a type this editor does not know shows its type as `FutureType (unknown)`, under ❓ and with no fields; its *ⓘ* reads `This type is not in this editor's schema. It is kept as it is — leave it alone unless you mean to replace it.`, and it is sent back as it was.
  - A link with no `bot_id` shows `No bot ID found in the URL. Open the link the bot sent you in a direct message — a link opened by hand cannot be applied.`, and has no *Apply changes*.
  - A link whose settings cannot be read shows `The settings in this link could not be read. Starting from scratch.`, and the editor opens empty.
- **US-G2.** As an owner, I change the mode under *🛡 Mode* in *Group settings*: *Notifications — act and tell me*, *Silent — act without telling me*, *Dry — only tell me what it would do*.
- **US-G3.** As an owner, I press *+ Add rule* and pick a condition from a list grouped by subject (💬 text, 🔗 links, 📏 shape and size, 📎 attachments, 👤 author, 👥 group), then its actions. Each field shows its default and limits (*👤 Author Hits Message Rate Limit* starts at 5 messages in 1 minute), and *ⓘ* opens the condition's full description. A list shows its entries one by one, each with × to remove it; I add one by typing it and pressing Enter, and one already there is refused with `That entry is already in the list.` A list longer than 20 entries also gets a filter (`filter 150 entries`) that shows only the entries containing what I type. *bulk edit* turns any list into text, one entry per line (`One per line; applied when you click away — this is how a long list gets pasted in, or replaced wholesale. Up to 10000 entries, 100 characters each.`), and *chips* turns it back.
- **US-G4.** As an owner, when I change a condition's type or an action's type, every field the new type also has keeps its value, so *All* becomes *Any* with its whole subtree, and *🔗 Message Contains a Link to Any of These Websites* becomes *🔗 Message Contains a Link to a Website Outside This List* with its domains. A composite inside another can be unwrapped: its conditions move up into the parent.
- **US-G5.** As an owner, when a rule's tree passes 8 levels or 64 conditions, the rule shows `This rule has 70 conditions nested 9 levels deep. The bot accepts at most 64 conditions and 8 levels, so simplify it before applying — otherwise the rules will be rejected.` An action another one of the same rule already covers (*🗑 Moderate the message* beside *🚪 Kick the author* with *Also delete every message the author has ever sent in this group*) carries the badge *covered*: "A stronger action on this rule already covers it".
- **US-G6.** As an owner, I undo and redo any change with *Undo* and *Redo* (or Ctrl/Cmd+Z, Ctrl/Cmd+Shift+Z, Ctrl+Y outside a text field), up to 20 steps; a button with nothing to do is hidden. Nothing asks me to confirm a change, since every change can be undone.
- **US-G7.** As an owner, when I press *Apply changes*, I see *Changes before you send them*, with the number of rules before and after (`2 → 3 rules`): every change since I opened the link, each rule as one `IF … THEN …` line marked *changed*, *added*, *removed* or *moved* (`was rule 2`), then `Unchanged rules: 1, 3`. A changed setting is a line of its own above the rules: `🛡 Mode: Dry — only tell me what it would do → Notifications — act and tell me`. A list that lost entries is shown in red. A reorder inside *All* or *Any* is not a change. With no change at all: `Nothing has changed since you opened this page. You can still copy the link and send it again.`
  - Past 70% of what one SimpleX message carries (15000 bytes): `This link is 11,200 bytes, 75% of what a single chat message can carry. Once it passes 15,000, the bot will no longer be able to read it.`
  - Past all of it: `This link is 16,400 bytes — past the 15,000 a single chat message can carry. The bot will not be able to read it. Shorten a long list before sending.`
- **US-G8.** As an owner, when I confirm with *Copy the link*, the editor copies `[Settings for group 7](https://ed-asriyan.github.io/simplex-chat-group-moderator#bot_id=7&config=…)` and shows `Link copied. Paste it into your chat with the bot and send it — that is what applies the changes.` with *Close*; I send it to the bot (US-C1). From then on, the changes are counted from what I copied. When the browser does not let the editor copy, it shows `Could not reach the clipboard. Copy the whole address from the address bar and send that to the bot.`
- **US-G9.** As an owner, when I press *Edit with AI*, I see *Edit your settings with AI* with two steps.
  1. *Copy the question*: *Copy the prompt* copies a request describing every condition, action and setting the editor knows, with my configuration, and I see `Prompt copied. Paste it into your AI chat.`
  2. *Paste what it answers*: I paste the AI's whole reply and press *Load into the editor*. It loads only if it is a valid configuration, as a change I can undo, and is sent only through *Apply changes*: `Loaded 3 rules — 1 changed, 1 added. Press Apply changes when you are ready; you will see exactly what changed before anything is sent.` A reply that is no configuration gets `No settings found in that text. Copy the AI's whole reply, or just its code block.`; one with mistakes gets `Not loaded — the AI's settings have problems:` and the list of them; nothing pasted gets `Paste the AI's reply first.`

  *Undo the load* goes back to what I had before pasting (`Reverted to the settings and rules you had before pasting.`); after a later edit it refuses with `You have edited the settings since. Use Undo in the header to step back.`
- **US-G10.** As an owner, a list longer than 100 entries goes into the AI prompt as a marker (`<<KEEP LIST 1: 347 ENTRIES>>`) unless I unfold `1 long list sent as a placeholder` (`2 long lists sent as a placeholder` with two) under *Copy the question* and tick it there, and comes back as it was; an answer that changes a marker's count is refused (`the AI wrote 340 where your list has 347 entries — it looks like it edited the list blind`). An API key never goes into the prompt: it is replaced by `<<KEEP SECRET 1>>`, and an answer that writes a key of its own is refused (`the AI wrote its own value into the secret field “api_key”. Secrets never go through the AI`).
- **US-G11.** As an owner, *Import JSON* opens *Import settings from JSON*: I paste a configuration or press *Choose JSON file*, then *Load settings*. It is checked as an AI answer is, and loads as a change I can undo: `Loaded the settings and 3 rules. Press Apply changes to send.` Something that is not JSON gets `That is not valid JSON.`; JSON without rules gets `The rules list is missing.`; nothing gets `Paste the JSON or choose a file first.` *Export JSON* opens *Export settings as JSON*: *Copy JSON* copies the configuration (`Settings copied as JSON.`), *Download JSON* downloads `moderation-settings.json` (`Settings exported.`), with API keys emptied unless I untick *Clear secret values (API keys)*.
