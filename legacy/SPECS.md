
# REALTIME COMMUNITY PLATFORM — COMPLETE OPTIMIZED SYSTEM BLUEPRINT
Single Node • Rust Backend • PostgreSQL • WebSocket • Caddy TLS • Event-Driven

This document is the **authoritative system specification**.  
All implementation must strictly follow this blueprint.  
Every function, operation, and logical step in code MUST contain explanatory comments.

---

# 1. HIGH LEVEL ARCHITECTURE

## 1.1 Deployment Model

- Single physical / virtual machine
- OS: Ubuntu 24.04 LTS
- Reverse proxy: Caddy
- Strict TLS (HTTP → HTTPS redirect)
- Rust backend (no Docker)
- PostgreSQL (same machine)
- Local file storage
- No Redis
- No external queue

---

# 2. PROJECT STRUCTURE

```

/web
├── index.html        # Captcha gate
├── auth.html         # Login / Register / Reset / PIN
├── app.html          # Main UI (FINAL WIREFRAME HERE)
├── melt.css
├── melt.js
└── assets/

/src
├── main.rs
├── config/
├── db/
├── models/
├── auth/
├── websocket/
├── events/
├── permissions/
├── rate_limit/
├── presence/
├── chats/
├── community/
├── discussion/
├── notifications/
├── files/
├── logging/
├── backup/
└── health/

```

IMPORTANT:
UI wireframe MUST live at:

```

web/app.html

```

---

# 3. CORE DESIGN PRINCIPLES

- Event-driven (NOT event-sourced)
- Database is source of truth
- Event table is audit log only
- No replay
- No snapshot
- Transaction integrity mandatory
- Permission-based moderation (no role enum)
- u8 event types (0 and 255 reserved)
- All serializable structures must implement serialization

---

# 4. AUTHENTICATION FLOW

## 4.1 index.html

- Only captcha
- On success:
  - Check session
  - If valid → app.html
  - If invalid → auth.html

---

## 4.2 auth.html

Contains:
- Sign in
- Sign up
- Reset password
- PIN entry (may be requested on refresh)

---

## 4.3 Registration

Flow:

```

User registers
→ status = Pending
→ Cannot login
→ Admin approves
→ status = Active

```

If admin rejects:
→ Hard delete account

No email verification.

---

# 5. USER MODEL

## 5.1 Constraints

- login_username (3–16 chars)
- public_username (3–16 chars)
- immutable
- case insensitive match for mention
- exact match only
- password policy: only a-z0-9

## 5.2 Password Hashing

Algorithm:
Argon2id

Reason:
Balanced security + performance

---

## 5.3 Status Enum

| Value | Status |
|-------|--------|
| 1 | Pending |
| 2 | Active |
| 3 | Suspended |
| 4 | Banned |

Behavior:

| Status | Login | Read | Write |
|--------|-------|------|-------|
| Pending | ❌ | ❌ | ❌ |
| Active | ✅ | ✅ | ✅ |
| Suspended | ✅ | ✅ | ❌ |
| Banned | ❌ | ❌ | ❌ |

Suspended users:
- Can login
- Can connect WebSocket
- All write operations rejected

---

# 6. SESSION SYSTEM

## 6.1 Model

- DB-backed sessions
- Signed secure cookie
- 7 days max lifetime
- 24h inactivity timeout
- Multi-device supported

## 6.2 Logout

- Logout current device
- Logout all devices

---

# 7. PRESENCE SYSTEM

Table: user_sessions

- One row per active WS connection
- On connect → insert
- On disconnect → mark disconnected_at
- last_seen updated

Online rule:
User online if active_session_count > 0

If any device active → user active

Events:
- UserConnected
- UserDisconnected

---

# 8. FRIEND & DM SYSTEM

## 8.1 Friend Logic

- Not friend → DM request
- Friend → Direct chat

## 8.2 DM Start Rate

Limits:
- 2 per minute
- 10 per hour
- 30 per day

Configurable in DB.

---

# 9. CHAT SYSTEM

## 9.1 Chat Types

| Value | Type |
|-------|------|
| 1 | Direct Message |
| 2 | Group |
| 3 | Community |

Group creation:
Max 3 per user

## 9.2 Group Rule

User only sees messages after joined_at.

---

# 10. MESSAGE TABLES (SEPARATED)

- dm_messages
- group_messages
- community_messages

Soft delete:
- deleted flag
- Mesaj gövdesi herkeste “Mesaj silindi” olarak güncellenir (orijinal saklanmaz)

Message edit:
- Allowed for 300 seconds from created_at
- After that → locked
- Discussion edit requires moderation approval

Message DTO (REST + WS):
- id, chat_id, chat_type, sender_id, body, created_at
- edited_at (nullable), deleted (bool)

