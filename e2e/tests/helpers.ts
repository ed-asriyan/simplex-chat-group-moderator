// What the stories share: a moderated group with a member in it, the owner's
// notification, and the editor opened on a link.
import { expect, Page } from '@playwright/test';
import { Person, botId, configOf, settingsLink } from '../harness/person';

export const ISSUE = 'https://github.com/ed-asriyan/simplex-chat-group-moderator/issues/new?template=moderation-rule-bug.yml';

/** An owner, their group Crypto Chat moderated by the bot, and Bob in it. */
export interface Scene {
  owner: Person;
  member: Person;
  /** The group in the owner's CLI and in Bob's. */
  ownerGroup: number;
  memberGroup: number;
  /** The bot's id for the group, and the editor link it sent. */
  id: string;
  link: string;
}

export async function scene(config?: unknown, role = 'owner', memberName: string | [string, string] = 'Bob'): Promise<Scene> {
  const owner = await Person.join('Alice');
  await owner.message();
  const { group, link } = await owner.moderatedGroup('Crypto Chat', role);
  const member = Array.isArray(memberName) ? await Person.create(memberName[0], memberName[1]) : await Person.create(memberName);
  const memberGroup = await member.joinGroup(await owner.groupLink(group));
  const s = { owner, member, ownerGroup: group, memberGroup, id: botId(link), link };
  if (config) await apply(s, config);
  return s;
}

/** The owner sends the configuration's link, which the bot saves. */
export async function apply(s: Scene, config: unknown) {
  expect(await s.owner.ask(settingsLink(s.id, config))).toBe('✅ Group settings updated.');
}

/** One rule for when someone sends a message: `condition` and `actions` (by default, moderate the message). */
export function rule(condition: unknown, actions: unknown[] = [{ type: 'ModerateMessage' }]) {
  return { actions, condition };
}

/** A rule with a name, a trigger and, if it is off, `enabled: false` (US-G13). */
export function named(name: string, trigger: 'message_posted' | 'member_joined', condition: unknown, actions: unknown[], enabled = true) {
  return { name, trigger, ...(condition === undefined ? {} : { condition }), actions, ...(enabled ? {} : { enabled: false }) };
}

/** A rule for when someone joins; no `condition` matches everyone who joins. */
export function joinRule(condition: unknown, actions: unknown[], name?: string) {
  return { ...(name ? { name } : {}), trigger: 'member_joined', ...(condition === undefined ? {} : { condition }), actions };
}

export function rules(...list: unknown[]) {
  return { mode: 'notifications', rules: list };
}

/** The owner's notification of US-D2. */
export function notice(did: string, message: string | null, reasons: string[], group = 'Crypto Chat') {
  // Each line quoted; the story does not say how a line break inside a quote is written.
  const quoted = message === null ? '_(no text)_' : message.split('\n').map((l) => `> ${l}`).join('\n');
  return `🛡 I ${did} in **${group}**.\n\n**The message:**\n${quoted}\n\n**Reason:**\n${reasons.map((r) => `- ${r}`).join('\n')}\n\nIf it's a false positive, please [file an issue](${ISSUE}).`;
}

/** Bob posts `content`; it is moderated, and the owner is told why. */
export async function moderated(s: Scene, content: string | Record<string, unknown>, reasons: string[], message?: string | null) {
  const item = await s.member.post(s.memberGroup, content);
  expect(await s.member.seesInGroup()).toEqual({ kind: 'deleted', itemIds: [item] });
  const text = message !== undefined ? message : quoted(typeof content === 'string' ? content : (content.text as string) || null);
  expect(lineEnds(await s.owner.message())).toBe(lineEnds(notice('moderated the message', text, reasons)));
}

/** A message as the notification quotes it: cut after 300 characters. */
export function quoted(text: string | null): string | null {
  return text !== null && [...text].length > 300 ? `${[...text].slice(0, 300).join('')}...` : text;
}

/** The owner's notification of a join (US-D6): what the bot did, and to whom. */
export function joinNotice(did: string, member: string, reasons: string[], group = 'Crypto Chat') {
  return `🛡 I ${did} when they joined **${group}**.\n\n**The member:** ${member}\n\n**Reason:**\n${reasons.map((r) => `- ${r}`).join('\n')}\n\nIf it's a false positive, please [file an issue](${ISSUE}).`;
}

/** A notification with the spaces at the ends of its lines left out: the story does not say how a line ends. */
export function lineEnds(text: string): string {
  return text.replace(/ +\n/g, '\n');
}

/** Bob posts `content`; it is moderated, and the owner is told, whatever the reason. */
export async function moderatedAnyway(s: Scene, content: string | Record<string, unknown>) {
  const item = await s.member.post(s.memberGroup, content);
  expect(await s.member.seesInGroup()).toEqual({ kind: 'deleted', itemIds: [item] });
  expect(await s.owner.message()).toMatch(/^🛡 I moderated the message in \*\*Crypto Chat\*\*\.\n/);
}

