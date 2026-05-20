# API & WebSocket Guide

## HTTP API Overview
1. **Session/Authentication**
   - `/api/captcha/verify` (POST): verifies captcha token (dev secret: `i-am-human`). Returns `next_page` pointing to `app.html` or `auth.html` depending on if a valid session exists.
   - `/api/auth/login` (POST): login by `login_username` & `password` and optional `device_label`. Returns signed cookie set via `Set-Cookie` and a redirect payload.
   - `/api/auth/register`, `/api/auth/reset`, `/api/auth/pin`, `/api/auth/logout`, `/api/auth/logout-all`: follow the flows described in `SPECS.md`. All expect JSON body and send `application/json` responses wrapped inside `ApiEnvelope`.

2. **Chat/Discussion**
   - `/api/chats/message` (POST): send chat message. Requires `user_id`, `chat_id`, `chat_type`, `body`. Rate limited (1 msg / 3s) and enforces mention/permission rules.
   - `/api/chats/message/edit` (POST): sender-only edit within 300s of `created_at`; body trimmed; re-parses mentions; broadcasts `chat.message` with `edited_at`.
   - `/api/chats/message/delete` (POST): sender-only soft delete; body becomes `Mesaj silindi`, `deleted = true`, `mentions = []`; broadcasts `chat.message`.
   - `/api/chats/dm-request` (POST) & `/api/chats/group` (POST): DM request/group creation flows tied to authenticated user_id.
   - `/api/discussion/post` (POST) & `/api/discussion/vote` (POST): create posts or vote. `/api/discussion/list` (GET) supports `sort` query (`hot`, `top`, `new`).
   - `/api/community/mute` (POST): mutes community notifications for `mute_minutes`.

3. **Notifications & Files**
   - `/api/notifications/list` (GET): accepts `user_id`, `cursor`, `limit` and returns paged notifications (chronological cursor).
   - `/api/files/prepare` (POST): validates file metadata before upload (checks MIME, size, permission level) and updates UI with readiness state.
   - `/api/files/upload` (POST multipart): accepts `user_id`, `file_id`, and the binary payload to persist the file content to disk; use `FormData` + `Melt.uploadFile` helper.

4. **Admin**
   - `/api/admin/overview`, `/api/admin/users`, `/api/admin/users/approve`, `/api/admin/users/status`, `/api/admin/users/reject`, `/api/admin/users/permission`, `/api/admin/rate-config`, `/api/admin/audit`: endpoints for admin dashboard in `admin.html`. All require admin privileges checked before serving `admin.html`.

## WebSocket Event Flow
- Connection occurs at `/ws` using signed session cookie. Only authenticated sessions may connect.
- Server emits `WsServerEvent::chat.message`, `WsServerEvent::discussion.post`, `WsServerEvent::discussion.vote`, `WsServerEvent::notification.created`, `WsServerEvent::admin.event`, etc. Each handler receives a `payload` object tailored to the event (message DTOs, discussion posts, votes, notification records, admin metadata).
- Client `Melt.startPresenceSocket(elementId)` handles connection management, periodic ping, and reconnection with exponential backoff. The method internally manages timers to update `presence-status` fields on the UI.
- Extend `Melt.onWebSocketEvent` with your own listeners (e.g., refresh discussion list on `discussion.post`, refresh notifications when `notification.created` for the authenticated user, and highlight admin changes whenever `admin.event` fires). Guard `payload.type` and the required identifiers before mutating UI state.
- Future WebSocket improvements (see `EKSIK.md` sections 8 & 9) should add more event types if necessary.

### chat.message payload
```jsonc
{
  "type": "chat.message",
  "payload": {
    "chat_id": "<uuid>",
    "chat_type": 1|2|3,
    "message": {
      "id": "<uuid>",
      "body": "text or Mesaj silindi",
      "sender_id": "<uuid>",
      "created_at": "2026-02-16T12:00:00Z",
      "edited_at": "2026-02-16T12:02:00Z|null",
      "deleted": false
    }
  }
}
```

## Client Integration Notes
1. **Session gating**
   - Begin at `index.html`. The captcha gate posts to `/api/captcha/verify`. On success, follow the `next_page` from response for either `app.html` or `auth.html`.
   - All subsequent pages rely on server-set HttpOnly cookies; `Melt.api` uses `credentials: "same-origin"` so cookies are sent automatically.
2. **Presence-driven UI**
   - Both `app.html` and `admin.html` call `Melt.startPresenceSocket` to maintain a WebSocket link. Use `Melt.onWebSocketEvent` to react to server broadcasts.
3. **Error handling**
   - APIs uniformly return `ApiEnvelope`. Check `result.ok` and `result.body.ok` before trusting `result.body.data`.
   - WebSocket parsing issues are logged to console. Always guard `payload.type`, `payload.chat_id`, and `payload.message` before mutating UI state, and keep updates idempotent so repeated broadcasts don't duplicate messages.
4. **File upload helper**
   - Use `Melt.uploadFile(file, { user_id, file_id })` after `/api/files/prepare` returns `file_id`. The helper uses `FormData` to send the binary payload and the required metadata, then returns another `ApiEnvelope` response to confirm the upload succeeded.
