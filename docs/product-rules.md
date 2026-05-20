# Product Rules

This document closes the current product rule questions before architecture and database design. It is documentation only and does not prescribe implementation frameworks.

## 1. Summary

Novastrum separates private messaging from community discussion:

- Direct messages are private.
- Groups are private to members.
- Users only see groups they are members of.
- Group messages are visible only to members.
- New group members cannot see old messages.
- Group history requires a join-boundary marker such as `visible_from_message_id`.
- Communities are Reddit-like topic spaces.
- Public community content is discussion content, not open community chat.
- Discussion posts belong to communities.
- A mandatory General community is not required.
- A combined feed may replace a General community.
- Packet protocol starts in readable mode and may add compact mode later.

This document resolves the first-pass rules for DM policy, community visibility, combined feeds, community creation, group roles, deletion behavior, user status, identity, and packet registry boundaries.

## 2. Product Rules Table

| Area | Finalized rule | Can be deferred? | Main design impact |
| --- | --- | --- | --- |
| DM trust | Per-user `dm_policy`: `everyone`, `shared_group_members`, `friends_only`, `none`. | Friendship implementation can be deferred. | User settings, DM creation checks, first-message checks. |
| Communities | Start with public communities. Reading public discussion does not require joining. | Private communities can be deferred. | Community visibility model, discussion queries, feed filtering. |
| Combined feed | Use a combined discussion feed across visible communities. No mandatory General community. | Advanced filters can be deferred. | Feed queries aggregate visible community posts. |
| Community creation | Initially admin-only. | Owner/moderator community roles can be deferred. | Admin service creates communities; moderation model remains small. |
| Groups | Roles start with `owner` and `member`; max size is 10. | Rich roles can be deferred. | Group membership table needs role, status, join boundary. |
| Group history | Leave/rejoin creates a new visibility boundary. | No. | Message history filters must respect each membership period. |
| Deletion | Deleted messages remain as placeholders; body is cleared/redacted for normal users. | Separate moderation retention can be deferred. | Messages need deletion metadata and redacted display semantics. |
| Registration | Registration mode is configurable: `open`, `approval_required`, later `invite_only`. | Invite-only can be deferred. | Config and user status lifecycle. |
| Status | `pending`, `active`, `suspended`, `banned`, `deleted`. | No. | Auth/write/read checks and admin actions. |
| Identity | `user_name` is unique handle; `public_name` is display name. | Change-limit policy details can be deferred. | Unique handle, editable display name. |
| Packets | Readable first: server `id/type/state/data`, client `req/type/data`; compact later: `i/t/s/d`. | Compact mode can be deferred. | Packet registry must be stable before WebSocket coding. |

## 3. DM Rules

Direct messages are private. The server must enforce whether a user can start a direct conversation or send the first direct message based on the recipient's DM policy.

Supported `dm_policy` values:

- `everyone`: any authenticated active user may start a DM, subject to rate limits and abuse controls.
- `shared_group_members`: only users who share at least one group with the recipient may start a DM.
- `friends_only`: only accepted friends may start a DM.
- `none`: no new inbound DMs are allowed, except possible admin/system exceptions if later defined.

Rules:

- `dm_policy` is stored per user.
- Server enforcement is required before creating a direct conversation.
- Server enforcement is required before sending the first direct message if direct conversation creation and first message are separate actions.
- If friendships are not implemented initially, `friends_only` remains reserved but inactive or deferred.
- Existing direct conversations after policy changes need an explicit later rule; do not assume policy retroactively closes existing conversations unless defined.
- Rate limits still apply even when policy allows the DM.

Default policy is still open. Product owner should choose a default before implementation.

## 4. Community Rules

Communities are topic spaces for discussion content.

Rules:

- Start with public communities.
- Public community discussion is readable by all users who are allowed to access the site.
- Reading public community discussion does not require joining.
- Posting and replying require authentication.
- Posting and replying require the user to be allowed to write, so suspended/banned/deleted status rules still apply.
- Joining or following a community affects personal feed ranking/filtering and notifications, not basic read access.
- Private communities are deferred.
- Public community content is discussion content, not open realtime community chat.
- Community discussion posts must belong to a community.

