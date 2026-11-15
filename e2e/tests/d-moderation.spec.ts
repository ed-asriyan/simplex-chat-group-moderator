// D. Moderation.
import { expect, test } from '@playwright/test';
import { Person } from '../harness/person';
import { advance } from '../harness/stack';
import { apply, joinNotice, joinRule, mixedNotice, moderated, moderatedAnyway, named, notice, rule, rules, scene, turnedOff, untouched } from './helpers';

// US-D1
test('a rule the earlier matches already cover is not checked, and an edit is checked again', async () => {
  // Both rules moderate: only the first one that matches counts.
  const s = await scene(rules(
    rule({ type: 'ContainsWords', keywords: ['spam'] }),
    rule({ type: 'ContainsLinksInList', domains: ['casino.example'] }),
  ));
  await moderated(s, 'spam at https://casino.example', ["contains word: 'spam'"]);
  await moderated(s, 'see https://casino.example', ['casino.example']);
  const item = await s.member.post(s.memberGroup, 'hello');
  await s.member.seesNothing(3000);
  await s.member.edit(s.memberGroup, item, 'hello spam');
  expect(await s.member.seesInGroup()).toEqual({ kind: 'deleted', itemIds: [item] });
  expect(await s.owner.message()).toBe(notice('moderated the message', 'hello spam', ["contains word: 'spam'"]));
});

// US-D1
test('a rule with both switches off is not checked, and its actions cover nothing', async () => {
  const s = await scene(rules(
    named('Banned words', 'message_posted', { type: 'ContainsWords', keywords: ['spam'] }, [{ type: 'ModerateMessage' }], false, false),
    named('Casino links', 'message_posted', { type: 'ContainsWords', keywords: ['spam', 'casino'] }, [{ type: 'ModerateMessage' }]),
  ));
  // The first rule matches and is off: only the second one counts, and tells why.
  await moderated(s, 'spam', ["**Casino links**: contains word: 'spam'"]);
  // A rule that is off and moderates would have covered the second: it does not.
  await moderated(s, 'casino', ["**Casino links**: contains word: 'casino'"]);
});

// US-D1
test('a rule with Actions off and Notifications on is checked, and covers nothing for the rules that are on', async () => {
  const s = await scene(rules(
    named('Trial', 'message_posted', { type: 'ContainsWords', keywords: ['spam'] }, [{ type: 'ModerateMessage' }], false),
    named('Banned words', 'message_posted', { type: 'ContainsWords', keywords: ['spam'] }, [{ type: 'ModerateMessage' }]),
  ));
  // The rule that is on still moderates: the one before it, which only tells, does not cover it.
  await moderatedAnyway(s, 'spam');
});

// US-D1
test('a rule whose every action is covered is not checked, whatever its switches', async () => {
  const s = await scene(rules(
    named('Banned words', 'message_posted', { type: 'ContainsWords', keywords: ['spam'] }, [{ type: 'ModerateMessage' }]),
    // Its only action is covered by the first one's, which carried it out: nothing is told for it either.
    named('Trial', 'message_posted', { type: 'ContainsWords', keywords: ['spam'] }, [{ type: 'ModerateMessage' }], false),
  ));
  await moderated(s, 'spam', ["**Banned words**: contains word: 'spam'"]);
});

// US-D1
test('only the rules for when someone sends a message are checked on a message', async () => {
  const s = await scene(rules(
    joinRule({ type: 'MemberNameContainsWords', keywords: ['bob'] }, [{ type: 'KickAuthor', delete_all_messages: false }], 'Names'),
    named('Banned words', 'message_posted', { type: 'ContainsWords', keywords: ['spam'] }, [{ type: 'ModerateMessage' }]),
  ));
  await moderated(s, 'spam', ["**Banned words**: contains word: 'spam'"]);
});

// US-D1
test('every rule that matches contributes its actions', async () => {
  // The second rule does more than the first, so it is checked and adds its observer.
  const s = await scene(rules(
    rule({ type: 'ContainsWords', keywords: ['spam'] }),
    rule({ type: 'ContainsLinksInList', domains: ['spam.example'] }, [{ type: 'ModerateMessage' }, { type: 'SetAuthorObserver', duration_minutes: 10 }]),
  ));
  const item = await s.member.post(s.memberGroup, 'spam at https://spam.example');
  const seen = [await s.member.seesInGroup(), await s.member.seesInGroup()];
  expect(seen).toContainEqual({ kind: 'deleted', itemIds: [item] });
  expect(seen).toContainEqual({ kind: 'role', role: 'observer' });
  expect(await s.owner.message()).toBe(notice('set the member as observer for 10 minutes and moderated the message', 'spam at https://spam.example', ["contains word: 'spam'", 'spam.example']));
});

