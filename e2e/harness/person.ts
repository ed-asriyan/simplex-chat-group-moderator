// A person in the people's SimpleX CLI: a user profile of its own, which talks
// to the bot in a direct chat and owns or joins groups the bot moderates.
// What they see arrives as their CLI's events: the bot's messages, messages
// deleted, their role changed, their removal from a group.
import LZ from 'lz-string';
import { Cli, Event, Queue } from './cli';
import { botAddress, EDITOR, PERSON_WS } from './stack';

/** How long a person waits for something before the test fails. */
export const PATIENCE = 60_000;

let cli: Cli | undefined;
let count = 0;

async function shared(): Promise<Cli> {
  cli ??= await Cli.connect(PERSON_WS);
  return cli;
}

/** What a person sees happen in a group. */
export type GroupEvent =
  | { kind: 'deleted'; itemIds: number[] }
  | { kind: 'role'; role: string }
  | { kind: 'removed' };

export class Person {
  private constructor(
    private cli: Cli,
    readonly name: string,
    readonly userId: number,
    private events: Queue,
    /** The bot as a contact of this person, once connected. */
    public bot = 0,
  ) {}

  /**
   * A new person, not yet connected to the bot. Their display name is `name`
   * made unique: the token goes where `{}` is in it, or at its end. A name
   * with spaces (`Free Casino Bonus`) is a display name with spaces, as
   * SimpleX allows; `fullName` is the profile's full name.
   */
  static async create(name: string, fullName = ''): Promise<Person> {
    const c = await shared();
    const token = `${++count}x${Date.now() % 100000}`;
    const unique = name.includes('{}') ? name.replace('{}', token) : `${name}${token}`;
    const events = c.listen(unique);
    const plain = !/\s/.test(unique) && fullName === '';
    const created = await c.command(
      plain ? `/create user ${unique}` : `/_create user ${JSON.stringify({ profile: { displayName: unique, fullName }, pastTimestamp: false })}`,
    );
    if (created.type !== 'activeUser') throw new Error(`create ${unique}: ${JSON.stringify(created)}`);
    return new Person(c, unique, created.user.userId, events);
  }

  /** A new person who connects to the bot; the welcome is the next message. */
  static async join(name: string): Promise<Person> {
    const person = await Person.create(name);
    await person.connect();
    return person;
  }

  async connect() {
    await this.activate();
    const sent = await this.cli.command(`/c ${botAddress()}`);
    if (sent.type !== 'sentInvitation') throw new Error(`connect: ${JSON.stringify(sent)}`);
    const connected = await this.until((e) => e.type === 'contactConnected');
    this.bot = connected.contact.contactId;
  }

  private links = new Map<string, string>();

  private async activate() {
    // A name with spaces cannot be typed as a command: the user's id can.
    const r = await this.cli.command(/\s/.test(this.name) ? `/_user ${this.userId}` : `/user ${this.name}`);
    if (r.type !== 'activeUser') throw new Error(`activate ${this.name}: ${JSON.stringify(r)}`);
  }

  private async next(ms = PATIENCE): Promise<Event> {
    const e = await this.events.next(ms);
    if (!e) throw new Error(`${this.name}: nothing in ${ms / 1000}s`);
    if (process.env.E2E_DEBUG) console.log(this.name, e.type, JSON.stringify(e).slice(0, 300));
    return e;
  }

  private async until(match: (e: Event) => boolean, ms = PATIENCE): Promise<Event> {
    const deadline = Date.now() + ms;
    for (;;) {
      const e = await this.next(Math.max(1, deadline - Date.now()));
      if (match(e)) return e;
    }
  }

  // ---- the chat with the bot ----

  /** Sends `text` to the bot. */
  async send(text: string) {
    await this.activate();
    const r = await this.cli.command(`/_send @${this.bot} json ${JSON.stringify([{ msgContent: { type: 'text', text } }])}`);
    if (r.type !== 'newChatItems') throw new Error(`send: ${JSON.stringify(r)}`);
  }

  /** The bot's next message to this person in their direct chat. */
  async message(ms = PATIENCE): Promise<string> {
    for (;;) {
      const e = await this.next(ms);
      if (e.type !== 'newChatItems') continue;
      for (const item of e.chatItems) {
        if (item.chatInfo?.type === 'direct' && item.chatInfo.contact?.contactId === this.bot && item.chatItem?.content?.type === 'rcvMsgContent')
          return item.chatItem.content.msgContent.text;
      }
    }
  }

