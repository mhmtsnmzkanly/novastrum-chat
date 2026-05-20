# Product Decisions

This document records current owner decisions for the rebuild. It supersedes conflicting legacy assumptions where noted, but it does not define implementation details or framework choices.

## 1. Summary

Novastrum should separate private messaging from public/community discussion:

- Direct messages are private.
- Group chats are private to members.
- Communities are Reddit-like topic spaces.
- Discussion posts belong to communities.
- Publicly visible community-facing content is discussion content, not open community chat.
- Open public community chat is not required by default.
- Groups are not public discovery spaces.
- New group members must not see messages from before they joined.
- Packet protocol should be compact-capable, but readable packets should be the first development/debug format.

These decisions narrow the rebuild and reduce scope risk. The product should not start by building every legacy surface at once.

## 2. Finalized Decisions

- Users only see groups where they are members.
- Group chats are private to group members.
- Group messages are visible only to group members.
- New group members must not see old group messages.
- Group membership must include a join-boundary concept such as `visible_from_message_id` or an equivalent marker.
- Direct messages are private.
- Community discussion is the public/community-facing layer.
- Communities are topic spaces such as Programming, Anime, MMORPG, Linux, and Philosophy.
- Discussion posts belong to communities.
- Public community content should be discussion content, not open community chat.
- A global `General` community is not mandatory.
- If a global feed is needed, prefer a combined feed from all visible communities instead of a special mandatory General community.
- The compact packet-style protocol direction is accepted as a product/API preference.
- Packet envelopes need a packet/action type field. `id/state/data` and `req/data` alone are not enough.
- Readable mode should use object payloads for development, debugging, and frontend integration.
- Compact mode should use short field names and array payloads only when backed by a strict schema registry.

## 3. Tentative Decisions

- Use the packet-style envelope vocabulary for both REST-inspired responses and WebSocket packets, without blindly preserving the legacy `ApiEnvelope`.
- REST can use a readable packet-inspired response shape.
- WebSocket should use packet envelopes directly.
- Compact mode should target WebSocket before REST.
- Packet type ranges should be reserved by domain, for example `100 auth.*`, `200 chat.*`, `300 group.*`, `400 community.*`, `500 discussion.*`, `600 notification.*`, and `900 error.*`.
- Group join boundaries should preferably be represented by `visible_from_message_id`, but a timestamp or sequence boundary may be acceptable if it is easier to enforce correctly.
- Communities may have membership, following, visibility, or moderation rules later, but that is not decided yet.

## 4. Deferred Decisions

- Whether any community should have realtime chat at all.
- Whether users join/follow communities or simply browse visible communities.
- Whether communities are public to all authenticated users, invite-only, moderated, or mixed.
- Whether a combined community feed is required.
- Whether direct messaging requires accepted friendship or another trust model.
- Whether groups have owners, admins, moderators, or only members.
- Whether group invitations require recipient acceptance.
- Whether group messages use hard deletion, soft deletion, audit retention, or a mixed policy.
- Whether packet compact mode is hand-authored, generated, or negotiated at connection time.
- Whether REST responses include numeric type ids or only readable type strings.

## 5. Group Visibility Rules

- A group is visible only to its members.
- Groups are not public discovery spaces by default.
- A user should not see group names, member lists, messages, or metadata for groups they do not belong to.
- Group list queries must be scoped to the current user.
- Group detail queries must enforce membership.
- Group message queries must enforce membership and the member's history boundary.
- Group realtime delivery must be scoped to active members who are allowed to see the delivered message.

Implication: group discovery/search should not be built until the owner explicitly decides how private groups can be found or invited into.

## 6. Group History Rules

- New group members must not see old group messages.
- A member's visible history starts at the point they joined.
- The design should support a join-boundary marker, preferably `visible_from_message_id` or an equivalent.
- The boundary must be enforced in backend reads and realtime catch-up/resync.
- Removing and re-adding a member should create a new visibility boundary unless the owner decides otherwise.

Open enforcement models:

- `visible_from_message_id`: precise, stable against clock issues, and aligns well with ordered message ids.
- `visible_from_created_at`: simpler, but fragile if clocks, ordering, or same-timestamp messages are mishandled.
- `visible_from_sequence`: strong if conversations have monotonic sequence numbers.

Current recommendation: use a message id or conversation sequence boundary rather than relying only on timestamps.

## 7. Community and Discussion Rules

- Communities are topic spaces, not chat rooms by default.
- Discussion posts belong to a community.
- Public/community-facing content is discussion content.
- Open community chat should not be assumed.
- A global `General` community is optional, not mandatory.
- A global feed, if wanted, should combine posts from visible communities rather than force all users into a special General space.

Example communities:

- Programming
- Anime
- MMORPG
- Linux
- Philosophy

