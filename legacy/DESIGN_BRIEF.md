# Novastrum UI Design Brief (for new agent)

## Scope
- Build a **single-page web UI** for chat (DM/Group/Community), discussion threads, notifications, and DM request handling.
- Use existing backend APIs and websocket events; do **not** change backend contracts.
- Modern, responsive layout; no external CDN required (can use Tailwind via build or local CSS, but keep setup notes minimal).

## Key User Flows
1) **Bootstrap**
   - GET `/api/bootstrap/state` → `{ current_user_id: string|null, users: [{id, login_username, status}], chats: [{id, chat_type}] }`.
   - If 401/403 → redirect to `/index.html`.

2) **Chat**
   - Send message: POST `/api/chats/message` with `{ user_id, chat_id, chat_type, body }`.
   - Edit message: POST `/api/chats/message/edit` `{ user_id, message_id, chat_type, new_body }` (<=300s, sender only).
   - Delete message: POST `/api/chats/message/delete` `{ user_id, message_id, chat_type }` (soft delete, body="Mesaj silindi").
   - DM request start: POST `/api/chats/dm-request` `{ from_user_id, to_user_id }` (blocked if pending exists either direction).
   - Group create: POST `/api/chats/group` `{ user_id }` (max 3 per user).

3) **DM Requests (Friend Flow)**
   - List incoming: GET `/api/chats/dm-request/list` (auth user only).
   - Accept: POST `/api/chats/dm-request/accept` `{ user_id, request_id }` → creates friendship + DM chat + members.
   - Reject: POST `/api/chats/dm-request/reject` `{ user_id, request_id }`.
   - Constraint: If a pending request exists either direction, new request is rejected (409).

4) **Discussions**
   - Create post/reply: POST `/api/discussion/post` `{ user_id, thread_id?, parent_id?, title, body }` (depth<=3).
   - Vote: POST `/api/discussion/vote` `{ user_id, post_id, value(-1|1), undo? }` (rate 1/min).
   - List: GET `/api/discussion/list?sort=hot|top|new&cursor=` → `{ items: DiscussionPost[], next_cursor }`.

5) **Notifications**
   - List: GET `/api/notifications/list?user_id=&cursor=&limit=`.
   - Mute community: POST `/api/community/mute` `{ user_id, chat_id, mute_minutes }` (mention overrides mute).

6) **Auth**
   - Assumes session cookie exists; failed auth → redirect to `/index.html`.

## Data Shapes (frontend relevant)
- **ChatMessage**: `{ id, chat_id, chat_type (1|2|3 or "DirectMessage"|"Group"|"Community"), sender_id, body, mentions[], deleted, created_at, edited_at? }`
- **ChatRoom**: `{ id, chat_type, created_by, created_at, label? }`
- **DiscussionPost**: `{ id, thread_id, parent_id, depth, author_id, title, body, created_at, edited_at? }`
- **NotificationItem**: `{ id, user_id, chat_id?, kind, body, unread, created_at }`
- **DmRequestRow**: `{ id, from_user_id, to_user_id, created_at }`

## WebSocket Events (connection.js maps string `type`)
- `chat.message` payload: `{ chat_id, chat_type, message: ChatMessageDTO }`
- `discussion.post` payload: DiscussionPost
- `discussion.vote` payload: `{ post_id, score }`
- `notification.created` payload: Notification
- `admin.event` (rare, admin only)

## Helpers available
- `web/connection.js`:
  - `Connection.connect()` opens WS.
  - Commands: `send_dm`, `edit_message`, `delete_message`, `discussion_reply`, `discussion_vote`, `mute_chat`, plus low-level `on(event, handler)` for WS.
- `web/melt.js`:
  - `Melt.api(path, method, payload)` fetch wrapper.
  - `Melt.highlightMentions(text)` to render @username.
  - `Melt.showNotice/hideNotice`, `Melt.toast` for lightweight UI messages.

## UX/logic requirements
- Show chat lists segmented: Direct (friends), Groups, Community.
- Message feed should display body, time, edited/deleted state; allow edit (<=300s) & delete for own messages.
- DM request inbox: list incoming with sender full name (login_username), Accept/Reject buttons.
- Prevent duplicate outgoing requests if one is pending (already enforced server-side; surface error).
- Group create button (visible when logged in).
- New chat start field to send DM request to selected user.
- Discussions tab: create thread, list posts, vote buttons; pagination via `next_cursor`.
- Basic error handling: 401/403 → redirect to `/index.html`; other API errors → toast/notice.

## Minimal state you’ll need in the new UI
- `currentUserId`, `users`, `chats`, `dmRequests`, `messagesByChatId`, `discussionItems`, `discussionCursor`.

## Notes
- Chat history API for pagination is not implemented; current messages are WS + send responses. Keep feed append-only for now.
- Presence/unread counters exist server-side but not exposed in bootstrap; optional.
- File upload endpoints exist (`/api/files/prepare`, `/api/files/upload`) but raw UI currently omits them; include only if needed.

