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

  A configuration is not valid when, for instance, a rule has no action, a composite condition is empty, a condition tree is deeper than 8 levels or has more than 64 conditions, a regex pattern does not compile, a number is outside its range, or *Author Hits Moderation Rate Limit* sits under a *Not* (`'AuthorHitsModerationRateLimit' cannot be placed under a 'Not' condition, because it already depends on what the other rules do with this message.`). A link the bot cannot decompress gets `Could not decompress the settings from the URL. Open the editor link again, make your changes, and send back the updated URL.` as the reason.
- **US-C3.** As a person, when I send a link for a group I do not own, nothing is saved and I get the refusal of US-C2 with the reason `User <my id> is not the owner of group 7`.
- **US-C4.** As an owner, when the configuration I send has an AI condition (US-E8, US-E9), the bot first checks each API key with its provider, and saves nothing if one does not work, giving the provider's verdict as the reason, e.g. `OpenRouter rejected the API key …a1b2. Check that it was copied whole and has not been deleted or disabled at openrouter.ai/settings/keys.` A key is only ever shown by its last characters.
- **US-C5.** As an owner, when I send a link to the editor without its settings (no `bot_id` or no `config` after `#`), the bot does not read it as settings, and I get the answer of US-A7.

## D. Moderation
- **US-D1.** As an owner, when a member posts in Crypto Chat, the bot checks the message against every rule of the group, in order. Every rule that matches contributes its actions; the bot merges them (US-F4) and carries them out according to the group's mode. An edited message is checked again, as it is after the edit.
- **US-D2.** As an owner of a group in the mode *Notifications*, when a member's message matches, the bot carries out the actions and tells me:

  ```text
  🛡 I moderated the message and set the author as observer for 60 minutes in **Crypto Chat**.

  **The message:**
  > Buy cheap followers at spam.example

  **Reason:**
  - spam.example

  If it's a false positive, please [file an issue](https://github.com/ed-asriyan/simplex-chat-group-moderator/issues/new?template=moderation-rule-bug.yml).
  ```

  The actions are named in the order they were carried out (`moderated the message`, `set the author as observer`, `… for 1 minute`, `… for 60 minutes`, `kicked the author`, `kicked the author and deleted all their messages`), the last joined by `and`. Each matched rule gives one reason line. The message is quoted with its line breaks, cut after 300 characters with `...`, and a message without text shows as `_(no text)_`.
- **US-D3.** As an owner of a group in the mode *Silent*, when a member's message matches, the bot carries out the actions and does not tell me.
- **US-D4.** As an owner of a group in the mode *Dry*, when a member's message matches, the bot does nothing in the group and tells me what it would have done, in the form of US-D2 with `🛡 I would moderate the message …` (`would … kick the author`, `would … kick the author and delete all their messages`; `set the author as observer` reads the same).
- **US-D5.** As an owner, a moderator's, an admin's or an owner's message in my group is never checked, whatever the rules say.

## E. Conditions
A rule's condition is one of the conditions below, or a combination of them (US-E10). Each matched condition gives the reason line of the owner's notification, shown here after the condition.

- **US-E1.** As an owner, when my rule has **💬 Message Contains Any of These Words** with the words `spam`, `blow job`, a member's message matches when it contains one of them as a whole word or phrase — never as a substring (`ass` does not match `classic`) — however it is disguised: capitals, leet-speak (`b@dw0rd`), Latin/Cyrillic look-alikes, styled or full-width letters, upside-down text, spaced-out letters (`s.p.a.m`), flooded letters (`spaaaam`), `@mentions`, English plurals and endings (`spams`), and, for a word of five letters or more, written split or joined (`blowjob`). Reason: `contains word: 'spam'`.
- **US-E2.** As an owner, when my rule has **💬 Message Exactly Matches Any of These Texts**, a message matches when its whole text, without leading and trailing spaces, is one of the texts, in any letter case unless *Case sensitive* is on.
- **US-E3.** As an owner, when my rule has **💬 Message Matches Any of These Regex Patterns** (`\d{4,}`), a message matches when its raw text matches one of the patterns, in Rust `regex` syntax; nothing is undone for it as for words.
- **US-E4.** As an owner, when my rule has **💬 Message Contains Repeated Sequence** (at least 5 repeats, sequences of 1 or more characters, up to 50), a message matches when some sequence repeats back-to-back that many times (`aaaaa`, `!!!!!`, `hahahahaha`), letter case aside. Reason: `"ha" repeated 5 times in a row`.
- **US-E5.** As an owner, links:
  - **🔗 Message Contains a Link to Any of These Websites** (`spam.example`) matches a link to a listed domain or one of its subdomains;
  - **🔗 Message Contains a Link to a Website Outside This List** (`simplex.chat`) matches a link to any other website, and with an empty list, any link;
  - **🔗 Message Contains a Link Outside Top 100 Websites** matches a link to a website neither among the top 100 (Google, GitHub, Wikipedia, YouTube, Reddit, …) nor in my extra list.

  Each sees through disguised links (`spam [dot] com`, `hxxp://`); a link with no `http://` or `www.` counts only when it ends in a common domain ending (`.com`, `.ru`, `.io`, `.me`, `.chat`, `.xyz`, …). The reason is the link's domain (`spam.example`).