// US-D2
test('notifications mode acts and tells the owner', async () => {
  const s = await scene(rules(rule({ type: 'ContainsLinksInList', domains: ['spam.example'] }, [
    { type: 'ModerateMessage' }, { type: 'SetAuthorObserver', duration_minutes: 60 },
  ])));
  const item = await s.member.post(s.memberGroup, 'Buy cheap followers at https://spam.example');
  const seen = [await s.member.seesInGroup(), await s.member.seesInGroup()];
  expect(seen).toContainEqual({ kind: 'deleted', itemIds: [item] });
  expect(seen).toContainEqual({ kind: 'role', role: 'observer' });
  expect(await s.owner.message()).toBe(notice('set the member as observer for 60 minutes and moderated the message', 'Buy cheap followers at https://spam.example', ['spam.example']));
});

// US-D2
test('the reason line names the rule, in bold, before what matched', async () => {
  const s = await scene(rules(named('Links from newcomers', 'message_posted', { type: 'ContainsLinksInList', domains: ['spam.example'] }, [
    { type: 'ModerateMessage' }, { type: 'SetAuthorObserver', duration_minutes: 60 },
  ])));
  const item = await s.member.post(s.memberGroup, 'Buy cheap followers at https://spam.example');
  const seen = [await s.member.seesInGroup(), await s.member.seesInGroup()];
  expect(seen).toContainEqual({ kind: 'deleted', itemIds: [item] });
  expect(await s.owner.message()).toBe(
    notice('set the member as observer for 60 minutes and moderated the message', 'Buy cheap followers at https://spam.example', ['**Links from newcomers**: spam.example']),
  );
});

// US-D2
test('the notification quotes the message, cut after 300 characters', async () => {
  const s = await scene(rules(rule({ type: 'ContainsWords', keywords: ['spam'] }), rule({ type: 'ContainsImage' })));
  const long = `spam ${'x'.repeat(400)}`;
  await moderated(s, long, ["contains word: 'spam'"], `${long.slice(0, 300)}...`);
  await moderated(s, 'first line spam\nsecond line', ["contains word: 'spam'"]);
  await moderated(s, { type: 'image', text: '', image: PIXEL }, ['contains an image'], null);
});

// US-D2
test('each matched rule gives one reason line, in the order of the rules', async () => {
  const s = await scene(rules(
    rule({ type: 'ContainsWords', keywords: ['spam'] }),
    rule({ type: 'ContainsImage' }, [{ type: 'ModerateMessage' }, { type: 'SetAuthorObserver', duration_minutes: 10 }]),
  ));
  const item = await s.member.post(s.memberGroup, { type: 'image', text: 'spam', image: PIXEL });
  const seen = [await s.member.seesInGroup(), await s.member.seesInGroup()];
  expect(seen).toContainEqual({ kind: 'deleted', itemIds: [item] });
  expect(seen).toContainEqual({ kind: 'role', role: 'observer' });
  expect(await s.owner.message()).toBe(notice('set the member as observer for 10 minutes and moderated the message', 'spam', ["contains word: 'spam'", 'contains an image']));
});

// US-D2
test('the message is named before the kick', async () => {
  // The observer before the message is the first test of US-D2.
  const s = await scene(rules(rule({ type: 'ContainsWords', keywords: ['spam'] }, [
    { type: 'KickAuthor', delete_all_messages: false }, { type: 'ModerateMessage' },
  ])));
  await s.member.post(s.memberGroup, 'spam');
  expect(await s.owner.message()).toBe(notice('moderated the message and kicked the member', 'spam', ["contains word: 'spam'"]));
});

// US-D2
test('the notification names the actions and the reasons of the rules with Notifications on alone', async () => {
  const s = await scene(rules(
    // Carried out, but not told: its action and its reason are not in the notification.
    named('Quiet', 'message_posted', { type: 'ContainsWords', keywords: ['spam'] }, [{ type: 'ModerateMessage' }], true, false),
    named('Loud', 'message_posted', { type: 'ContainsWords', keywords: ['spam'] }, [{ type: 'SetAuthorObserver', duration_minutes: 10 }]),
  ));
  const item = await s.member.post(s.memberGroup, 'spam');
  const seen = [await s.member.seesInGroup(), await s.member.seesInGroup()];
  expect(seen).toContainEqual({ kind: 'deleted', itemIds: [item] });
  expect(seen).toContainEqual({ kind: 'role', role: 'observer' });
  expect(await s.owner.message()).toBe(notice('set the member as observer for 10 minutes', 'spam', ["**Loud**: contains word: 'spam'"]));
});

