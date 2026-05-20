# REST API Contract

## 1. Executive Summary

This document defines the REST API contract for the Novastrum Chat rebuild. REST should handle authentication, current-user bootstrap, profile/settings updates, conversation lookup, message history, group membership actions, communities, discussion content, and later operational APIs.

Realtime delivery and realtime client commands belong to WebSocket protocol design and should be specified in `docs/websocket-protocol.md`. REST may send messages and perform state-changing actions, but WebSocket is the preferred delivery path for live updates.

REST responses should use a readable packet-inspired envelope. This keeps REST aligned with the packet protocol direction without blindly preserving the legacy `ApiEnvelope` shape.

## 2. REST Response Contract

### Success Response

```json
{
  "state": "ok",
  "type": "auth.login",
  "req": "req_01J...",
  "data": {}
}
```

### Error Response

```json
{
  "state": "error",
  "type": "error.validation",
  "req": "req_01J...",
  "data": {
    "code": "validation_failed",
    "message": "Validation failed",
    "fields": {
      "user_name": "required"
    }
  }
}
```

### Paginated Response

```json
{
  "state": "ok",
  "type": "chat.messages.list",
  "req": "req_01J...",
  "data": {
    "items": [],
    "page": {
      "next_cursor": "msg_01J...",
      "has_more": true
    }
  }
}
```

### Fields

| Field | Meaning |
| --- | --- |
| `state` | `ok` or `error`. REST should also use appropriate HTTP status codes. |
| `type` | Readable packet-style response type, such as `auth.login` or `error.validation`. |
| `req` | Request correlation id. Prefer accepting a client-provided request id header later, but server generation is acceptable for the first contract. |
| `data` | Success payload or structured error payload. |

Validation errors should use `state: "error"`, `type: "error.validation"`, `data.code: "validation_failed"`, a human-readable `message`, and a `fields` object keyed by field name.

Pagination should use cursor pagination by default. Lists should return `items` and `page`, where `page.next_cursor` is nullable and `page.has_more` is boolean.

REST response bodies do not need a server event `id` field initially. WebSocket packets need packet ids for event delivery, idempotency, reconnect, and replay behavior; that is a separate protocol concern.

## 3. Authentication Model

### Endpoints

| Method | Path | Purpose |
| --- | --- | --- |
| `POST` | `/api/auth/register` | Create a user account. |
| `POST` | `/api/auth/login` | Create a session and set the session cookie. |
| `POST` | `/api/auth/logout` | Revoke the current session. |
| `GET` | `/api/me` | Return the current user and bootstrap flags. |
| `POST` | `/api/auth/logout-all` | Later: revoke all sessions for the current user. |

### Registration Behavior

Registration mode must be configurable:

| Mode | Behavior |
| --- | --- |
| `open` | Registration creates an `active` user unless policy changes require moderation. |
| `approval_required` | Registration creates a `pending` user that needs admin approval before write access. |
| `invite_only` | Later. Requires invite token validation before account creation. |

### User Status Behavior

| Status | Login | Read | Write |
| --- | --- | --- | --- |
| `pending` | Tentative. Product decision needed. | Tentative. | No. |
| `active` | Yes. | Yes. | Yes. |
| `suspended` | Yes. | Yes. | No. |
| `banned` | No. | No. | No. |
| `deleted` | No. | No. | No. |

Authentication should be session-cookie based. Session cookie settings should be `HttpOnly`, `Secure` in production, and use a same-site policy appropriate for the final deployment shape.

Because cookie authentication is used, CSRF protection must be designed before write endpoints are exposed to browsers. The first implementation should choose one clear policy: same-site-only plus origin checks, or explicit CSRF tokens for state-changing REST requests.

## 4. User API

### Endpoints

| Method | Path | Purpose |
| --- | --- | --- |
| `GET` | `/api/users/search?q=` | Search users by handle/display name for DM or group member selection. |
| `GET` | `/api/users/:user_name` | Fetch public user profile by handle. |
| `PATCH` | `/api/me/profile` | Update current user's display profile. |
| `PATCH` | `/api/me/settings` | Update current user's settings, including `dm_policy`. |