Examples of communities:

- Programming
- Anime
- MMORPG
- Linux
- Philosophy

## 5. Combined Feed Rules

The product should not require a special General community.

Rules:

- Use a combined discussion feed across visible communities.
- The combined feed includes posts from all communities visible to the current viewer.
- For the initial public-community model, visible communities are public communities.
- If following/joining is added, the combined feed can support personal feed modes such as followed communities only.
- The combined feed should support simple ordering first.
- Later filters may include newest, popular, solved/open, community, author, or tag.
- A mandatory General community should not be created just to provide a global feed.

Practical implication: the feed is a query/view over discussion posts, not a standalone community.

## 6. Group Rules

Groups are private chat spaces.

Visibility:

- Users only see groups they are members of.
- Group names, metadata, member lists, and messages are hidden from non-members.
- Group messages are visible only to members.
- Realtime group delivery is only to eligible current members.

Roles:

- Start with two group roles: `owner` and `member`.
- Owner can add members.
- Owner can remove members.
- Member can leave.
- Rich roles such as moderator/admin can be added later.

Size:

- Group max size remains 10.
- Owner counts toward the max size.

History:

- New group members cannot see old messages.
- Every membership period needs a visibility boundary.
- Use `visible_from_message_id` or equivalent.
- If a user leaves and rejoins, assign a new visibility boundary.
- Rejoined users must not regain access to messages from before the rejoin point.
- Message history, search, export, and realtime resync must all enforce the boundary.

Membership lifecycle:

- Member leaves should mark the membership period ended rather than erase the historical row.
- Remove and re-add should behave like a new membership period.
- Invite acceptance is deferred.

## 7. Message Deletion Rules

Deleted messages remain visible as placeholders in history, but normal users cannot see deleted message content.

Rules:

- On user delete, prefer clearing/redacting the body:
  - `body = NULL` or equivalent.
  - `deleted_at` is set.
  - `deleted_by` is set.
- Message history returns a placeholder for deleted messages.
- Normal users see only the placeholder, not the original body.
- Realtime delete events should update clients to the placeholder state.
- Deleted messages should not disappear from ordering, pagination, or reply context unless a later rule says otherwise.
- Moderation/audit retention can be handled separately later.

Tradeoff:

- Redacting content improves user privacy and reduces exposure after deletion.
- Retaining content for moderation can help abuse investigation and legal/compliance review.
- The current rule prefers privacy for normal product behavior and defers separate audit-retention design.

## 8. Registration and User Status Rules

Registration mode must be configurable.

Supported registration modes:

- `open`: new users can become active without admin approval.
- `approval_required`: new users start as pending and require admin approval.
- `invite_only`: reserved for later.

User statuses:

- `pending`: account exists but cannot fully participate until approved.
- `active`: can log in, read, and write according to permissions.
- `suspended`: can log in and read but cannot write.
- `banned`: cannot log in.
- `deleted`: hidden/deactivated user state.

Rules:

- Suspended users can log in and read.
- Suspended users cannot write.
- Banned users cannot log in.
- Deleted users are hidden/deactivated.
- If a user is banned while sessions exist, active sessions should be treated as invalid by auth checks.
- If a user is suspended while sessions exist, active sessions may remain readable but writes must fail.
- Deleted users should not appear as active participants in people pickers or discovery surfaces.

## 9. User Identity Rules

User identity has two separate names:

- `user_name`: unique handle.
- `public_name`: display name.

Rules:

- `user_name` must be unique.
- `user_name` is the stable handle for mentions, URLs, lookup, and identity.
- `user_name` should be immutable or change-limited.
- `public_name` is editable.
- `public_name` is for display and should not be used as a stable identifier.
- If `user_name` changes are allowed later, they need history, cooldown, anti-impersonation, and mention/URL behavior rules.