/** Another member who joins the scene's group. */
export async function anotherMember(s: Scene, name = 'Carol'): Promise<{ person: Person; group: number }> {
  const person = await Person.create(name);
  const group = await person.joinGroup(await s.owner.groupLink(s.ownerGroup));
  return { person, group };
}

/** Bob posts `content`, and nothing happens. */
export async function untouched(s: Scene, content: string | Record<string, unknown>) {
  await s.member.post(s.memberGroup, content);
  await s.member.seesNothing(4000);
  await s.owner.hearsNothing(0);
}

// ---- the editor ----
// Every element is reached as a person does: by its role and its accessible name.

export const MESSAGES = '💬 When someone sends a message';
export const JOINS = '🚪 When someone joins';

/** The bot's group 7 is the editor's, at a fixed id, for the screenshots; a link from the bot carries its own. */
export function editorUrl(config: unknown, id: string | null = '7', group: string | null = 'Crypto Chat') {
  const link = settingsLink(id ?? '7', config, group ?? undefined).match(/\((http[^)]+)\)/)![1];
  return id === null ? link.replace(/bot_id=\d+&/, '') : link;
}

/** Opens the editor on an empty configuration of the bot's group 7. */
export async function emptyEditor(page: Page) {
  await open(page, editorUrl({ mode: 'notifications', rules: [] }));
}

/**
 * The page as the person sees it now matches its approved screenshot,
 * `tests/__screenshots__/<project>/<spec>/<name>.png` (AGENTS.md, *Workflow*).
 */
export async function looks(page: Page, name: string) {
  await expect(page).toHaveScreenshot(`${name}.png`, { fullPage: true });
}

/** Opens `url` afresh: going to another link that differs only by its hash would not reload the editor. */
export async function open(page: Page, url: string) {
  await page.goto('about:blank');
  await page.goto(url);
}

/** The editor link the bot sends for the scene's group now (US-B4). */
export async function currentLink(s: Scene): Promise<string> {
  await s.owner.send('/groups');
  return (await s.owner.message()).match(/\((http[^)]+)\)/)![1];
}

export const section = (page: Page, name: string) => page.getByRole('region', { name, exact: true });

/** The section's *+ Add rule*: the page for a new rule. */
export async function newRule(page: Page, trigger: 'messages' | 'joins' = 'messages') {
  await page.getByRole('button', { name: trigger === 'messages' ? '+ Add rule when someone sends a message' : '+ Add rule when someone joins', exact: true }).click();
  await expect(page.getByRole('heading', { name: 'New rule', level: 1 })).toBeVisible();
}

/** A type in the picker, by its title in the stories. */
export const pickable = (page: Page, title: string) =>
  page.getByRole('dialog', { name: /^(Pick a condition|Pick an action|Change the condition's type|Change the action's type)$/ }).getByRole('button', { name: new RegExp(`^${title.replace(/[.*+?^${}()|[\]\\/]/g, '\\$&')}\\s`) });

/** From the group's page: a new rule for `trigger` built by hand, its condition `title` picked; its page is open. */
export async function addCondition(page: Page, title: string, trigger: 'messages' | 'joins' = 'messages') {
  await newRule(page, trigger);
  await page.getByRole('button', { name: /^Build it myself/ }).click();
  await pickable(page, title).click();
}

/** The same, from an empty editor. */
export async function conditionEditor(page: Page, title: string, trigger: 'messages' | 'joins' = 'messages') {
  await emptyEditor(page);
  await addCondition(page, title, trigger);
}

/** The group's page, from a rule's or New rule's. */
export async function toGroup(page: Page) {
  await page.getByRole('button', { name: 'Back to rules' }).click();
}

/** Opens the rule called `name` from the group's page. */
export async function openRule(page: Page, name: string | RegExp) {
  await page.getByRole('button', { name, exact: typeof name === 'string' }).click();
}

/** The condition `title` is in the rule's page. */
export const conditionRow = (page: Page, title: string) => page.getByRole('button', { name: `About “${title}”`, exact: true });

/** *Apply changes*, or *Apply N changes*: its dialog. */
export async function applyDialog(page: Page) {
  await page.getByRole('button', { name: /^Apply (changes|\d+ changes?)$/ }).click();
  return page.getByRole('dialog', { name: 'Changes before you send them' });
}

/** The configuration the editor would send now: through *Apply changes* and *Copy the link*. */
export async function savedConfig(page: Page): Promise<any> {
  // Apply is on the group's page alone.
  if (await page.getByRole('button', { name: 'Back to rules' }).isVisible()) await toGroup(page);
  const dialog = await applyDialog(page);
  await dialog.getByRole('button', { name: 'Copy the link' }).click();
  await expect(dialog.getByText('Link copied.')).toBeVisible();
  const config = configOf(await page.evaluate(() => navigator.clipboard.readText()));
  await dialog.getByRole('button', { name: 'Close' }).click();
  return config;
}