// US-D3
test('a message that matches rules with Notifications off is acted on without telling', async () => {
  const s = await scene(rules(named('Quiet', 'message_posted', { type: 'ContainsWords', keywords: ['spam'] }, [{ type: 'ModerateMessage' }], true, false)));
  const item = await s.member.post(s.memberGroup, 'spam');
  expect(await s.member.seesInGroup()).toEqual({ kind: 'deleted', itemIds: [item] });
  await s.owner.hearsNothing(5000);
});

// US-D3
test('only the actions of the rules with Actions on are carried out when no rule tells', async () => {
  const s = await scene(rules(
    named('Quiet', 'message_posted', { type: 'ContainsWords', keywords: ['spam'] }, [{ type: 'ModerateMessage' }], true, false),
    // Both switches off: not checked, so the member is not kicked and nothing is told.
    named('Off', 'message_posted', { type: 'ContainsWords', keywords: ['spam'] }, [{ type: 'KickAuthor', delete_all_messages: false }], false, false),
  ));
  const item = await s.member.post(s.memberGroup, 'spam');
  expect(await s.member.seesInGroup()).toEqual({ kind: 'deleted', itemIds: [item] });
  await s.member.seesNothing(4000);
  await s.owner.hearsNothing(0);
});

// US-D4
test('a rule with Actions off only tells what it would do, and the reason line is marked', async () => {
  const dry = (actions: unknown[]) => ({ rules: [turnedOff(rule({ type: 'ContainsWords', keywords: ['spam'] }, actions), 'enabled')] });
  const s = await scene(dry([{ type: 'SetAuthorObserver', duration_minutes: 60 }, { type: 'ModerateMessage' }]));
  for (const [actions, did] of [
    [[{ type: 'SetAuthorObserver', duration_minutes: 60 }, { type: 'ModerateMessage' }], 'would set the member as observer for 60 minutes and moderate the message'],
    [[{ type: 'ModerateMessage' }, { type: 'KickAuthor', delete_all_messages: false }], 'would moderate the message and kick the member'],
    [[{ type: 'KickAuthor', delete_all_messages: true }], 'would kick the member and delete all their messages'],
  ] as const) {
    await apply(s, dry([...actions]));
    await s.member.post(s.memberGroup, 'spam');
    expect(await s.owner.message()).toBe(notice(did, 'spam', ["_(off)_ contains word: 'spam'"]));
    // Nothing in the group: the message stays, the member stays a member.
    await s.member.seesNothing(4000);
  }
});

// US-D4
test('the reason line of a rule with Actions off names the rule, then marks it', async () => {
  const s = await scene(rules(named('Flood', 'message_posted', { type: 'ContainsWords', keywords: ['spam'] }, [{ type: 'ModerateMessage' }], false)));
  await s.member.post(s.memberGroup, 'spam');
  expect(await s.owner.message()).toBe(notice('would moderate the message', 'spam', ["**Flood** _(off)_: contains word: 'spam'"]));
  await s.member.seesNothing(4000);
});

// US-D4
test('a message that matches rules that act and rules that only tell is told in one notification', async () => {
  const s = await scene(rules(
    named('Banned words', 'message_posted', { type: 'ContainsWords', keywords: ['spam'] }, [{ type: 'ModerateMessage' }]),
    named('Trial kick', 'message_posted', { type: 'ContainsWords', keywords: ['spam'] }, [{ type: 'KickAuthor', delete_all_messages: false }], false),
  ));
  const item = await s.member.post(s.memberGroup, 'spam');
  expect(await s.member.seesInGroup()).toEqual({ kind: 'deleted', itemIds: [item] });
  expect(await s.owner.message()).toBe(
    mixedNotice('moderated the message', 'kick the member', 'spam', ["**Banned words**: contains word: 'spam'", "**Trial kick** _(off)_: contains word: 'spam'"]),
  );
  // Nothing more reaches the owner or the group: the member was not kicked.
  await s.member.seesNothing(4000);
  await s.owner.hearsNothing(0);
});

