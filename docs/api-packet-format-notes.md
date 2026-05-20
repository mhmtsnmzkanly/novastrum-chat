# API Packet Format Notes

## 1. Goal

Define a packet-style API and realtime envelope direction for Novastrum that supports:

- Clear routing by packet type.
- Readable development/debug payloads.
- Compact transport payloads later.
- Shared schema definitions across REST-inspired responses and WebSocket events.
- Reconnect, request/reply correlation, validation errors, and idempotency.

This is a protocol design note only. It does not choose a framework or implementation.

## 2. Owner Packet Idea

Owner's initial server packet idea:

```json
{
  "id": "packet_id",
  "state": "ok|error",
  "data": []
}
```

Owner's initial client packet idea:

```json
{
  "req": "request_id",
  "data": []
}
```

This captures the desired compact style: a small envelope, an id/request identifier, state, and data.

The direction is good, but the shape is missing one field that should be treated as mandatory: packet type.

## 3. Problem With Missing Type Field

Without a packet/action type field:

- The receiver cannot know which schema should decode `data`.
- Arrays become ambiguous because position only has meaning inside a known schema.
- Client handlers must infer intent from payload shape, which is fragile.
- Error packets cannot be categorized cleanly.
- Request/reply packets cannot distinguish login, send-message, validation, or notification payloads.
- Versioning becomes difficult because old and new payloads may look similar.
- Debug logs become hard to read.
- Authorization and rate-limit handling become harder to audit.

Recommendation: every client request, server reply, and server event should include a type field in readable mode or a numeric type id in compact mode.

## 4. Recommended Readable Server Packet

Readable server packet:

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

Field notes:

- `id`: unique packet/event/reply id.
- `type`: readable packet type string.
- `state`: `ok` or `error`.
- `data`: object payload in readable mode.

Server packets can represent replies to client requests or server-originated events. If the packet is a reply, include request correlation as described later.

## 5. Recommended Readable Client Packet

Readable client packet:

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

Field notes:

- `req`: client-generated request id for correlation and retries.
- `type`: requested action.
- `data`: object payload in readable mode.

The client should not send sender id for actions where the authenticated session already determines the actor.

## 6. Error Packet

Readable generic error packet:

```json
{
  "id": "err_01J...",
  "req": "req_01J...",
  "type": "error.auth",
  "state": "error",
  "data": {
    "code": "not_authenticated",
    "message": "Authentication is required."
  }
}
```

Notes:

- `req` is present when the error replies to a client request.
- Server-originated errors may omit `req`.
- `message` is safe user-facing text or a safe developer/debug message depending on environment.
- Sensitive internals should not be exposed in client packets.

## 7. Validation Error Packet

Readable validation error packet:

```json
{
  "id": "err_01J...",
  "req": "req_01J...",
  "type": "error.validation",
  "state": "error",
  "data": {
    "code": "validation_failed",
    "fields": [
      {
        "path": "body",
        "code": "too_long",
        "message": "Message body is too long."
      }
    ]
  }
}
```

Validation errors should be structured enough for the frontend to place errors near fields, not just show a generic toast.

## 8. REST Response Format

REST can use a readable packet-inspired response format:

```json
{
  "id": "res_01J...",
  "req": "req_01J...",
  "type": "chat.message.created",
  "state": "ok",
  "data": {
    "message_id": "msg_01J..."
  }
}
```

Recommendations:

- REST should use readable mode by default.
- REST does not need compact array payloads at the start.
- REST may include `req` if the client supplies a request id header or body field, but this is optional.
- HTTP status should still carry transport-level success/failure.
- Packet `state` should carry application-level success/failure.
- Do not blindly preserve legacy `ApiEnvelope`; keep the useful idea of consistent responses but use packet vocabulary if chosen.

## 9. WebSocket Packet Format

WebSocket should use packet envelopes for both directions.

Readable client-to-server:

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

Readable server-to-client reply:

```json
{
  "id": "res_01J...",
  "req": "req_01J...",
  "type": "chat.message.created",
  "state": "ok",
  "data": {
    "message_id": "msg_01J..."
  }
}
```