- **US-E6.** As an owner, the message's shape:
  - **📏 Message Is Empty or Blank** matches a message of only whitespace, line breaks or invisible characters, and never one with an attachment (reason: `empty message`);
  - **📏 Message Contains Invisible Characters** matches a message with even one zero-width, Braille-blank, bidi or variation character, emoji joined into one aside;
  - **📏 Message Exceeds Max Characters** (1000), **Max Words** (150) and **Max Lines** (10, a line longer than 40 characters counting as several, as it wraps on a phone; 0 for line breaks only) match a message over the limit (reason: `1203 characters`, `12 lines`).
- **US-E7.** As an owner, what a message carries: **📎 Message Contains an Image**, **a Video**, **a Voice Message**, **a File** (any other attachment) match a message carrying one. A caption is the message's text for every other condition.
- **US-E8.** As an owner, when my rule has **💬 Flagged by OpenAI Omni** with my own OpenAI key, each message with text it checks goes to OpenAI's Moderation API, and it matches when a category is past its trigger: per category *Off*, *OpenAI decides*, or a minimum score in percent. A call that fails is tried again up to *Attempts* times (3), *Pause Between Attempts* (1 s) apart; when OpenAI cannot be reached, it does not match. Reason: `flagged by OpenAI Omni: harassment (OpenAI), hate 87% ≥ 80%`.
- **US-E9.** As an owner, when my rule has **💬 Flagged by OpenRouter Instruction** with my own OpenRouter key, a model from the list and an instruction ("Match advertising: crypto promotions, casinos, unsolicited offers. Do not match normal conversation."), each message with text it checks goes to the model with the instruction and, if I ask for them, up to 10 earlier messages of the group as context, each with its author's display name, the checked message with its author's too (messages the bot deleted are left out, and the bot keeps them in memory only, for 24 hours at most, so there are fewer or none right after saving or a restart); it matches when the model answers yes, and the model's one-sentence reason is the reason line. Only providers that do not keep the text to train on are used. A call that fails is retried as in US-E8; when OpenRouter cannot be reached or gives no clear answer, it does not match.
- **US-E10.** As an owner, I combine conditions: **🧩 All of These Conditions Match (AND)** matches when every condition inside it does, **🔀 Any of These Conditions Match (OR)** when one does, **✖️ This Condition Does Not Match (NOT)** when the one inside it does not. They nest up to 8 levels deep and 64 conditions per rule. Inside *All*, the AI conditions are asked only when the others match.
- **US-E11.** As an owner, the author's and the group's pace, each over a time window of 1 to 60 minutes, counting the message being checked:
  - **👤 Author Hits Message Rate Limit** (5 messages in 1 min), **Character Rate Limit** (2000 characters), **Line Rate Limit** (30 lines, wrapped as in US-E6) match when the author's messages in the window reach the number (reason: `author sent at least 5 messages in 1 min`);
  - **👥 Group Hits Message Rate Limit** (30), **Character Rate Limit** (10000), **Line Rate Limit** (150) do the same over every member's messages together, so they catch a raid by accounts that each stay under the author's limits; an edit does not count (reason: `group received at least 30 messages in 1 min`).
- **US-E12.** As an owner, when my rule has **👤 Author Hits Moderation Rate Limit** (3 moderated messages in 60 min), it matches when that many of the author's messages were moderated in the window, the one being checked included when another rule moderates it, and then matches the author's next messages too until the window has room. It cannot sit under a *Not* (US-C2).
- **US-E13.** As an owner, when my rule has **👤 Author Joined Recently** (60 min), it matches a message from a member who joined less than that long ago. A member who was in the group before the bot joined never matches.