// US-D4
test('an action it would carry out that one carried out already covers is not named', async () => {
  const s = await scene(rules(
    named('Banned words', 'message_posted', { type: 'ContainsWords', keywords: ['spam'] }, [{ type: 'ModerateMessage' }]),
    // Covered by the moderation that was carried out; the rule is still told, as a rule that is off covers nothing but is checked.
    named('Trial', 'message_posted', { type: 'ContainsWords', keywords: ['spam'] }, [{ type: 'ModerateMessage' }, { type: 'SetAuthorObserver', duration_minutes: 10 }], false),
  ));
  const item = await s.member.post(s.memberGroup, 'spam');
  expect(await s.member.seesInGroup()).toEqual({ kind: 'deleted', itemIds: [item] });
  expect(await s.owner.message()).toBe(
    mixedNotice('moderated the message', 'set the member as observer for 10 minutes', 'spam', ["**Banned words**: contains word: 'spam'", "**Trial** _(off)_: contains word: 'spam'"]),
  );
});

// US-D5
test('a moderator, an admin or an owner is never checked', async () => {
  const s = await scene(rules(rule({ type: 'ContainsWords', keywords: ['spam'] })));
  await s.owner.post(s.ownerGroup, 'spam from the owner');
  await s.owner.seesNothing(5000);
  for (const role of ['moderator', 'admin']) {
    await s.owner.setRole(s.ownerGroup, s.member.name, role);
    expect(await s.member.seesInGroup()).toEqual({ kind: 'role', role });
    await s.member.post(s.memberGroup, `spam from a ${role}`);
    await s.member.seesNothing(5000);
    await s.owner.hearsNothing(0);
  }
});

// US-D5: joining is no event for a moderator, an admin or an owner, whichever way they come in.
test('someone the owner adds as a moderator, an admin or an owner is never checked', async () => {
  const s = await scene(rules(joinRule(undefined, [{ type: 'KickAuthor', delete_all_messages: false }], 'Everyone out')));
  for (const role of ['moderator', 'admin', 'owner']) {
    const person = await Person.create(role);
    await s.owner.befriend(person);
    await s.owner.addToGroup(s.ownerGroup, person, role);
    await person.acceptInvitation();
    await person.seesNothing(5000);
    await s.owner.hearsNothing(0);
  }
});

// ---- When someone joins ----

/** The name Free Casino Bonus, as a display name made unique at its end. */
const FREE = 'Free Casino Bonus';

// US-D6
test('a join is checked against the rules for when someone joins, and told by a rule with both switches on', async () => {
  const s = await scene(rules(joinRule({ type: 'MemberNameContainsWords', keywords: ['casino'] }, [{ type: 'KickAuthor', delete_all_messages: false }], 'Spam names')));
  const bob = await Person.create(FREE);
  await bob.joinGroupAndWatch(await s.owner.groupLink(s.ownerGroup));
  expect(await bob.seesInGroup()).toEqual({ kind: 'removed' });
  expect(await s.owner.message()).toBe(joinNotice('kicked the member', bob.name, ["**Spam names**: name contains word: 'casino'"]));
  // Someone with another name stays.
  const carol = await Person.create('Carol');
  await carol.joinGroupAndWatch(await s.owner.groupLink(s.ownerGroup));
  await carol.seesNothing(5000);
  await s.owner.hearsNothing(0);
});

// US-D6
test('a rule with no condition matches everyone who joins, and a rule without a name gives what matched alone', async () => {
  const s = await scene(rules(
    // The shorter time first: it matches everyone, and the longer one is not covered by it.
    joinRule(undefined, [{ type: 'SetAuthorObserver', duration_minutes: 10 }], 'Quarantine newcomers'),
    joinRule({ type: 'MemberNameContainsWords', keywords: ['casino'] }, [{ type: 'SetAuthorObserver', duration_minutes: 60 }]),
  ));
  const bob = await Person.create(FREE);
  await bob.joinGroupAndWatch(await s.owner.groupLink(s.ownerGroup));
  expect(await bob.seesInGroup()).toEqual({ kind: 'role', role: 'observer' });
  // Every rule that matches contributes; a longer observer time covers a shorter one (US-F4).
  expect(await s.owner.message()).toBe(
    joinNotice('set the member as observer for 60 minutes', bob.name, ['**Quarantine newcomers**: everyone who joins', "name contains word: 'casino'"]),
  );
  // The observer time is up: they are a member again (US-F2).
  advance(61);
  expect(await bob.seesInGroup(120_000)).toEqual({ kind: 'role', role: 'member' });
});