### Contract Notes

`user_name` is the unique handle and may be used in user URLs. `public_name` is the editable display name. API responses should include user `public_id`; internal numeric ids must not be exposed.

`dm_policy` is stored per user and can be one of:

| Value | Meaning |
| --- | --- |
| `everyone` | Any active authenticated user may start a direct conversation. |
| `shared_group_members` | Only users sharing at least one active group membership may start a direct conversation. |
| `friends_only` | Reserved until friendship support exists. |
| `none` | No new direct conversations from other users. |

Status visibility should be conservative. Public user responses should not expose sensitive moderation state beyond what the viewer needs. For deleted users, show a stable deleted-user placeholder where historical content requires attribution.

## 5. Conversations API

### Endpoints

| Method | Path | Purpose |
| --- | --- | --- |
| `GET` | `/api/conversations` | List direct and group conversations visible to the current user. |
| `POST` | `/api/conversations/direct` | Create or return the direct conversation with another user. |
| `GET` | `/api/conversations/:conversation_id` | Fetch conversation metadata if visible to the current user. |
| `GET` | `/api/conversations/:conversation_id/messages` | Fetch paginated message history. |

### Contract Notes

`conversation_id` is a public conversation id, not an internal numeric id.

Direct conversation creation must enforce direct conversation uniqueness. The backend should return the existing direct conversation when the canonical pair already exists.

Before creating a direct conversation or sending the first direct message, the server must enforce the recipient's `dm_policy`.

Conversation list visibility:

| Conversation Kind | Visibility Rule |
| --- | --- |
| `direct` | Only active participants can see it. |
| `group` | Only active members can see it. |

Message history must be cursor-paginated. For group conversations, history queries must apply the requesting member's `visible_from_message_id` boundary. A user who joined or rejoined later must not receive messages older than that boundary.

## 6. Messages API

### Endpoints

| Method | Path | Purpose |
| --- | --- | --- |
| `POST` | `/api/conversations/:conversation_id/messages` | Send a text message through REST. |
| `DELETE` | `/api/messages/:message_id` | Soft-delete/redact a message. |
| `PATCH` | `/api/messages/:message_id` | Later: edit a message. |

### Contract Notes

Messages should be text-only first. Media attachments are later.

The initial body length limit should be explicit before implementation. Until decided, the API contract should reject empty messages and reserve a server-configured maximum.

Deleted messages remain as placeholders in history. Normal users should see the message position, timestamp, deletion state, and safe author display, but not deleted body content. Delete behavior should set `deleted_at`, `deleted_by`, and redact or clear `body`.

Message editing is deferred unless product rules change. If implemented later, edited messages need `edited_at`, authorization rules, and possibly edit history policy.

## 7. Groups API

### Endpoints

| Method | Path | Purpose |
| --- | --- | --- |
| `POST` | `/api/groups` | Create a private group conversation. |
| `GET` | `/api/groups` | List groups where the current user is a member. |
| `GET` | `/api/groups/:group_id` | Fetch group details if the current user is a member. |
| `POST` | `/api/groups/:group_id/members` | Owner adds a member. |
| `DELETE` | `/api/groups/:group_id/members/:user_id` | Owner removes a member. |
| `POST` | `/api/groups/:group_id/leave` | Current member leaves the group. |

### Contract Notes

Groups are private to members. There is no public group discovery by default. `GET /api/groups` must only return groups the current user belongs to.

Group roles start as:

| Role | Permissions |
| --- | --- |
| `owner` | Add/remove members and manage basic group membership. |
| `member` | Read visible group history, send messages, and leave. |

Group max size is 10. Adding the eleventh active member should return `group_full`.

When a member is added, the server must assign a new visibility boundary such as `visible_from_message_id`. New members can only see messages created after they joined. If a user leaves and rejoins, a new membership lifecycle row and a new visibility boundary must be used; rejoining must not restore access to older messages.