Readable server-to-client event:

```json
{
  "id": "evt_01J...",
  "type": "chat.message.created",
  "state": "ok",
  "data": {
    "message_id": "msg_01J...",
    "conversation_id": "cnv_01J..."
  }
}
```

Recommendations:

- WebSocket should be the first target for compact mode.
- Client handlers should route by `type`.
- Server events must be idempotent by `id` or by stable domain object id plus version.

## 10. Compact Packet Mode

Compact mode example:

```json
{
  "i": "evt_01J...",
  "t": 202,
  "s": 1,
  "d": ["msg_01J...", "cnv_01J...", "usr_01J...", "Hello"]
}
```

Where:

- `i` = packet id.
- `r` = request id, when a reply correlates to a client request.
- `t` = numeric packet type id.
- `s` = state, `1` for ok and `0` for error.
- `d` = compact array payload.

Client compact request example:

```json
{
  "r": "req_01J...",
  "t": 201,
  "d": ["cnv_01J...", "Hello"]
}
```

Compact mode must not be the only development/debug format at the start.

## 11. Packet Compression Strategy

Use two protocol modes:

A. readable mode

```json
{
  "id": "evt_01J...",
  "type": "chat.message.created",
  "state": "ok",
  "data": {}
}
```

B. compact mode

```json
{
  "i": "evt_01J...",
  "t": 202,
  "s": 1,
  "d": []
}
```

Strategy:

- Start with readable mode for all development and early integration.
- Define packet schemas in a registry before compact mode.
- Generate compact encoder/decoder mappings from the same registry used for readable mode.
- Target WebSocket compact mode first because it benefits most from repeated realtime packets.
- Keep readable mode available for debugging even after compact mode exists.
- Do not hand-maintain separate readable and compact schemas.

## 12. Data Object vs Data Array

Data as object:

- Safer for development.
- Easier to debug in browser tools and logs.
- More resilient to adding optional fields.
- Easier for frontend integration.
- Better for validation errors and partial payloads.
- Slightly larger over the wire.

Data as array:

- Smaller over the wire.
- Good for high-volume realtime packets.
- Fragile without strict field order.
- Harder to debug.
- Harder to evolve safely.
- Dangerous if client and server schemas drift.

Recommendation:

- Readable mode uses `data` as an object.
- Compact mode uses `d` as an array.
- Both forms must be generated from the same packet schema registry.
- Array payloads require strict ordering, versioning, and compatibility rules.

## 13. Packet Type Registry

Packet types should be registered in a single schema registry. The registry can start as documentation and later become generated schema metadata.

Suggested numeric ranges:

| Range | Domain |
| --- | --- |
| 100-199 | `auth.*` |
| 200-299 | `chat.*` |
| 300-399 | `group.*` |
| 400-499 | `community.*` |
| 500-599 | `discussion.*` |
| 600-699 | `notification.*` |
| 700-799 | `presence.*` |
| 800-899 | `admin.*` / operations |
| 900-999 | `error.*` |

Example mapping:

| Type id | Type string | Direction | Readable `data` fields | Compact `d` order |
| --- | --- | --- | --- | --- |
| 101 | `auth.session.current` | server | `user_id`, `status` | `user_id`, `status` |
| 201 | `chat.message.send` | client | `conversation_id`, `body` | `conversation_id`, `body` |
| 202 | `chat.message.created` | server | `message_id`, `conversation_id`, `sender_id`, `body`, `created_at` | `message_id`, `conversation_id`, `sender_id`, `body`, `created_at` |
| 203 | `chat.message.updated` | server | `message_id`, `conversation_id`, `body`, `edited_at` | `message_id`, `conversation_id`, `body`, `edited_at` |
| 204 | `chat.message.deleted` | server | `message_id`, `conversation_id`, `deleted_at` | `message_id`, `conversation_id`, `deleted_at` |
| 301 | `group.member.added` | server | `group_id`, `user_id`, `visible_from_message_id` | `group_id`, `user_id`, `visible_from_message_id` |
| 401 | `community.created` | server | `community_id`, `name`, `slug` | `community_id`, `name`, `slug` |
| 501 | `discussion.post.created` | server | `post_id`, `community_id`, `author_id`, `title` | `post_id`, `community_id`, `author_id`, `title` |
| 502 | `discussion.reply.created` | server | `reply_id`, `post_id`, `community_id`, `author_id`, `body` | `reply_id`, `post_id`, `community_id`, `author_id`, `body` |
| 601 | `notification.created` | server | `notification_id`, `kind`, `source_id`, `created_at` | `notification_id`, `kind`, `source_id`, `created_at` |
| 901 | `error.validation` | server | `code`, `fields` | `code`, `fields` |
| 902 | `error.auth` | server | `code`, `message` | `code`, `message` |