  /** Sends `text` and returns the bot's answer. */
  async ask(text: string): Promise<string> {
    await this.send(text);
    return this.message();
  }

  /** Asserts that the bot sends nothing for `ms`. */
  async hearsNothing(ms: number) {
    const deadline = Date.now() + ms;
    for (;;) {
      const left = deadline - Date.now();
      if (left <= 0) return;
      const e = await this.events.next(left);
      if (!e) return;
      if (e.type === 'newChatItems')
        for (const item of e.chatItems)
          if (item.chatInfo?.type === 'direct' && item.chatItem?.content?.type === 'rcvMsgContent')
            throw new Error(`${this.name}: expected nothing, the bot said ${JSON.stringify(item.chatItem.content.msgContent.text)}`);
    }
  }

  // ---- groups ----

  /** Creates a group; returns its id in this person's CLI. */
  async createGroup(name: string): Promise<number> {
    await this.activate();
    const r = await this.cli.command(`/_group ${this.userId} ${JSON.stringify({ displayName: name, fullName: '' })}`);
    if (r.type !== 'groupCreated') throw new Error(`create group: ${JSON.stringify(r)}`);
    return r.groupInfo.groupId;
  }

  /** Invites the bot to the group with `role`. */
  async inviteBot(group: number, role: string) {
    await this.activate();
    const r = await this.cli.command(`/_add #${group} ${this.bot} ${role}`);
    if (r.type !== 'sentGroupInvitation') throw new Error(`invite the bot: ${JSON.stringify(r)}`);
  }