Owner-leaves behavior is not finalized. Until it is, owners should not be allowed to leave if that would leave the group without an owner, or the API should require an explicit transfer step.

## 8. Communities API

### Endpoints

| Method | Path | Purpose |
| --- | --- | --- |
| `GET` | `/api/communities` | List visible communities. |
| `GET` | `/api/communities/:slug` | Fetch public community details. |
| `POST` | `/api/communities` | Admin creates a community initially. |
| `POST` | `/api/communities/:slug/follow` | Follow a community. |
| `DELETE` | `/api/communities/:slug/follow` | Unfollow a community. |

### Contract Notes

Communities are Reddit-like topic spaces. Public community discussion is readable without joining. Joining or following affects personal feed weighting, notification preferences, and later personalization; it does not control basic read access for public communities.

Posting and replying require authentication and write permission. Suspended users may read but cannot post or reply. Banned and deleted users cannot access authenticated actions.

Community creation is admin-only initially. Community owner/moderator roles are deferred.

Private communities are deferred. Do not design public group-like chat into communities at this stage.

## 9. Discussion API

### Endpoints

| Method | Path | Purpose |
| --- | --- | --- |
| `GET` | `/api/discussions/feed` | Combined discussion feed across visible communities. |
| `GET` | `/api/communities/:slug/discussions` | Paginated community discussion list. |
| `POST` | `/api/communities/:slug/discussions` | Create a discussion post in a community. |
| `GET` | `/api/discussions/:post_id` | Fetch discussion post with replies. |
| `POST` | `/api/discussions/:post_id/replies` | Add a reply to a discussion post. |
| `POST` | `/api/discussions/:post_id/votes` | Vote on a discussion post. |
| `POST` | `/api/discussion-replies/:reply_id/votes` | Vote on a discussion reply. |
| `PATCH` | `/api/discussions/:post_id/solved` | Later: mark solved/open. |

### Contract Notes

Discussion posts belong to communities. A combined feed should aggregate posts from visible communities and should not require a special General community.

Discussion lists should use cursor pagination. Feed filters may include `newest` first, with `popular`, `solved`, and `open` later.

The exact vote model remains open. The REST contract should allow one vote per user per target, but the database design may finalize either unified votes or split post/reply vote tables.

Solved/open support should be designed into the data model but does not need to be built into the first API slice.

## 10. Notifications API

Notifications are later unless a concrete first-build workflow requires them.

| Method | Path | Purpose |
| --- | --- | --- |
| `GET` | `/api/notifications` | Later: list notifications. |
| `PATCH` | `/api/notifications/:id/read` | Later: mark one notification as read. |
| `GET` | `/api/notifications/unread-count` | Later: unread count badge. |

Notification events should not be overbuilt before chat, group, and discussion event rules are concrete.

## 11. Media API

Media is later and must be security-aware before implementation.

| Method | Path | Purpose |
| --- | --- | --- |
| `POST` | `/api/media/prepare` | Later: prepare an upload. |
| `POST` | `/api/media/upload` | Later: upload media. |
| `DELETE` | `/api/media/:media_id` | Later: remove or detach media. |

Media design must define file type allowlists, maximum sizes, malware scanning expectations if needed, storage location, authorization, attachment ownership, URL signing or access control, and deletion behavior.

## 12. Admin API

Admin APIs should be minimal early.

Initial admin needs:

| Need | Possible Endpoint |
| --- | --- |
| Approve pending users when `approval_required` is enabled. | Later: `POST /api/admin/users/:user_id/approve` |
| Create communities initially. | `POST /api/communities` with admin authorization. |
| Change user statuses. | Later: `PATCH /api/admin/users/:user_id/status` |

Admin endpoints should not become a large operations surface before product moderation rules are clearer.

## 13. Error Codes