Implication: the product map should treat `Community` and `Discussion` as tightly related. Community provides topic/container identity; discussion provides the public content surface.

## 8. Direct/Group/Community Boundary

Direct/private chat:

- Private conversation between users.
- Visibility is limited to participants.
- Trust model is still open: accepted friendship, accepted DM request, or another rule.

Group chat:

- Private conversation among members.
- Visibility is limited to members.
- History is limited by each member's join boundary.
- Groups are not public spaces.

Community discussion:

- Public or semi-public topic-oriented content.
- Organized by community.
- Visible according to community visibility rules, not group membership rules.
- Should not be modeled as open chat unless explicitly added later.

Boundary principle: do not mix private chat semantics and public discussion semantics. They have different visibility, history, moderation, notification, and discovery rules.

## 9. Packet Protocol Decision

The owner prefers a compact packet-style protocol.

Initial owner idea:

```json
{
  "id": "packet_id",
  "state": "ok|error",
  "data": []
}
```

```json
{
  "req": "request_id",
  "data": []
}
```

Current decision: keep the compact packet direction, but add a required packet/action type field. Without `type`, clients cannot know how to decode `data`, route actions, validate payloads, correlate event semantics, or evolve schemas safely.

Readable mode should look like:

```json
{
  "id": "evt_01J...",
  "type": "chat.message.created",
  "state": "ok",
  "data": {
    "message_id": "msg_01J...",
    "conversation_id": "cnv_01J...",
    "sender_id": "usr_01J...",
    "body": "Hello"
  }
}
```

Compact mode can later map the same schema to:

```json
{
  "i": "evt_01J...",
  "t": 202,
  "s": 1,
  "d": ["msg_01J...", "cnv_01J...", "usr_01J...", "Hello"]
}
```

Readable and compact modes must be generated from the same packet schema registry.

## 10. Remaining Product Questions

- What trust rule allows direct messages: accepted friendship, accepted DM request, or open DM with abuse controls?
- Are communities visible to all authenticated users by default?
- Can communities be private or moderated?
- Do users join communities, follow them, or simply browse them?
- Is a combined community feed required for the first rebuild?
- Who can create communities?
- Who can moderate communities and discussion posts?
- Are community discussion replies nested, flat, or both?
- Do groups require invite acceptance?
- Can group members leave voluntarily?
- What happens to a user's visibility if they leave and later rejoin a group?
- Do group owners/admins exist?
- Should deleted group/private messages remain visible to admins for audit?
- Which packet mode is default in development: readable only, readable with opt-in compact, or negotiated?
- Should REST responses include packet type ids or only type strings?
- What minimum packet registry must be stable before implementation starts?

## 11. Architecture Impact

- Private chat and public/community discussion should be separate product domains.
- Community should not automatically imply a chat conversation.
- Group access checks are stricter than generic conversation access checks because membership and join-boundary rules must both apply.
- Realtime delivery must respect group membership and message visibility boundaries.
- Reconnect/resync must not leak group messages from before a member joined.
- Packet schemas should be defined before client/server implementation to avoid ad hoc envelopes.
- Protocol mode should be explicit: readable for development/debugging, compact for optimized transport.
- Legacy `ApiEnvelope` is useful context but should not be preserved blindly.

## 12. Database Design Impact

- Groups need a membership table with at least user id, group id, joined_at, membership status, and a join-boundary marker.
- Prefer `visible_from_message_id` or conversation sequence boundary over timestamp-only history rules.
- Group message queries need to filter by membership and join boundary.
- Community records should represent topic spaces.
- Discussion posts should reference a community id.
- Public feed queries should be able to combine posts from all visible communities.
- Chat conversations should distinguish direct and private group conversations from community discussion containers.
- Packet registry likely needs a durable source of truth in docs or schema files; it may not need a runtime database table, but the mapping must be stable and versioned.

## 13. Frontend Design Impact

- Navigation should not assume a public community chat tab.
- Communities should be presented as topic spaces with discussion feeds/posts.
- Direct and group chat should be presented as private messaging surfaces.
- Group member additions should not show prior messages; the UI should naturally start history at the member's join boundary.
- If a boundary marker is exposed, the frontend may show a "You joined here" divider.
- Readable packets should be used for development tools, browser debugging, and early frontend integration.
- Compact packets should not be the only available format during early rebuild work.
- Frontend packet handlers should route by `type`, not by guessing from payload shape.

## 14. Recommended Next Document

Create `docs/domain-model-boundaries.md` next.

That document should define the product-domain entities and relationships before implementation:

- User
- Session
- Direct conversation
- Group
- Group membership and join boundary
- Community
- Discussion post/reply
- Message
- Notification
- Packet schema registry

It should stay framework-neutral and focus on visibility, ownership, lifecycle, and invariants.