  /**
   * A group of this person's the bot moderates: created, the bot invited as
   * `role` and joined, its two messages read. Returns the group and the
   * editor link the bot sent for it.
   */
  async moderatedGroup(name = 'Crypto Chat', role = 'moderator'): Promise<{ group: number; link: string }> {
    const group = await this.createGroup(name);
    await this.inviteBot(group, role);
    const joined = await this.message();
    if (joined !== '✅ I joined the group. Open the editor below to set up its rules.') throw new Error(`joined: ${joined}`);
    const link = (await this.message()).match(/\((http[^)]+)\)/)![1];
    return { group, link };
  }

  /** A link to join the group with `role` (a member by default); a group has one for each role, made the first time. */
  async groupLink(group: number, role = 'member'): Promise<string> {
    const key = `${group}/${role}`;
    const made = this.links.get(key);
    if (made) return made;
    await this.activate();
    const r = await this.cli.command(`/_create link #${group} ${role}`);
    const link = JSON.stringify(r).match(/"connShortLink":"([^"]+)"/) || JSON.stringify(r).match(/"connFullLink":"([^"]+)"/);
    if (!link) throw new Error(`group link: ${JSON.stringify(r)}`);
    this.links.set(key, link[1]);
    return link[1];
  }

  /** Joins a group by its link; returns its id in this person's CLI once connected. */
  async joinGroup(link: string): Promise<number> {
    await this.activate();
    const r = await this.cli.command(`/c ${link}`);
    if (r.type !== 'sentInvitation' && r.type !== 'sentConfirmation') throw new Error(`join: ${JSON.stringify(r)}`);
    const joined = await this.until((e) => e.type === 'userJoinedGroup');
    await this.until((e) => e.type === 'connectedToGroupMember', PATIENCE).catch(() => undefined);
    return joined.groupInfo.groupId;
  }

  /**
   * Joins a group by its link and returns its id as soon as this person is in:
   * what the bot does to them next (a role changed, their removal) is still to
   * be seen with `seesInGroup`, which `joinGroup` would have read past.
   */
  async joinGroupAndWatch(link: string): Promise<number> {
    await this.activate();
    const r = await this.cli.command(`/c ${link}`);
    if (r.type !== 'sentInvitation' && r.type !== 'sentConfirmation') throw new Error(`join: ${JSON.stringify(r)}`);
    const joined = await this.until((e) => e.type === 'userJoinedGroup');
    return joined.groupInfo.groupId;
  }

  /** Posts a message in the group; returns its id in this person's CLI. */
  async post(group: number, content: string | Record<string, unknown>): Promise<number> {
    await this.activate();
    const msgContent = typeof content === 'string' ? { type: 'text', text: content } : content;
    const r = await this.cli.command(`/_send #${group} json ${JSON.stringify([{ msgContent }])}`);
    if (r.type !== 'newChatItems') throw new Error(`post: ${JSON.stringify(r)}`);
    return r.chatItems[0].chatItem.meta.itemId;
  }

  /** Edits one of this person's messages in the group. */
  async edit(group: number, item: number, text: string) {
    await this.activate();
    const r = await this.cli.command(`/_update item #${group} ${item} json ${JSON.stringify({ msgContent: { type: 'text', text }, mentions: {} })}`);
    if (r.type !== 'chatItemUpdated') throw new Error(`edit: ${JSON.stringify(r)}`);
  }

  /** Gives a member of the group another role. */
  async setRole(group: number, member: string, role: string) {
    await this.activate();
    const members = await this.cli.command(`/_members #${group}`);
    const m = members.group.members.find((x: any) => x.localDisplayName === member);
    const r = await this.cli.command(`/_member role #${group} ${m.groupMemberId} ${role}`);
    if (r.type !== 'membersRoleUser' && r.type !== 'memberRoleUser') throw new Error(`role: ${JSON.stringify(r).slice(0, 300)}`);
  }

  /** Removes the bot from the group. */
  async removeBot(group: number) {
    await this.activate();
    const members = await this.cli.command(`/_members #${group}`);
    const bot = members.group.members.find((m: any) => m.memberContactId === this.bot);
    const r = await this.cli.command(`/_remove #${group} ${bot.groupMemberId}`);
    if (r.type !== 'userDeletedMembers') throw new Error(`remove the bot: ${JSON.stringify(r)}`);
  }

  /** The messages others posted in the group, oldest first, as this person's chat shows them now. */
  async history(group: number): Promise<{ text: string; deleted: boolean }[]> {
    await this.activate();
    const r = await this.cli.command(`/_get chat #${group} count=100`);
    if (r.type !== 'apiChat') throw new Error(`history: ${JSON.stringify(r).slice(0, 300)}`);
    return r.chat.chatItems
      .filter((ci: Event) => ci.content?.type === 'rcvMsgContent')
      .map((ci: Event) => ({ text: ci.content.msgContent.text, deleted: !!ci.meta.itemDeleted }));
  }

  /** The next thing this person sees happen in the group. */
  async seesInGroup(ms = PATIENCE): Promise<GroupEvent> {
    for (;;) {
      const e = await this.next(ms);
      if (e.type === 'chatItemsDeleted') return { kind: 'deleted', itemIds: e.chatItemDeletions.map((d: any) => d.deletedChatItem.chatItem.meta.itemId) };
      if (e.type === 'memberRole' && e.member?.groupMemberId === e.groupInfo?.membership?.groupMemberId) return { kind: 'role', role: e.toRole };
      if (e.type === 'deletedMemberUser') return { kind: 'removed' };
    }
  }

  /** Asserts that nothing happens to this person in groups, or from the bot, for `ms`. */
  async seesNothing(ms: number) {
    const deadline = Date.now() + ms;
    for (;;) {
      const left = deadline - Date.now();
      if (left <= 0) return;
      const e = await this.events.next(left);
      if (!e) return;
      if (['chatItemsDeleted', 'memberRole', 'deletedMemberUser'].includes(e.type)) throw new Error(`${this.name}: expected nothing, saw ${e.type}`);
      if (e.type === 'newChatItems')
        for (const item of e.chatItems)
          if (item.chatInfo?.type === 'direct' && item.chatItem?.content?.type === 'rcvMsgContent')
            throw new Error(`${this.name}: expected nothing, the bot said ${JSON.stringify(item.chatItem.content.msgContent.text)}`);
    }
  }
}

/** The bot's id for a group, from the link it sent for it. */
export function botId(link: string): string {
  return link.match(/bot_id=(\d+)/)![1];
}

/** The editor link carrying `config` for the bot's group `id`, as the editor copies it. */
export function settingsLink(id: string, config: unknown, group?: string): string {
  const named = group === undefined ? '' : `&group=${encodeURIComponent(group)}`;
  return `[Settings for ${group ?? `group ${id}`}](${EDITOR}/#bot_id=${id}${named}&config=${LZ.compressToEncodedURIComponent(JSON.stringify(config))})`;
}

/** The configuration in an editor link. */
export function configOf(link: string): any {
  const packed = link.match(/config=([^&)\s]+)/)![1];
  return JSON.parse(LZ.decompressFromEncodedURIComponent(packed)!);
}