| Code | Meaning |
| --- | --- |
| `validation_failed` | Request payload or query parameters failed validation. |
| `auth_required` | Authentication is required. |
| `invalid_credentials` | Login credentials are invalid. |
| `pending_approval` | Account is pending approval. |
| `suspended` | User may read but cannot write. |
| `banned` | User is banned and cannot log in. |
| `forbidden` | Authenticated user lacks permission. |
| `not_found` | Resource does not exist or is not visible to the requester. |
| `rate_limited` | Request exceeded rate limits. |
| `conflict` | Resource conflict, such as uniqueness violation. |
| `dm_not_allowed` | Recipient DM policy blocks the action. |
| `group_full` | Group max size would be exceeded. |
| `not_group_member` | User is not an active group member. |
| `group_history_forbidden` | Requested group history is before the user's visibility boundary. |

Where possible, APIs should prefer `not_found` over exposing the existence of private conversations or private/deferred resources.

## 14. Authorization Matrix

| Endpoint Group | Guest | Pending User | Active User | Suspended User | Banned User | Admin |
| --- | --- | --- | --- | --- | --- | --- |
| Auth register/login | Yes | Login tentative | Yes | Yes | No | Yes |
| `GET /api/me` | No | Yes, if login allowed | Yes | Yes | No | Yes |
| User search/profile | Public profile only | Read tentative | Yes | Yes | No | Yes |
| Profile/settings write | No | No | Own account | No | No | Yes for admin actions later |
| Conversations list/detail | No | No | Visible only | Read visible only | No | Admin only if moderation policy later allows |
| Message history | No | No | Visible only | Read visible only | No | Admin only if audit policy later allows |
| Send messages | No | No | Yes, if authorized | No | No | Yes, as normal user/admin where authorized |
| Delete own message | No | No | Own messages where allowed | No | No | Later moderation delete |
| Groups read | No | No | Member groups only | Member groups only | No | Admin access later only if policy allows |
| Group membership writes | No | No | Owner only | No | No | Later admin moderation |
| Communities read | Public communities | Public communities | Public communities | Public communities | No | Yes |
| Community create | No | No | No | No | No | Yes |
| Discussion read | Public visible posts | Public visible posts | Public visible posts | Public visible posts | No | Yes |
| Discussion write/vote | No | No | Yes | No | No | Yes where authorized |
| Notifications | No | No | Own notifications later | Own read later | No | Admin not applicable initially |
| Media | No | No | Later, if authorized | No | No | Later admin moderation |

Pending-user login/read behavior is not fully finalized. Until it is, pending users should be denied write actions.

## 15. API Types and Public IDs

The API must expose public ids, not internal numeric ids. Internal `BIGINT UNSIGNED` ids are database implementation details.

Recommended public id examples:

| Entity | Example |
| --- | --- |
| User | `usr_01J...` |
| Session | `ses_01J...` |
| Conversation | `cnv_01J...` |
| Message | `msg_01J...` |
| Community | `com_01J...` |
| Discussion post | `pst_01J...` |
| Discussion reply | `rpl_01J...` |

User URLs may use `user_name` because handles are product-facing identifiers. Community URLs may use `slug` because slugs are product-facing identifiers. Other URL path ids should be public ids.

## 16. Open Questions

1. What is the default `dm_policy` for new users?
2. What is the initial registration mode: `open` or `approval_required`?
3. Can pending users log in and read anything, or are they blocked until approval?
4. Should discussion votes use a unified vote endpoint/table shape or split post/reply vote handling?
5. Is `user_name` immutable, or change-limited with a cooldown/history policy?
6. What happens when a group owner wants to leave or is deleted?
7. How should deleted users display in old messages, discussions, and replies?
8. Should REST `req` be client-supplied through a header/body field, server-generated, or both?
9. What first body length limit should messages, posts, and replies use?
10. Should suspended users be able to update profile/settings, or is all write access blocked?

## 17. Recommended Next Document

Create `docs/websocket-protocol.md` next. It should define realtime packet envelopes, readable and compact modes, packet type registry usage, request/reply correlation, delivery acknowledgements, reconnect/resync behavior, and idempotency rules.