// US-D6
test('a join is checked in the order of the rules, skipping those with both switches off and those already covered', async () => {
  const s = await scene(rules(
    named('Off', 'member_joined', undefined, [{ type: 'KickAuthor', delete_all_messages: false }], false, false),
    joinRule({ type: 'MemberNameContainsWords', keywords: ['casino'] }, [{ type: 'KickAuthor', delete_all_messages: false }], 'First'),
    // Its only action is covered by the first one's: it is not checked, and gives no reason.
    joinRule({ type: 'MemberNameContainsWords', keywords: ['casino'] }, [{ type: 'KickAuthor', delete_all_messages: false }], 'Second'),
  ));
  const bob = await Person.create(FREE);
  await bob.joinGroupAndWatch(await s.owner.groupLink(s.ownerGroup));
  expect(await bob.seesInGroup()).toEqual({ kind: 'removed' });
  expect(await s.owner.message()).toBe(joinNotice('kicked the member', bob.name, ["**First**: name contains word: 'casino'"]));
});

// US-D6
test('someone who joins as an observer is checked too', async () => {
  const s = await scene(rules(joinRule(undefined, [{ type: 'KickAuthor', delete_all_messages: false }], 'Everyone out')));
  const person = await Person.create('Olga');
  await person.joinGroupAndWatch(await s.owner.groupLink(s.ownerGroup, 'observer'));
  expect(await person.seesInGroup()).toEqual({ kind: 'removed' });
  expect(await s.owner.message()).toBe(joinNotice('kicked the member', person.name, ['**Everyone out**: everyone who joins']));
});

// US-D6
test('a rule with Notifications off acts on a join without telling', async () => {
  const s = await scene(rules(turnedOff(joinRule(undefined, [{ type: 'KickAuthor', delete_all_messages: false }], 'Everyone out'), 'notify')));
  const bob = await Person.create('Bob');
  await bob.joinGroupAndWatch(await s.owner.groupLink(s.ownerGroup));
  expect(await bob.seesInGroup()).toEqual({ kind: 'removed' });
  await s.owner.hearsNothing(5000);
});

// US-D6
test('a rule with Actions off tells what it would do on a join, and does nothing', async () => {
  const s = await scene(rules(turnedOff(joinRule({ type: 'MemberNameContainsWords', keywords: ['casino'] }, [{ type: 'KickAuthor', delete_all_messages: false }], 'Spam names'), 'enabled')));
  const bob = await Person.create(FREE);
  const group = await bob.joinGroupAndWatch(await s.owner.groupLink(s.ownerGroup));
  expect(await s.owner.message()).toBe(joinNotice('would kick the member', bob.name, ["**Spam names** _(off)_: name contains word: 'casino'"]));
  // Nothing in the group: they stay, and can still write.
  await bob.seesNothing(4000);
  await bob.post(group, 'hello');
  await bob.seesNothing(3000);
});

// US-D6
test('someone added to the group by another member is checked as one who joins by its link', async () => {
  const s = await scene(rules(joinRule({ type: 'MemberNameContainsWords', keywords: ['casino'] }, [{ type: 'KickAuthor', delete_all_messages: false }], 'Spam names')));
  // Bob, an admin, adds a contact of his: a member cannot add people.
  await s.owner.setRole(s.ownerGroup, s.member.name, 'admin');
  expect(await s.member.seesInGroup()).toEqual({ kind: 'role', role: 'admin' });
  const carol = await Person.create(FREE);
  await s.member.befriend(carol);
  await s.member.addToGroup(s.memberGroup, carol);
  await carol.acceptInvitation();
  expect(await carol.seesInGroup()).toEqual({ kind: 'removed' });
  expect(await s.owner.message()).toBe(joinNotice('kicked the member', carol.name, ["**Spam names**: name contains word: 'casino'"]));
});

// US-D6
test('someone the owner adds to the group as a member or an observer is checked', async () => {
  const s = await scene(rules(joinRule(undefined, [{ type: 'KickAuthor', delete_all_messages: false }], 'Everyone out')));
  for (const role of ['member', 'observer']) {
    const person = await Person.create(`Added${role}`);
    await s.owner.befriend(person);
    await s.owner.addToGroup(s.ownerGroup, person, role);
    await person.acceptInvitation();
    expect(await person.seesInGroup()).toEqual({ kind: 'removed' });
    expect(await s.owner.message()).toBe(joinNotice('kicked the member', person.name, ['**Everyone out**: everyone who joins']));
  }
});

const PIXEL = 'data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNkYAAAAAYAAjCB0C8AAAAASUVORK5CYII=';
