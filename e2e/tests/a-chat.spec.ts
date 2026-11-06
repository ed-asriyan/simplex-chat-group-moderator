// A. Talking to the bot.
import { expect, test } from '@playwright/test';
import { Person } from '../harness/person';

const REPO = 'https://github.com/ed-asriyan/simplex-chat-group-moderator';

const WELCOME = `Hi! I'm an **automated moderation** bot for SimpleX groups.

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

My code is [available on GitHub](${REPO}).`;

const HELP = `### How to use this bot

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
- /feature — request a new moderation rule type or feature.`;

// US-A1
test('connecting brings the welcome', async () => {
  const alice = await Person.join('Alice');
  expect((await alice.message()).trimEnd()).toBe(WELCOME);
});

// US-A2
test('start brings the welcome', async () => {
  const alice = await Person.join('Alice');
  await alice.message();
  expect((await alice.ask('/start')).trimEnd()).toBe(WELCOME);
});

// US-A3
test('help gives the guide', async () => {
  const alice = await Person.join('Alice');
  await alice.message();
  expect((await alice.ask('/help')).trimEnd()).toBe(HELP);
});

// US-A4, US-A5, US-A6
test('source, issue and feature give their links', async () => {
  const alice = await Person.join('Alice');
  await alice.message();
  expect(await alice.ask('/source')).toBe(`My source code is [on GitHub](${REPO}).`);
  expect(await alice.ask('/issue')).toBe(`🐞 Please report bugs and unexpected moderation [on GitHub](${REPO}/issues/new?template=moderation-rule-bug.yml).`);
  expect(await alice.ask('/feature')).toBe(`💡 Please request new moderation rule types and features [on GitHub](${REPO}/issues/new?template=feature-request.yml).`);
});

// US-A7
test('anything else is an unknown command', async () => {
  const alice = await Person.join('Alice');
  await alice.message();
  for (const text of ['/foo', 'hello', ' ']) {
    expect(await alice.ask(text), text).toBe('Unknown command. Send /help to see what I can do.');
  }
});