Registry rules:

- A type id must map to exactly one type string.
- A type string must map to exactly one type id.
- Compact payload order must be explicit.
- Fields cannot be reordered for an existing type/version.
- Deprecated packet types should remain reserved, not reused.

## 14. Packet Versioning

Readable mode can tolerate additive fields better than compact mode, but both need version rules.

Recommendations:

- Include a protocol version during connection/bootstrap.
- Include packet schema version in the registry.
- Prefer additive optional fields in readable mode.
- For compact arrays, do not insert new fields in the middle of `d`.
- If compact payload order must change, create a new packet type id or versioned type.
- Keep old decoders during transition windows if clients can lag behind.

Possible readable packet with version:

```json
{
  "id": "evt_01J...",
  "type": "chat.message.created",
  "v": 1,
  "state": "ok",
  "data": {}
}
```

Possible compact packet with version:

```json
{
  "i": "evt_01J...",
  "t": 202,
  "v": 1,
  "s": 1,
  "d": []
}
```

## 15. Request/Reply Correlation

Client requests should include `req`.

Server replies should include both:

- `id`: unique server packet id.
- `req`: original client request id.

Example:

```json
{
  "id": "res_01J...",
  "req": "req_01J...",
  "type": "chat.message.created",
  "state": "ok",
  "data": {
    "message_id": "msg_01J..."
  }
}
```

Guidelines:

- `req` is client-generated and unique enough for the session.
- Server should return the same `req` on success or error.
- The client can use `req` to resolve pending UI state.
- Server-generated events that are not replies do not need `req`.

## 16. Reconnect and Idempotency Notes

Realtime packets must assume reconnects, retries, duplicate delivery, and missed delivery.

Recommendations:

- Every server event should have a stable `id`.
- Domain objects should have stable ids, for example `message_id` or `post_id`.
- Client should deduplicate by packet id and/or domain object id.
- Message history remains the source of truth after reconnect.
- Reconnect should trigger resync from the last known message/event cursor.
- Group resync must enforce the member's join-boundary marker.
- Compact mode must not remove ids needed for idempotency.
- Client retries should use `req` to avoid duplicate pending UI states.
- Server-side idempotency for client commands may need a request-id ledger for actions such as message send.

## 17. Open Questions

- Is protocol mode negotiated per WebSocket connection, user setting, environment, or deployment config?
- Should REST accept compact mode at all, or stay readable-only?
- Should packet ids use ULID-style sortable ids, UUIDs, or another format?
- Should request ids be globally unique or only unique per connection/session?
- Should server replies use the same type as the resulting event or a separate `.ok` type?
- Should errors have domain-specific types, such as `chat.error.rate_limited`, or only shared `error.*` types with codes?
- What fields are mandatory in every packet: `id`, `type`, `state`, `v`, `data`?
- Should compact state use `1/0` only, or reserve more numeric states?
- Should compact packet type ids be stable forever once published?
- Should packet schemas be stored as Markdown tables, JSON Schema, Protocol Buffers-like definitions, or another registry format?
- What is the minimum initial packet registry needed before coding starts?
- How should authorization failures be represented: `error.auth`, `error.forbidden`, or both?
- Should validation field paths use strings, arrays, or JSON pointer syntax?
