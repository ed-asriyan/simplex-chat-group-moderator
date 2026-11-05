// B. Groups.
import { expect, test } from '@playwright/test';
import { Person, botId, configOf } from '../harness/person';
import { EDITOR } from '../harness/stack';

const NO_GROUPS = "You don't have any groups yet. Invite me to your group as **moderator** or higher to get started — /help explains how.";

// US-B1
for (const role of ['moderator', 'admin', 'owner']) {
  test(`the bot joins a group it is invited to as ${role}`, async () => {
    const alice = await Person.join('Alice');
    await alice.message();
    const group = await alice.createGroup('Crypto Chat');
    await alice.inviteBot(group, role);
    expect(await alice.message()).toBe('✅ I joined the group. Open the editor below to set up its rules.');
    const link = await alice.message();
    expect(link).toMatch(new RegExp(`^\\*Crypto Chat\\*\\n\\[View and Edit Settings\\]\\(${EDITOR.replace(/[.]/g, '\\.')}#bot_id=\\d+&config=[^)]+\\)$`));
    // A new group has no rules and the mode Notifications.
    expect(configOf(link)).toEqual({ mode: 'notifications', rules: [] });
  });
}

// US-B2
test('the bot does not join with a role below moderator', async () => {
  const alice = await Person.join('Alice');
  await alice.message();
  for (const role of ['member', 'observer']) {
    const group = await alice.createGroup(`Low ${role}`);
    await alice.inviteBot(group, role);
    expect(await alice.message(), role).toBe("❌ I can't join this group: I need the **moderator** role or higher to delete messages. Invite me again with that role.");
  }
  expect(await alice.ask('/groups')).toBe(NO_GROUPS);
});

// US-B3: an invitation the bot cannot accept (expired, or for a group it is
// in already) cannot be made from a SimpleX client, which refuses both; it is
// left to the unit tests.

// US-B4
test('groups lists each group with its link, by its current name', async () => {
  const alice = await Person.join('Alice');
  await alice.message();
  const first = await alice.moderatedGroup('Crypto Chat');
  const second = await alice.moderatedGroup('Dogs');
  await alice.send('/groups');
  const listed = [await alice.message(), await alice.message()].sort();
  expect(listed).toEqual([
    `*Crypto Chat*\n[View and Edit Settings](${first.link})`,
    `*Dogs*\n[View and Edit Settings](${second.link})`,
  ].sort());
  expect(botId(first.link)).not.toBe(botId(second.link));
});

// US-B5
test('groups with no group says how to start', async () => {
  const alice = await Person.join('Alice');
  await alice.message();
  expect(await alice.ask('/groups')).toBe(NO_GROUPS);
});

// US-B6
test('removing the bot forgets the group', async () => {
  const alice = await Person.join('Alice');
  await alice.message();
  const { group } = await alice.moderatedGroup('Crypto Chat');
  await alice.removeBot(group);
  await expect.poll(async () => alice.ask('/groups'), { timeout: 30_000 }).toBe(NO_GROUPS);
});
