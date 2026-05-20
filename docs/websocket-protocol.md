# WebSocket Protocol

## 1. Executive Summary

Novastrum should use WebSocket for authenticated realtime packets: live chat delivery, command acknowledgements, presence changes, and later notifications. REST remains responsible for bootstrap, CRUD, history, settings, pagination, and reconnect recovery.

The protocol starts with readable JSON packets. Compact packets are planned later, but compact mode must be generated from the same packet registry rather than maintained by hand.

The database is the source of truth. `WsHub` owns live connections and fanout only. `WsHub` must not write to the database and must not become the authority for membership, message history, presence history, unread state, or authorization.

## 2. WebSocket Responsibilities

WebSocket handles:

- Realtime delivery of committed events.
- Client commands that need live response, such as sending a chat message.
- Presence events.
- Delete/update events for already visible realtime state.
- Notifications later, once notification event rules are concrete.

WebSocket does not handle:

- Initial app bootstrap.
- Login/logout as the primary auth path.
- Full history retrieval.
- Pagination.
- Settings forms.
- Admin CRUD.
- Reconnect recovery as an in-memory replay stream.

## 3. What REST Handles vs What WebSocket Handles

| Area | REST | WebSocket |
| --- | --- | --- |
| Authentication bootstrap | Login, logout, current user, session cookie. | Authenticates upgrade using existing session cookie. |
| App bootstrap | Current user, initial lists, settings. | Not responsible. |
| Message history | Cursor-paginated history. | Realtime message events after connection. |
| Send message | May support REST send. | Preferred live command path. |
| Group membership | CRUD/action endpoints. | Realtime membership events after committed changes. |
| Community/discussion | CRUD, feed, lists, pagination. | Optional live discussion events later. |
| Presence | Last seen can be read from REST later. | Live presence changes. |
| Notifications | Later list/read endpoints. | Later live notification delivery. |
| Reconnect recovery | Bootstrap/history/resync queries. | Reconnect transport only; no first-version event replay. |

## 4. Authentication

The WebSocket endpoint should authenticate with the same session-cookie model as REST. The upgrade request must validate the session cookie before registering the connection.

Rules:

- Reject unauthenticated connections.
- Reject banned users.
- Reject deleted users.
- Allow suspended users to connect and read packets they are authorized to receive.
- Suspended users cannot send write commands.
- Pending user behavior remains open and should follow the final product rule chosen for REST.
- If a user becomes banned or deleted while connected, the backend should close or invalidate the connection.
- If a user becomes suspended while connected, the backend should reject later write commands even if the socket remains open.

The client must never send `user_id` as an authority field. The authenticated session determines the actor.

## 5. Connection Lifecycle

Expected lifecycle:

1. Client opens WebSocket connection, likely `/ws`.
2. Server authenticates the session cookie during upgrade or immediately after upgrade.
3. Server rejects unauthenticated, banned, or deleted users.
4. Server registers the connection in `WsHub`.
5. Server may emit an initial lightweight connection-ready packet.
6. Client sends readable command packets.
7. Server validates packet shape and dispatches by `type`.
8. Services perform authorization, transactions, and database writes.
9. Server sends replies and committed events.
10. Connection uses ping/pong or heartbeat handling.
11. Disconnect removes the connection from `WsHub`.
12. `last_seen_at` is updated with throttling, not on every packet.

Multiple connections per user must be allowed. A user may have browser tabs, mobile sessions, or reconnect overlap. `WsHub` should route by user id and connection id.

## 6. Readable Packet Format

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

Fields:

| Field | Direction | Meaning |
| --- | --- | --- |
| `id` | Server to client | Unique server packet id for dedupe and debugging. |
| `req` | Client to server | Client-generated request id. |
| `reply_to` | Server to client | Original client request id when a packet replies to a command. |
| `type` | Both | Readable packet type, such as `chat.message.send`. |
| `state` | Server to client | `ok` or `error`. |
| `data` | Both | Readable object payload. |
| `sent_at` | Optional server field | Server timestamp for ordering/debugging. |
| `version` | Optional initially | Protocol version if versioning is made per-packet. |