Recommendation: treat `user_name` as immutable for the first rebuild unless the owner explicitly chooses a change-limited policy.

## 10. Packet Registry Rules

Packet protocol starts with readable mode:

Readable server packet:

```json
{
  "id": "evt_01J...",
  "type": "chat.message.created",
  "state": "ok",
  "data": {}
}
```

Readable client packet:

```json
{
  "req": "req_01J...",
  "type": "chat.message.send",
  "data": {}
}
```

Compact mode can come later:

```json
{
  "i": "evt_01J...",
  "t": 202,
  "s": 1,
  "d": []
}
```

Rules:

- Readable mode is the first development/debug protocol.
- Compact mode is not required for initial development.
- Compact mode requires a strict packet schema registry.
- Readable `data` is an object.
- Compact `d` is an array.
- Both must be generated from the same schema registry once compact mode exists.
- Stable packet registry should be finalized before WebSocket coding.

Preliminary packet id ranges:

| Range | Domain |
| --- | --- |
| 100-199 | auth/user |
| 200-299 | chat/direct |
| 300-399 | group |
| 400-499 | community |
| 500-599 | discussion |
| 600-699 | notification/presence |
| 900-999 | error/system |

Preliminary packet registry:

| Compact id | Readable type | Direction | Notes |
| --- | --- | --- | --- |
| 101 | `auth.login` | client/server reply | Login request or login result shape must be defined before implementation. |
| 102 | `auth.logout` | client/server reply | Logout current session. |
| 103 | `auth.me` | client/server reply | Current authenticated user/session state. |
| 201 | `chat.message.send` | client | Send direct/private message command. |
| 202 | `chat.message.created` | server | Message created event/reply. |
| 203 | `chat.message.deleted` | server | Message redacted/deleted event. |
| 301 | `group.created` | server | Group created event/reply. |
| 302 | `group.member.added` | server | Member added with visibility boundary. |
| 303 | `group.member.removed` | server | Member removed or left. |
| 401 | `community.created` | server | Community created by admin. |
| 501 | `discussion.post.created` | server | Top-level community discussion post created. |
| 502 | `discussion.reply.created` | server | Discussion reply created. |
| 503 | `discussion.vote.created` | server | Discussion vote created or changed. |
| 601 | `presence.changed` | server | User/session presence change. |
| 901 | `error.validation` | server | Validation failure. |
| 902 | `error.auth` | server | Authentication failure. |
| 903 | `error.forbidden` | server | Authenticated but not allowed. |
| 904 | `error.not_found` | server | Resource not found or intentionally hidden. |
| 905 | `error.rate_limited` | server | Rate limit failure. |

These IDs are preliminary and not implementation yet.

## 11. Rules That Affect Database Design

- Users need `user_name`, `public_name`, status, registration mode effects, and `dm_policy`.
- `user_name` needs uniqueness and likely immutability or change-history support if changes are allowed.
- User status needs at least `pending`, `active`, `suspended`, `banned`, and `deleted`.
- Direct conversations need participant records and first-message/creation policy enforcement.
- Groups need private membership rows with role: `owner` or `member`.
- Groups need max size enforcement around membership count.
- Group memberships need lifecycle fields, such as joined/left/removed timestamps and active/inactive state.
- Group memberships need a visibility boundary, preferably `visible_from_message_id`.
- Rejoin creates a new membership period and new visibility boundary.
- Group messages need query support by group and by membership visibility boundary.
- Message rows need deletion metadata: body nullable/redacted, `deleted_at`, `deleted_by`.
- Audit/moderation retention, if added, should be separate from normal message body visibility.
- Communities need visibility state, starting with public.
- Discussion posts need `community_id`.
- Combined feed queries need to aggregate posts across visible communities.
- Community follow/join state, if added, affects feed/preferences, not public read access.
- Packet registry needs a durable schema source before WebSocket coding; it does not necessarily need a runtime database table.

## 12. Rules That Affect Backend Services

