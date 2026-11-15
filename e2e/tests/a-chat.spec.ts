// A. Talking to the bot.
import { expect, test } from '@playwright/test';
import { Person } from '../harness/person';
import { EDITOR } from '../harness/stack';

const REPO = 'https://github.com/ed-asriyan/simplex-chat-group-moderator';

const WELCOME = `Hi! 👋 I'm a **moderation bot** for SimpleX groups. You set the rules, and I catch:

- 🚫 **Banned words**, even disguised like b@d_w0rd
- 🔗 **Links** to websites you don't allow
- 🌊 **Floods**: too many messages, walls of text
- 🚪 **Spammers joining**, by names that advertise
- 🤖 **Anything you can describe**, checked by AI with your own key

Then I 🗑 delete the message, 👁 make the member an observer, or 🚪 kick them.

### 🚀 Get started
Invite me to your group as **Moderator** to delete messages, **Admin** to also make members observers, or **Owner** to also kick them. Then send /groups to set up the rules.

### 👀 Not ready to invite me?
[Try the editor on a demo group](${EDITOR}): see exactly what you would set up, without inviting me or giving me any role.

Send /help for the commands. My code is open [on GitHub](${REPO}).`;

const HELP = `### How to use this bot

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