## F. Actions
- **US-F1.** As an owner, when a rule with **🗑 Moderate the message** matches, the bot marks the message as moderated in the group. It needs the moderator role or higher.
- **US-F2.** As an owner, when a rule with **👁 Set author as observer** matches, the author becomes an observer who can read but not write; with a duration (up to 43200 minutes, 30 days), the bot makes them a member again when it is up, within a minute, and with 0 they stay an observer until I change their role. The message itself stays unless the rule also moderates it. It needs the admin role or higher. A restore that keeps failing for a day is given up.
- **US-F3.** As an owner, when a rule with **🚪 Kick the author** matches, the author is removed from the group; with *Also delete every message the author has ever sent in this group*, their whole history goes too, the message included. Without it the message stays unless the rule also moderates it. It needs the owner role.
- **US-F4.** As an owner, when several matched rules, or one rule, call for actions that overlap, the bot does each thing once: an action another one already covers is dropped (a kick that deletes every message covers *Moderate the message*; a longer observer time covers a shorter one), and the rest run in the order observer, moderate, kick. A configuration I save is merged the same way.

## G. The web editor
- **US-G1.** As an owner, when I open a group's link (US-B4), the editor shows *Moderation settings for* the group: the *Group settings* panel with its mode, then the *Rules*, each as its condition tree and its actions in plain words. A rule of a type this editor does not know is shown read-only and kept as it is.
- **US-G2.** As an owner, I change the mode in *Group settings*: *Notifications — act and tell me*, *Silent — act without telling me*, *Dry — only tell me what it would do*.
- **US-G3.** As an owner, I press *+ Add rule* and pick a condition from a list grouped by subject (💬 text, 🔗 links, 📏 shape and size, 📎 attachments, 👤 author, 👥 group), then its actions. Each field shows its default and limits, and *ⓘ* opens the condition's full description. A list longer than 20 entries is edited as text, one entry per line.
- **US-G4.** As an owner, when I change a condition's type or an action's type, every field the new type also has keeps its value, so *All* becomes *Any* with its whole subtree, and one link condition becomes another with its domains. A composite inside another can be unwrapped: its conditions move up into the parent.
- **US-G5.** As an owner, the editor warns me when a tree passes 8 levels or 64 conditions, and when an action is covered by another of the same rule (a *covered* badge).
- **US-G6.** As an owner, I undo and redo any change with *Undo* and *Redo* (or Ctrl/Cmd+Z, Ctrl/Cmd+Shift+Z, Ctrl+Y outside a text field), up to 20 steps; a button with nothing to do is hidden. Nothing asks me to confirm a change, since every change can be undone.
- **US-G7.** As an owner, when I press *Apply changes*, I see *Changes before you send them*: every change since I opened the link, the mode on a line of its own (`Mode: Dry → Notifications`), rules added, removed, moved or edited, and a list that lost entries in red. A reorder inside *All* or *Any* is not a change. The editor warns me when the link would be longer than SimpleX carries in one message (15000 bytes).
- **US-G8.** As an owner, when I confirm with *Copy the link*, the link with the new configuration is copied, and I am told to paste it into the chat with the bot and send it (US-C1). From then on, the changes are counted from what I copied.
- **US-G9.** As an owner, when I press *Edit with AI*, I get one step: *Copy the prompt*, which copies a request describing every condition, action and setting the editor knows, with my configuration; I paste it into any AI chat, paste its answer under *Paste what it answers*, and *Load into the editor*. The answer loads only if it is a valid configuration, as a change I can undo (*Undo the load*), and is sent only through *Apply changes*.
- **US-G10.** As an owner, a list longer than 100 entries goes into the AI prompt as a marker (`<<KEEP LIST 1: 347 ENTRIES>>`) unless I tick it under *Long lists*, and comes back as it was; an answer that changes a marker's count is refused. An API key never goes into the prompt: it is replaced by `<<KEEP SECRET 1>>`, and an answer that writes a key of its own is refused.
- **US-G11.** As an owner, *Import JSON* takes a configuration pasted or loaded from a file, checks it as an AI answer is checked, and loads it as a change I can undo; *Export JSON* copies the configuration or downloads `moderation-settings.json`, with API keys emptied unless I untick *Clear secret values (API keys)*.