## 10.1 Message Mutation API

- POST `/api/chats/message` — create message (rate limit 1 / 3s)
- POST `/api/chats/message/edit` — sender-only, within 300s, preserves mentions, re-broadcasts
- POST `/api/chats/message/delete` — sender-only soft delete, sets body to “Mesaj silindi”, clears mentions

---

# 11. COMMUNITY

- Everyone sees all messages
- Message deletion shows placeholder
- Editable 300 seconds
- Notification configurable
- Can mute community notifications

---

# 12. DISCUSSION SYSTEM

## 12.1 Model

Thread style:
- parent_id
- max depth = 3
- flat storage
- UI renders hierarchy

## 12.2 Voting

- 1 vote per user per post
- value = -1 or 1
- undo allowed
- rate: 1 per minute

Sorting:
- hot
- top
- new

## 12.3 Poll

Skeleton only.
Future extension ready.

---

# 13. MENTION SYSTEM

User types:
```

@username

```

Stored in DB as:
```

:mention=USER_ID

```

Rules:
- Max 5 per message
- Case insensitive match
- Exact match only
- Mention overrides mute

---

# 14. FILE SYSTEM

Allowed:
- zip (download only)
- image
- video
- audio

Path:
```

/storage/{year}/{month}/{uuid}

```

Permission level:

| Level | Limit |
|-------|--------|
| 1 | 2MB |
| 2 | 20MB |
| 3 | Unlimited |

MIME validation required.

---

# 15. RATE LIMITING

All stored in DB table `rate_config`.

## Login

- 3 failed attempts → 3 min lock
- Each additional → +30 sec

## Message

- 1 per 3 sec

## Upvote

- 1 per minute

User-based, not device-based.

---

# 16. PERMISSION SYSTEM

No roles.
Permission table only.

Examples:
- delete_message
- suspend_user
- approve_discussion_edit
- upload_file_level_1
- upload_file_level_2
- upload_file_level_3
- moderate_group
- moderate_community

Default permission set defined in config.

---

# 17. EVENT SYSTEM

## 17.1 Event Enum (u8)

0 reserved  
255 reserved  

Examples:

1  UserConnected  
2  UserDisconnected  
3  MessageSent  
4  MessageEdited  
5  MessageDeleted  
6  DiscussionCreated  
7  DiscussionReplied  
8  VoteCast  
9  FileUploaded  
10 FriendRequested  
11 FriendAccepted  
12 NotificationCreated  
13 DiscussionEditRequested  
14 DiscussionEditApproved  
15 DiscussionEditRejected  
16 AdminPermissionChanged  
17 AdminRateConfigUpdated  

## 17.2 Event Handling

- Insert event row inside transaction
- After commit → async dispatch
- If handler fails → log error
- No rollback

No event sourcing.
No replay.
No snapshot.

---

# 18. TRANSACTION BOUNDARY (CRITICAL)

Message Send:

BEGIN

1. Status check
2. Permission check
3. Rate limit check
4. Insert message
5. Parse mentions
6. Insert notifications
7. Insert event row
8. Insert system log

COMMIT

Dispatch async.

Rollback only on:
- Permission fail
- Validation fail
- Rate fail

---

# 19. NOTIFICATION SYSTEM

- Real-time via WebSocket
- Unread counter in DB
- Per-chat mute
- mute_until supported
- Mention overrides mute

---

# 20. LOGGING

## 20.1 Activity Logs

Examples:
- User logged in
- User connected
- User sent message
- User added friend
- User disconnected

Partition:
Monthly

Retention:
No deletion

---

# 21. BACKUP SYSTEM

Every 12 hours:

```

dump/YYYY-MM-DD-HH/
users.sql
chats.sql
events.sql
logs.sql
...
backup.zip

```

- Table-by-table export
- Zip compression

No automatic deletion.

---

# 22. PAGINATION

Time-based cursor:

```

WHERE created_at < ?
ORDER BY created_at DESC
LIMIT 50

```

---

# 23. MONITORING

- /health endpoint
- Panic hook
- Structured JSON logs

---

# 24. UI STRUCTURE (web/app.html)

Sections:

Sidebar:
- Chats
- Community
- Discussion
- Notifications

Main Area:
- Message panel
- Thread panel
- Vote controls
- File upload
- Mention highlight

Theme:
- Blue + Black
- melt.css
- melt.js
- lucide-icons

---

# 25. SYSTEM CHARACTERISTICS

- Event-driven
- Transaction safe
- Multi-device aware
- Moderation capable
- Abuse resistant
- Log heavy
- Single-node optimized
- Extensible
- Production baseline ready

---

END OF COMPLETE SYSTEM BLUEPRINT