- DM service must enforce recipient `dm_policy` before creating a direct conversation or accepting the first message.
- User service must enforce registration mode and status behavior.
- Auth service must prevent banned users from logging in.
- Write guards must block suspended, banned, and deleted users from writes.
- Group service must enforce membership visibility for every group read and write.
- Group service must enforce owner-only add/remove rules.
- Group service must enforce max size 10.
- Group history service must filter by visibility boundary.
- Realtime delivery must not send group messages to non-members or to members before their visibility boundary.
- Message service must redact deleted bodies for normal users.
- Community service starts with admin-only community creation.
- Discussion service must require a community for each post.
- Feed service should produce combined visible-community discussion feeds.
- Packet handling must route by readable `type` or compact `t`, never by payload guessing.
- Error service should produce distinct validation/auth/forbidden/not_found/rate_limited packets.

## 13. Rules That Affect Frontend UX

- DM settings should eventually expose `dm_policy`.
- If `friends_only` is unavailable because friendships are deferred, the UI should not offer it as an active choice.
- Community pages should feel like topic discussion spaces, not chat rooms.
- Public community discussion can be read without a join action.
- Join/follow should be framed as personalization and notifications, not access.
- The combined feed should be visible without inventing a General community.
- Group list should show only groups where the user is a member.
- Group UI should show owner/member capabilities simply.
- Owner can manage members; members can leave.
- New group members should see history starting at their join boundary.
- The UI may show a "You joined here" divider if boundary data is available.
- Deleted messages should render as placeholders.
- Suspended users should see clear write-disabled states.
- Banned users should be redirected or denied at auth.
- Deleted users should be hidden/deactivated rather than shown as normal accounts.
- Frontend packet handlers should route by `type` in readable mode and by registry-mapped `t` in compact mode.
- Readable mode should remain available for debugging.

## 14. Deferred Rules

- Friendships implementation and the active behavior of `friends_only`.
- Invite-only registration.
- Private communities.
- Community owner/moderator roles.
- Community-level moderation depth beyond admin-created communities.
- Community join/follow notification details.
- Group invite acceptance.
- Group moderator/admin roles beyond owner/member.
- Existing direct conversation behavior after a recipient changes `dm_policy`.
- Message edit rules and edit history.
- Moderation/audit retention of deleted message content.
- Media upload rules.
- Search rules.
- Compact packet generation, negotiation, and versioning.
- Full packet payload schemas and compact `d` field ordering.

## 15. Open Questions

- What is the default `dm_policy` for new users?
- Should existing direct conversations remain usable if a user later changes `dm_policy` to a stricter value?
- Can admins bypass `dm_policy` for support/moderation contact?
- Is the initial registration mode `open` or `approval_required`?
- What exact behavior should `pending` users have: can they log in to a waiting screen, or not log in at all?
- Should `deleted` users be anonymized in old messages and discussion posts?
- Should users be allowed to change `user_name` at all?
- If `user_name` changes are allowed, what cooldown and history rules apply?
- What is the initial default set of public communities?
- Are all authenticated users allowed to post in all public communities?
- Do community follows affect only feed ordering, or also notification defaults?
- Should discussion votes be upvote-only, up/down, or reaction-like?
- Should group owner transfer be supported?
- What happens if the only group owner leaves?
- Are removed group members allowed to see messages from their previous membership period after removal, or should removal end all future access while preserving only local cache?
- Is `visible_from_message_id` enough, or should group conversations use a monotonic sequence number?
- Should packet compact ids be considered permanent once published?
- Should REST include compact ids in readable responses for easier debugging/transition?

## 16. Recommended Next Document

Create `docs/domain-model-boundaries.md` next.

That document should translate these product rules into framework-neutral domain boundaries and invariants for:

- User and status lifecycle.
- User identity and DM policy.
- Direct conversation access.
- Group membership, roles, and visibility boundaries.
- Community visibility and discussion ownership.
- Combined feed visibility.
- Message deletion/redaction.
- Packet registry ownership.