Recommendation: start with readable object payloads and make `type` mandatory. Decide whether `version` is connection-negotiated or included on every packet before implementation.

## 7. Client Command Format

Client command packets should use:

```json
{
  "req": "req_01J...",
  "type": "chat.message.send",
  "data": {
    "conversation_id": "cnv_01J...",
    "body": "Hello"
  }
}
```

Rules:

- `req` should be generated by the client.
- `req` must be unique enough within the client session to correlate replies.
- `type` is mandatory.
- `data` must be an object in readable mode.
- Client packets must not include authoritative `sender_id`, `user_id`, role, or membership fields.
- Malformed packets should receive an error packet when possible, then may be disconnected if abusive.

## 8. Server Event Format

Server-originated event:

```json
{
  "id": "evt_01J...",
  "type": "chat.message.created",
  "state": "ok",
  "data": {
    "message_id": "msg_01J...",
    "conversation_id": "cnv_01J...",
    "sender_id": "usr_01J...",
    "body": "Hello",
    "created_at": "2026-05-20T12:00:00.000000Z"
  }
}
```

Server reply to client command:

```json
{
  "id": "res_01J...",
  "reply_to": "req_01J...",
  "type": "chat.message.created",
  "state": "ok",
  "data": {
    "message_id": "msg_01J..."
  }
}
```

Error reply:

```json
{
  "id": "err_01J...",
  "reply_to": "req_01J...",
  "type": "error.validation",
  "state": "error",
  "data": {
    "code": "validation_failed",
    "message": "Validation failed",
    "fields": {
      "body": "required"
    }
  }
}
```

Recommendation: use `reply_to` in server packets for acknowledgements and command errors. Server-originated events should omit `reply_to`.

## 9. Request/Reply Correlation

Request/reply rules:

- Client sends `req`.
- Server validates and processes the command.
- Server response includes `reply_to` with the original `req`.
- Client keeps a pending command map by `req`.
- Server may also broadcast a separate event with its own `id`.

Using `reply_to` avoids overloading `req` as both a client field and a server reply field. The exact naming remains open, but `reply_to` is the current recommendation for readable WebSocket server packets.

## 10. Packet Registry

Readable type strings are the source of truth initially. Compact numeric ids are reserved but not required for the first implementation.

Preliminary registry:

| Compact id | Readable type |
| --- | --- |
| `101` | `auth.login` |
| `102` | `auth.logout` |
| `103` | `auth.me` |
| `201` | `chat.message.send` |
| `202` | `chat.message.created` |
| `203` | `chat.message.deleted` |
| `301` | `group.created` |
| `302` | `group.member.added` |
| `303` | `group.member.removed` |
| `401` | `community.created` |
| `501` | `discussion.post.created` |
| `502` | `discussion.reply.created` |
| `503` | `discussion.vote.created` |
| `601` | `presence.changed` |
| `901` | `error.validation` |
| `902` | `error.auth` |
| `903` | `error.forbidden` |
| `904` | `error.not_found` |
| `905` | `error.rate_limited` |

Registry rules:

- Packet registry must exist before coding WebSocket command handling.
- Every command, reply, event, and error needs a registered readable `type`.
- Compact ids are reserved for later.
- Compact payload ordering must be defined by registry schema.
- Registry drift is a real risk; avoid separate manually maintained readable and compact definitions.

## 11. Chat Message Flow

Chat message send flow:

1. Client sends `chat.message.send`.
2. WebSocket handler validates packet envelope and payload shape.
3. Auth/session and user status are checked.
4. Suspended users are rejected for write commands.
5. `ChatService` validates conversation visibility, membership, DM policy, and group history rules as needed.
6. `ChatRepository` inserts the message inside the service transaction.
7. Database commit succeeds.
8. Server sends an acknowledgement packet to the sender with `reply_to`.
9. `WsHub` broadcasts `chat.message.created` to eligible connected recipients.
10. Offline users fetch missed messages later through REST message history.

DB write must happen before broadcast. A message should not be broadcast as created until the database commit succeeds.

Direct message rules:

- Only conversation participants receive direct message events.
- DM policy is enforced before direct conversation creation or first send.
- Existing direct conversation behavior after later policy changes still needs a final rule.

Group message rules:

- Only active members receive group message events.
- New or rejoined members cannot receive historical events before their visibility boundary.
- Realtime fanout should still rely on service-computed eligibility, not client claims.

## 12. Group Event Flow

Group member added:

1. Owner action arrives through REST or WebSocket command if later allowed.
2. Service validates owner role, max group size, target user status, and group visibility.
3. Service creates a new membership lifecycle row.
4. Service assigns `visible_from_message_id` or equivalent join boundary.
5. After commit, `WsHub` delivers `group.member.added` only to eligible current members.

Group member removed:

1. Owner removal or member leave is processed by the service.
2. Service marks membership as `left` or `removed`.
3. After commit, `WsHub` delivers `group.member.removed` only to members who are allowed to know about the group.
4. Removed users should stop receiving future group events.

Visibility implications:

- Group events must not leak group existence to non-members.
- A newly added user can learn the group exists at join time.
- A removed user may need a final removal event so the client can remove the group locally.
- A rejoined user receives a new visibility boundary and does not regain access to old messages.

## 13. Discussion Event Flow

Discussion event types include:

- `discussion.post.created`
- `discussion.reply.created`
- `discussion.vote.created`

Discussion events may later be broadcast to followers, viewers currently on the relevant community/post, or other interested clients. This fanout policy is not finalized.

REST remains the source for discussion feed, post detail, reply history, pagination, and missed updates. WebSocket discussion events are a live enhancement, not the canonical feed.

## 14. Presence Flow

Presence event type:

- `presence.changed`

Rules:

- Online presence is memory-first through `WsHub`.
- Multiple connections per user should collapse into one user-level online state for most UI.
- A user is online while at least one active connection exists.
- `last_seen_at` can be persisted with throttling.
- Do not update `last_seen_at` on every packet.
- Presence visibility rules are open and must be decided before broad presence broadcasting.

Example:

```json
{
  "id": "evt_01J...",
  "type": "presence.changed",
  "state": "ok",
  "data": {
    "user_id": "usr_01J...",
    "presence": "online",
    "last_seen_at": null
  }
}
```

## 15. Error Packets

Validation error:

```json
{
  "id": "err_01J...",
  "reply_to": "req_01J...",
  "type": "error.validation",
  "state": "error",
  "data": {
    "code": "validation_failed",
    "message": "Validation failed",
    "fields": {
      "body": "too_long"
    }
  }
}
```

Auth required:

```json
{
  "id": "err_01J...",
  "reply_to": "req_01J...",
  "type": "error.auth",
  "state": "error",
  "data": {
    "code": "auth_required",
    "message": "Authentication is required."
  }
}
```

Forbidden:

```json
{
  "id": "err_01J...",
  "reply_to": "req_01J...",
  "type": "error.forbidden",
  "state": "error",
  "data": {
    "code": "forbidden",
    "message": "You cannot perform this action."
  }
}
```

Not found:

```json
{
  "id": "err_01J...",
  "reply_to": "req_01J...",
  "type": "error.not_found",
  "state": "error",
  "data": {
    "code": "not_found",
    "message": "Resource not found."
  }
}
```

Rate limited:

```json
{
  "id": "err_01J...",
  "reply_to": "req_01J...",
  "type": "error.rate_limited",
  "state": "error",
  "data": {
    "code": "rate_limited",
    "message": "Too many requests.",
    "retry_after_ms": 5000
  }
}
```

Malformed packet:

```json
{
  "id": "err_01J...",
  "reply_to": "req_01J...",
  "type": "error.validation",
  "state": "error",
  "data": {
    "code": "malformed_packet",
    "message": "Packet shape is invalid."
  }
}
```

For private resources, prefer `not_found` when confirming existence would leak direct or group data.

## 16. Reconnect and Resync

Rules:

- WebSocket is not the source of truth.
- No event replay is required in the first implementation.
- After reconnect, the client should call REST bootstrap/history endpoints.
- Missed direct/group messages are fetched from REST message history.
- Missed discussion feed changes are fetched from REST feed/list endpoints.
- Packet `id` helps frontend duplicate handling.
- Domain ids such as `message_id` and `post_id` help state reconciliation.
- Group resync must enforce `visible_from_message_id`.

Recommended reconnect sequence:

1. Socket disconnects.
2. Client marks realtime state as disconnected.
3. Client reconnects with backoff.
4. Server authenticates the new socket.
5. Client calls REST bootstrap or targeted history endpoints using known cursors/last ids.
6. Client reconciles local state and dedupes by packet id/domain id.

## 17. Idempotency

Idempotency options:

- Client `req` helps correlate a command and identify duplicate in-flight sends from the same connection.
- Server packet `id` helps frontend dedupe repeated events.
- Domain ids help dedupe committed records.
- Message send idempotency can be deferred.
- Later, add `client_message_id` for message send idempotency across reconnects/retries.

Tradeoff:

- Deferring `client_message_id` keeps the first protocol simpler but may allow duplicate messages if the client retries after a lost acknowledgement.
- Adding `client_message_id` early improves retry safety but requires database uniqueness rules and extra frontend state.

Recommendation: document `client_message_id` as a likely addition before serious offline/retry behavior, but do not require it for the first protocol slice unless duplicate send prevention is a product priority.

## 18. Compact Mode

Compact mode is optional later. Readable mode comes first.

Compact server packet:

```json
{
  "i": "evt_01J...",
  "t": 202,
  "s": 1,
  "d": []
}
```

Compact client packet:

```json
{
  "r": "req_01J...",
  "t": 201,
  "d": []
}
```

Compact field meanings:

| Field | Meaning |
| --- | --- |
| `i` | Server packet id. |
| `r` | Client request id or reply correlation id, depending on direction. |
| `t` | Numeric packet type id. |
| `s` | State, `1` for ok and `0` for error. |
| `d` | Compact array payload. |

Rules:

- Compact `d` arrays require strict schema registry ordering.
- Compact packets should be generated from the registry.
- Do not hand-maintain separate compact payload mappings.
- Compact mode should target WebSocket before REST.
- Compact mode negotiation is still open.

## 19. Security Rules

Required security rules:

- Enforce a maximum packet size.
- Enforce allowed packet types per user status.
- Rate-limit write commands.
- Rate-limit malformed packets and disconnect abusive clients.
- Never trust client-provided `user_id`, `sender_id`, role, or membership claims.
- Validate direct/group/community/discussion authorization server-side.
- Validate group membership and visibility boundary server-side.
- Do not leak private group or direct packet data to unauthorized users.
- Prefer `not_found` over `forbidden` when resource existence is private.
- Treat WebSocket as untrusted input exactly like REST.
- Keep database writes inside services/repositories, not `WsHub`.

## 20. Frontend Client Expectations

Frontend should:

- Connect after REST auth/bootstrap.
- Reconnect with exponential backoff or capped backoff.
- Dispatch packets by readable `type`.
- Deduplicate server events by `id`.
- Reconcile domain state by public ids such as `message_id`.
- Maintain a pending command map by `req`.
- Resolve or reject pending commands when a packet with matching `reply_to` arrives.
- Fall back to REST resync after reconnect.
- Include a readable mode debugger/logging view during development.
- Avoid depending on compact packet mode for initial functionality.

## 21. Open Questions

1. What is the default protocol version?
2. Is `version` required on every packet, or negotiated once per connection?
3. Is `reply_to` the final server reply correlation field, or should the protocol use `req` in both directions?
4. Should `client_message_id` be required for `chat.message.send` idempotency?
5. How should compact mode negotiation work?
6. What are the presence visibility rules?
7. Should discussion events broadcast globally, to followers only, or only to clients currently viewing the related community/post?
8. Should there be a connection-ready packet after authentication?
9. What is the maximum packet size?
10. Which write command rate limits apply per user, per session, and per IP?

## 22. Recommended Next Document

Create `docs/build-plan.md` next. It should turn the architecture, database, REST contract, and WebSocket protocol into a practical build sequence without starting implementation.
