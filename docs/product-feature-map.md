# Product Feature Map

This document maps the product surface and dependencies for the Novastrum rebuild. It is intentionally framework-neutral and implementation-neutral. It describes what should exist, what each feature depends on, and what should be delayed to keep the rebuild controlled.

## What Should Exist In The Site

Novastrum should be a private/community communication site with:

- Account registration, login, session handling, and account status controls.
- A main authenticated application shell.
- Direct chat, group chat, and community chat.
- Message creation, history, edit, and soft delete.
- Friend/DM request flow before direct messaging.
- Threaded discussion posts with replies and voting.
- Realtime delivery for messages, discussion updates, notifications, and selected admin events.
- Notification list, unread state, and mute controls.
- Admin/moderation tools for users, permissions, status, rate limits, and discussion edit requests.
- Optional media upload once core messaging is stable.
- Operational basics: health checks, logging, backup policy, deployment notes, and secret handling.

## Categories And Features

## Foundation

Feature: User account
- Description: A person can register and has a stable identity for login, display, mentions, permissions, and ownership.
- Required dependencies: None.
- Database needs: Users table, unique login/public identifiers, password hash, status, timestamps.
- Backend needs: Register user, validate credentials, enforce username rules, expose current user identity.
- Frontend needs: Registration form, login form, auth error states, current-user bootstrap handling.
- Realtime needs: None initially.
- Security concerns: Password hashing, username enumeration, weak password policy, duplicate accounts, abuse registration.
- Can be postponed: No.
- Notes: Account identity is the root dependency for every user-facing feature.

Feature: Account status
- Description: Users can be pending, active, suspended, or banned, with predictable read/write/login behavior.
- Required dependencies: User account.
- Database needs: Status field and status-change audit trail if moderation is retained.
- Backend needs: Status checks in auth, read routes, and write routes.
- Frontend needs: Clear handling for pending approval, suspended write failures, banned login denial.
- Realtime needs: Optional admin event when status changes.
- Security concerns: Status bypass, stale sessions after ban, unclear suspended-user write access.
- Can be postponed: No, if registration approval remains part of the product.
- Notes: This prevents later moderation work from rewriting every route.

Feature: Session and authentication
- Description: Authenticated users can maintain secure sessions across requests and devices.
- Required dependencies: User account, account status.
- Database needs: Sessions table or equivalent persistence, expiry, inactivity tracking, device labels if needed.
- Backend needs: Login, logout, logout-all, current-session lookup, auth middleware/guard.
- Frontend needs: Login flow, logout action, redirect on unauthenticated access.
- Realtime needs: WebSocket auth later depends on the same session model.
- Security concerns: Cookie/session signing, fixation, expiry, inactive sessions, logout-all correctness.
- Can be postponed: No.
- Notes: Do not build chat before the session boundary is boring and reliable.

Feature: Authenticated app shell
- Description: A protected site area where logged-in users see navigation and product modules.
- Required dependencies: Session and authentication, bootstrap/current-user state.
- Database needs: Minimal current user and navigation state.
- Backend needs: Bootstrap endpoint or equivalent current-user payload.
- Frontend needs: Main layout, loading state, auth failure redirect, error surface.
- Realtime needs: None initially.
- Security concerns: Leaking protected data before auth check, stale bootstrap data.
- Can be postponed: No.
- Notes: The shell should be plain at first. It should not decide final visual design or framework architecture.

Feature: Authorization and permissions
- Description: The system knows who can perform admin/moderation-sensitive actions.
- Required dependencies: User account, account status, session.
- Database needs: Permissions table or simpler equivalent, user-permission assignment, audit history if needed.
- Backend needs: Permission checks around moderation/admin actions.
- Frontend needs: Hide or disable actions the user cannot use, but never rely on frontend checks.
- Realtime needs: Optional admin event when permissions change.
- Security concerns: Permission drift, role confusion, privilege escalation, missing route checks.
- Can be postponed: Partly. Basic admin guard cannot be postponed; fine-grained permissions can be simplified until needed.
- Notes: Keep the model small until actual moderation workflows prove the needed granularity.

Feature: Rate limits and abuse controls
- Description: Sensitive actions are throttled to prevent spam and accidental abuse.
- Required dependencies: User account, session, action definitions.
- Database needs: Rate config and action attempt tracking, or equivalent simple counters.
- Backend needs: Rate checks for login, messages, DM requests, votes, and uploads if enabled.
- Frontend needs: Friendly rate-limit errors and disabled/retry states where useful.
- Realtime needs: None.
- Security concerns: Brute force login, message spam, vote spam, account abuse, bypass by device/session.
- Can be postponed: Login and message rate limits should not be postponed; configurable admin UI can be postponed.
- Notes: Start with product-critical limits, not a broad rate-limit management system.

## Core Chat

Feature: Conversation model
- Description: A user can see chat containers for direct, group, and community messages.
- Required dependencies: User account, session, authorization, bootstrap state.
- Database needs: Conversations/chats table, chat type, membership where applicable, created_by, timestamps.
- Backend needs: List accessible conversations, enforce membership visibility.
- Frontend needs: Chat list, selected chat state, empty states.
- Realtime needs: Later used as a routing key for message events.
- Security concerns: Cross-chat access, membership leaks, community vs private visibility.
- Can be postponed: No for chat product.
- Notes: Keep the model flexible enough for direct, group, and community without starting all feature details at once.

Feature: Send message
- Description: A user can send a text message into a conversation they can write to.
- Required dependencies: Conversation model, user account, session, authorization, rate limits.
- Database needs: Message table(s), sender, chat id/type, body, created timestamp, deleted flag, edited timestamp.
- Backend needs: Validate membership/write permission, insert message, return message DTO.
- Frontend needs: Composer, send action, sending/error state, append confirmed message.
- Realtime needs: Optional at first; later emits `chat.message`.
- Security concerns: Unauthorized writes, spam, content length, mention abuse, malformed text.
- Can be postponed: No.
- Notes: A non-realtime send-and-refresh flow is enough before WebSocket delivery exists.

Feature: Message history
- Description: A user can load previous messages for a conversation.
- Required dependencies: Conversation model, send message, session, authorization.
- Database needs: Indexed messages by chat and creation time; cursor/pagination fields.
- Backend needs: List messages with cursor/limit and membership filtering.
- Frontend needs: Message feed, initial load, pagination/scroll behavior.
- Realtime needs: Reconnect resync later depends on reliable history.
- Security concerns: Reading messages from inaccessible chats, leaking pre-join group history if that rule remains.
- Can be postponed: No for a usable chat; advanced infinite scroll can be postponed.
- Notes: History is more foundational than realtime because it is the recovery path for missed events.

Feature: Message edit
- Description: Message sender can edit recent messages within a fixed time window.
- Required dependencies: Send message, message ownership, message history.
- Database needs: `edited_at`, current body, edit policy fields if needed.
- Backend needs: Ownership check, time-window check, update, return/broadcast new DTO.
- Frontend needs: Own-message action, edit input, edited marker.
- Realtime needs: Later emits updated `chat.message`.
- Security concerns: Editing after timeout, editing another user's message, audit expectations.
- Can be postponed: Yes.
- Notes: Useful but not required for the first stable chat path.

Feature: Message soft delete
- Description: Message sender can delete their message, leaving a placeholder instead of original body.
- Required dependencies: Send message, message ownership, message history.
- Database needs: Deleted flag and replacement body policy.
- Backend needs: Ownership check, soft-delete mutation, mentions cleanup if mentions exist.
- Frontend needs: Delete action, placeholder rendering.
- Realtime needs: Later emits updated `chat.message`.
- Security concerns: Deletion visibility, audit expectations, original body retention policy.
- Can be postponed: Yes.
- Notes: Decide whether original bodies are retained for moderation before coding.

Feature: Mentions
- Description: Users can reference other users with `@username`, creating highlight and notification behavior.
- Required dependencies: User account, message send, notification model if notifications are active.
- Database needs: Mention records or encoded mention references, username lookup strategy.
- Backend needs: Parse mentions, validate exact/case-insensitive usernames, enforce max mentions.
- Frontend needs: Highlight mentions, possibly autocomplete later.
- Realtime needs: Mention-triggered notification delivery later.
- Security concerns: Mention spam, username spoofing, user enumeration.
- Can be postponed: Yes.
- Notes: Mention highlighting can start display-only; notification override should wait for notification rules.

## Social Layer

Feature: User directory / people picker
- Description: Users can find another user to start a DM request.
- Required dependencies: User account, account status, session.
- Database needs: Queryable public user fields.
- Backend needs: User list/search endpoint with privacy limits.
- Frontend needs: People picker or start-chat field.
- Realtime needs: None.
- Security concerns: User enumeration, exposing pending/banned users, privacy.
- Can be postponed: Partly; manual exact username entry can stand in briefly.
- Notes: Avoid broad search before privacy rules are clear.

Feature: DM request
- Description: A user asks another user to allow direct messaging.
- Required dependencies: User account, user directory or exact username entry, session, rate limits.
- Database needs: DM request table with from/to/status/timestamps and uniqueness constraints.
- Backend needs: Create request, prevent duplicate pending requests in either direction, list incoming requests.
- Frontend needs: Send request flow, incoming request inbox, pending/duplicate errors.
- Realtime needs: Notification/realtime update can be added later.
- Security concerns: Request spam, harassment, blocked users if blocking is added later.
- Can be postponed: No if direct messages require friendship; yes if early chat starts with community-only.
- Notes: This is the gate into private direct conversations.

Feature: Accept/reject DM request
- Description: Recipient can accept or reject a pending request; accept creates friendship and direct chat access.
- Required dependencies: DM request, conversation model.
- Database needs: Friendships, DM chat membership, request status.
- Backend needs: Accept transaction, reject transaction, duplicate-safe creation.
- Frontend needs: Accept/reject controls, update chat list after accept.
- Realtime needs: Later notify requester and refresh recipient/requester state.
- Security concerns: Accepting someone else's request, duplicate DM chats, race conditions.
- Can be postponed: No if DM requests are active.
- Notes: This should be transactional and simple before adding notifications.

Feature: Friendship / direct membership
- Description: Accepted users have a persistent direct relationship and can see the direct chat.
- Required dependencies: Accept DM request, conversation model.
- Database needs: Friendship rows or direct chat membership rows.
- Backend needs: Membership checks for direct messages and chat listing.
- Frontend needs: Direct chat list and friend label rendering.
- Realtime needs: Optional online presence later.
- Security concerns: Access after unfriend/block if those features are added later.
- Can be postponed: No if direct messages exist.
- Notes: Avoid adding unfriend/block until the core request-to-chat path is stable.

## Groups

Feature: Group creation
- Description: A user can create a small group chat.
- Required dependencies: User account, session, conversation model, rate/limit policy.
- Database needs: Group chat row, creator, membership, group count constraint.
- Backend needs: Create group, enforce max groups per user.
- Frontend needs: Create group action and group list entry.
- Realtime needs: Optional group-created event later.
- Security concerns: Group spam, unauthorized membership insertion.
- Can be postponed: Yes.
- Notes: Groups add membership complexity; start after direct chat basics work.

Feature: Group membership management
- Description: Group creator/moderators can add or remove members.
- Required dependencies: Group creation, user directory, authorization.
- Database needs: Group members, joined_at, membership status.
- Backend needs: Add/remove member routes, access checks, history visibility rule.
- Frontend needs: Member list, add/remove controls, member labels.
- Realtime needs: Later notify affected members and update group state.
- Security concerns: Adding unwilling users, removing creator, reading messages before joined_at.
- Can be postponed: Yes.
- Notes: This can make the project messy if started before direct membership rules are solid.

Feature: Group rename/delete
- Description: A group can be renamed or closed/deleted by authorized users.
- Required dependencies: Group creation, authorization.
- Database needs: Group label, deleted/archived state if deletion is soft.
- Backend needs: Rename/delete handlers with permission checks.
- Frontend needs: Group settings panel.
- Realtime needs: Later push group metadata changes.
- Security concerns: Unauthorized destructive changes, audit trail.
- Can be postponed: Yes.
- Notes: Prefer archive/disable semantics over physical deletion if history matters.

## Communities

Feature: Community chat
- Description: A shared chat area visible to all eligible active users.
- Required dependencies: User account, session, conversation model, send message, message history.
- Database needs: Community conversation and messages.
- Backend needs: Read/write access rules for active/suspended/banned users.
- Frontend needs: Community tab/list item and feed.
- Realtime needs: Later `chat.message` broadcast.
- Security concerns: Spam, moderation load, public visibility within the site.
- Can be postponed: No if the site identity is community-first; yes if direct chat is chosen as the first core.
- Notes: Community chat is simpler than group membership but riskier for abuse.

Feature: Community mute
- Description: User can mute community notifications for a period.
- Required dependencies: Community chat, notifications, user account.
- Database needs: Per-user per-chat mute_until.
- Backend needs: Set mute, notification fanout respects mute.
- Frontend needs: Mute control and mute state display.
- Realtime needs: None, except notification behavior.
- Security concerns: Mention override correctness, user expectation around muted content.
- Can be postponed: Yes.
- Notes: Do not build mute before notifications exist.

## Discussion

Feature: Discussion thread/post
- Description: Users can create top-level discussion posts and replies.
- Required dependencies: User account, session, authorization, rate limits.
- Database needs: Discussion posts with thread_id, parent_id, depth, author, title/body, timestamps.
- Backend needs: Create/list posts, enforce max depth, cursor pagination.
- Frontend needs: Discussion list, create form, reply UI, basic thread display.
- Realtime needs: Later emits `discussion.post`.
- Security concerns: Spam, depth abuse, unauthorized writes.
- Can be postponed: Yes.
- Notes: Discussion is a separate product surface; keep it out until chat foundations are stable.

Feature: Discussion voting
- Description: Users can upvote/downvote or undo votes on posts.
- Required dependencies: Discussion thread/post, user account, rate limits.
- Database needs: Votes table with one vote per user per post and computed score.
- Backend needs: Cast/undo vote, enforce rate, list score.
- Frontend needs: Vote controls and score display.
- Realtime needs: Later emits `discussion.vote`.
- Security concerns: Vote spam, duplicate votes, race conditions.
- Can be postponed: Yes.
- Notes: Do not let sorting logic block basic discussion read/write.

Feature: Discussion edit moderation
- Description: Users request edits; admins approve or reject before content changes.
- Required dependencies: Discussion thread/post, moderation/admin, notifications or admin queue.
- Database needs: Edit request table, status, reviewer, timestamps.
- Backend needs: Request edit, list requests, approve/reject transaction.
- Frontend needs: User edit request UI and admin review queue.
- Realtime needs: Later admin event and user notification.
- Security concerns: Unauthorized edits, audit trail, abusive edits, stale approvals.
- Can be postponed: Yes.
- Notes: This is a complexity multiplier and should not be started early.

## Realtime

Feature: WebSocket session
- Description: Authenticated users can open a realtime connection tied to their session.
- Required dependencies: Session and authentication, app shell.
- Database needs: Optional connection/session tracking if presence is required.
- Backend needs: Authenticated WebSocket handshake, connection lifecycle, ping/keepalive.
- Frontend needs: Connect/disconnect state, reconnect behavior.
- Realtime needs: This is the realtime foundation.
- Security concerns: Auth bypass, connection leaks, stale banned users, denial of service.
- Can be postponed: Yes, if HTTP chat/history exists first.
- Notes: Realtime should enhance a working product, not replace reliable history.

Feature: Message delivery events
- Description: New or mutated messages are delivered to connected clients.
- Required dependencies: WebSocket session, send message, message history, conversation membership.
- Database needs: Message records already exist; optional event id.
- Backend needs: Publish after commit, route to authorized subscribers.
- Frontend needs: Idempotent append/update message handling.
- Realtime needs: `chat.message` event with stable schema.
- Security concerns: Broadcasting private messages to wrong users, duplicate delivery.
- Can be postponed: Yes.
- Notes: Requires history/resync to avoid append-only fragility.

Feature: Reconnect and idempotency
- Description: Clients can recover after reconnect without duplicate or missing UI state.
- Required dependencies: WebSocket session, message history, event schema.
- Database needs: Message/event ids, cursor state, updated_at where needed.
- Backend needs: Stable ids, optional event version, resync endpoints or history query.
- Frontend needs: Deduplicate by id, reload missed state after reconnect.
- Realtime needs: Event id/version if event stream grows.
- Security concerns: Replay of inaccessible data, stale authorization after reconnect.
- Can be postponed: Partly; basic dedupe should exist with first realtime delivery.
- Notes: This is one of the highest-risk areas if ignored.

Feature: Presence
- Description: Users can see whether others are online.
- Required dependencies: WebSocket session, user account, privacy decision.
- Database needs: Active connection/session records or ephemeral presence state.
- Backend needs: Track connect/disconnect/last_seen, expose status.
- Frontend needs: Online indicators.
- Realtime needs: Presence updates if live indicators are desired.
- Security concerns: Privacy, stalking risk, stale online state.
- Can be postponed: Yes.
- Notes: Presence is attractive but not required for core messaging.

## Notifications

Feature: Notification model
- Description: The system records user-specific events such as mentions, DM requests, accepted DMs, discussion moderation results, and admin-relevant changes.
- Required dependencies: User account, source event definitions.
- Database needs: Notifications table, unread flag, created_at, optional actor/source references.
- Backend needs: Create/list notifications, mark read if included.
- Frontend needs: Notification panel/list and unread affordance.
- Realtime needs: Later emits `notification.created`.
- Security concerns: Leaking private event content, notification spam.
- Can be postponed: Yes, except DM request visibility if no alternate inbox exists.
- Notes: Start with explicit notification use cases, not a generic everything-feed.

Feature: Unread count
- Description: User can see how many notifications or chats need attention.
- Required dependencies: Notification model or message read state.
- Database needs: Unread flags or per-user read markers.
- Backend needs: Count endpoint or bootstrap field.
- Frontend needs: Badges and clear read transitions.
- Realtime needs: Live count updates later.
- Security concerns: Cross-user count leaks, stale counts.
- Can be postponed: Yes.
- Notes: Badges create state complexity; avoid early unless critical to navigation.

Feature: Mention notifications
- Description: Mentioned users are notified, even when a community is muted.
- Required dependencies: Mentions, notification model, community mute if mute exists.
- Database needs: Mention references and notification rows.
- Backend needs: Mention parse, notification generation, mute override logic.
- Frontend needs: Mention highlights and notification rendering.
- Realtime needs: Optional `notification.created`.
- Security concerns: Mention spam, mute bypass expectations.
- Can be postponed: Yes.
- Notes: Mention notification should wait until mentions and notifications are both stable.

## Moderation

Feature: Admin user approval
- Description: Admins can approve pending registrations or reject/delete them.
- Required dependencies: User account, account status, authorization.
- Database needs: Pending users, status changes, audit trail.
- Backend needs: List pending/users, approve, reject.
- Frontend needs: Admin user list and decision actions.
- Realtime needs: Optional admin events.
- Security concerns: Unauthorized approval, mistaken deletion, auditability.
- Can be postponed: No, if pending registration remains required.
- Notes: This is foundational if the community is invite/approval gated.

Feature: User status moderation
- Description: Admins can suspend or ban users.
- Required dependencies: User account, account status, authorization.
- Database needs: Status and status-change audit fields.
- Backend needs: Change status, enforce status immediately.
- Frontend needs: Admin controls and user-state indicators.
- Realtime needs: Optional session invalidation/status event.
- Security concerns: Banned users keeping sessions, suspended writes slipping through.
- Can be postponed: Partly. Manual DB-only moderation is risky but possible briefly in private testing.
- Notes: Enforcement matters more than a polished admin UI.

Feature: Permission management
- Description: Admins can grant or remove specific capabilities.
- Required dependencies: Authorization and permissions.
- Database needs: Permission definitions and assignments.
- Backend needs: Toggle/list permissions.
- Frontend needs: Admin permission UI.
- Realtime needs: Optional admin event.
- Security concerns: Privilege escalation, overbroad permissions.
- Can be postponed: Yes, if early product has simple admin-only checks.
- Notes: Fine-grained permissions can overcomplicate early rebuild architecture.

Feature: Audit/activity log
- Description: Security- and moderation-relevant actions are recorded.
- Required dependencies: User account, session, source actions.
- Database needs: Activity/audit log records, actor, action, target, timestamps.
- Backend needs: Write audit entries and list them for admins.
- Frontend needs: Admin audit view.
- Realtime needs: None.
- Security concerns: Log tampering, sensitive data in logs, retention.
- Can be postponed: Partly. Logging important actions should start early; admin log UI can wait.
- Notes: Avoid elaborate log partitioning until volume is known.

Feature: Rate config admin
- Description: Admins can view/change rate-limit settings.
- Required dependencies: Rate limits, authorization.
- Database needs: Config rows.
- Backend needs: Read/update config, validation.
- Frontend needs: Admin settings form.
- Realtime needs: Optional admin event.
- Security concerns: Misconfiguration, disabling protections.
- Can be postponed: Yes.
- Notes: Hard-coded or config-file limits are enough before admin tuning is needed.

## Media

Feature: Upload prepare
- Description: Client asks whether a file is allowed before sending content.
- Required dependencies: User account, session, authorization, storage policy.
- Database needs: Pending file metadata, owner, size, MIME, status.
- Backend needs: Validate type/size/permission and issue file id.
- Frontend needs: File picker and preflight error display.
- Realtime needs: None.
- Security concerns: MIME spoofing, size abuse, quota, path traversal, malware policy.
- Can be postponed: Yes.
- Notes: Media should wait until text chat is stable.

Feature: File upload and storage
- Description: User uploads allowed content and can reference/download it.
- Required dependencies: Upload prepare, storage policy, message or post attachment model.
- Database needs: File records, storage path, owner, content type, created_at, visibility.
- Backend needs: Multipart receive, store file, enforce storage root, serve/download safely.
- Frontend needs: Upload progress, attachment rendering, download/open behavior.
- Realtime needs: Optional message event includes attachment metadata.
- Security concerns: Malware, private file access, storage cleanup, quota, backup scope.
- Can be postponed: Yes.
- Notes: This is a classic early-scope trap.

## Search

Feature: User search
- Description: Users can find people by public username.
- Required dependencies: User account, privacy policy.
- Database needs: Indexed public username/search fields.
- Backend needs: Search endpoint with limits.
- Frontend needs: Search input and results.
- Realtime needs: None.
- Security concerns: User enumeration and harassment.
- Can be postponed: Yes.
- Notes: Exact username entry can cover early DM request needs.

Feature: Message search
- Description: Users can search accessible chat history.
- Required dependencies: Message history, conversation access checks.
- Database needs: Search index or queryable message bodies.
- Backend needs: Search endpoint scoped to accessible conversations.
- Frontend needs: Search UI and result navigation.
- Realtime needs: None.
- Security concerns: Private-message leakage, deleted-message behavior.
- Can be postponed: Yes.
- Notes: Do not start before access rules and history are stable.

Feature: Discussion search
- Description: Users can search discussion posts.
- Required dependencies: Discussion posts.
- Database needs: Searchable post fields.
- Backend needs: Search/list endpoint.
- Frontend needs: Search UI in discussion area.
- Realtime needs: None.
- Security concerns: Deleted/hidden post visibility.
- Can be postponed: Yes.
- Notes: Basic sorting and pagination should come first.

## Settings

Feature: Profile/account settings
- Description: User can view and possibly update account-level preferences.
- Required dependencies: User account, session.
- Database needs: User preference fields.
- Backend needs: Read/update settings with validation.
- Frontend needs: Settings page/panel.
- Realtime needs: None.
- Security concerns: Username immutability, password changes, session invalidation.
- Can be postponed: Yes.
- Notes: Decide which fields are mutable before implementation.

Feature: Notification settings
- Description: User controls notification behavior such as community mute and possibly email/push later.
- Required dependencies: Notification model, community mute if included.
- Database needs: Preference/mute rows.
- Backend needs: Read/update preferences.
- Frontend needs: Settings controls.
- Realtime needs: None.
- Security concerns: Mention override clarity, privacy expectations.
- Can be postponed: Yes.
- Notes: Keep separate from core notification list.

Feature: Session/device management
- Description: User can see/logout current or all sessions.
- Required dependencies: Session and authentication.
- Database needs: Session/device metadata.
- Backend needs: Logout current, logout all, optional session list.
- Frontend needs: Account security controls.
- Realtime needs: Optional disconnect sessions.
- Security concerns: Stolen sessions, stale session display.
- Can be postponed: Logout-all should exist; full device list can wait.
- Notes: Session management is security-related but does not need a broad settings UI at first.

## Operations

Feature: Health check
- Description: Operators can verify the app is alive.
- Required dependencies: Runtime app process.
- Database needs: Optional DB connectivity check.
- Backend needs: Health endpoint.
- Frontend needs: None.
- Realtime needs: None.
- Security concerns: Leaking internals in health response.
- Can be postponed: No for deployed environments.
- Notes: Keep simple.

Feature: Structured logging
- Description: App emits useful logs for errors, security events, and operational visibility.
- Required dependencies: Runtime app process, core action definitions.
- Database needs: None unless activity logs are stored in DB.
- Backend needs: Consistent logging for failures and key actions.
- Frontend needs: None.
- Realtime needs: None.
- Security concerns: Logging secrets, tokens, passwords, private message bodies.
- Can be postponed: No for basic logs; advanced dashboards can wait.
- Notes: Logging policy should be decided before sensitive features like media.

Feature: Backup and restore policy
- Description: Data can be backed up and restored with documented retention and access controls.
- Required dependencies: Database model, storage policy if media exists.
- Database needs: Backupable schema, restore assumptions.
- Backend needs: Optional backup-plan endpoint; actual backup job can be external or internal later.
- Frontend needs: Admin visibility optional.
- Realtime needs: None.
- Security concerns: Backup exposure, private messages in dumps, retention, encryption, restore testing.
- Can be postponed: Backup UI can wait; policy cannot wait for production.
- Notes: Do not store production backup artifacts in the source repository.

Feature: Deployment and secret management notes
- Description: Operators know how to run the site safely with TLS, reverse proxy, and secrets.
- Required dependencies: Chosen deployment target.
- Database needs: Connection configuration.
- Backend needs: Environment/config validation.
- Frontend needs: None.
- Realtime needs: Proxy support for WebSocket.
- Security concerns: TLS, cookie security, secret leakage, captcha secret, database credentials.
- Can be postponed: No for production; yes for local-only prototype.
- Notes: This is documentation and configuration discipline, not app feature work.

Feature: API contract documentation
- Description: Product/backend/frontend share a clear contract for payloads, errors, and event schemas.
- Required dependencies: Stable feature definitions.
- Database needs: None.
- Backend needs: Request/response schema clarity.
- Frontend needs: Reliable integration expectations.
- Realtime needs: Event schema documentation.
- Security concerns: Documenting sensitive fields incorrectly.
- Can be postponed: Partly. Write only the contract needed for active features.
- Notes: Avoid full OpenAPI churn before product boundaries settle.

## Later / Optional

Feature: Captcha production provider
- Description: Registration/gate flow uses a real bot-protection provider.
- Required dependencies: Account registration, deployment config, owner choice of provider.
- Database needs: None directly.
- Backend needs: Server-side verification, failure handling, configurable secret.
- Frontend needs: Captcha widget and fallback error state.
- Realtime needs: None.
- Security concerns: Bot protection, provider secret, privacy/tracking tradeoffs.
- Can be postponed: Yes for local rebuild, no for public production.
- Notes: Owner must choose provider before implementation.

Feature: Polls
- Description: Discussion posts can include poll-like interactions.
- Required dependencies: Discussion posts, voting or separate poll model.
- Database needs: Poll questions/options/votes.
- Backend needs: Poll create/vote/list.
- Frontend needs: Poll rendering and controls.
- Realtime needs: Optional live result updates.
- Security concerns: Vote integrity, duplicate voting.
- Can be postponed: Yes.
- Notes: Legacy docs call this skeleton/future-ready only.

Feature: Advanced admin dashboard
- Description: Admins get rich operational views beyond necessary moderation actions.
- Required dependencies: Core moderation, audit logs, metrics decisions.
- Database needs: Aggregates or query support.
- Backend needs: Dashboard endpoints.
- Frontend needs: Admin dashboard screens.
- Realtime needs: Optional live admin events.
- Security concerns: Exposing private operational data.
- Can be postponed: Yes.
- Notes: Build task-specific admin tools before dashboards.

Feature: Full-text/global search
- Description: One search surface spans users, messages, discussions, and files.
- Required dependencies: User search, message search, discussion search, permission model.
- Database needs: Search indexes and scoped query strategy.
- Backend needs: Federated/scoped search.
- Frontend needs: Global search UI.
- Realtime needs: None.
- Security concerns: Severe cross-scope leakage risk.
- Can be postponed: Yes.
- Notes: This should be much later than scoped search.

Feature: Media scanning/quota automation
- Description: Uploaded media is scanned, quota-managed, and cleaned up automatically.
- Required dependencies: Media upload/storage, operations policy.
- Database needs: Scan status, quota counters, retention state.
- Backend needs: Scan pipeline integration and cleanup jobs.
- Frontend needs: Upload status and failure states.
- Realtime needs: Optional scan-complete event.
- Security concerns: Malware, privacy, false positives, storage abuse.
- Can be postponed: Yes, unless public media upload is enabled.
- Notes: Do not enable broad media uploads without at least a clear manual policy.

## A. Dependency Chain

Primary chain:

User account -> Account status -> Session -> Auth guard -> App shell -> Conversation -> Message send -> Message history -> Message mutations -> WebSocket delivery -> Reconnect/resync

Direct-message chain:

User account -> User lookup -> DM request -> Accept/reject -> Friendship/direct membership -> Direct conversation -> Direct message

Group chain:

User account -> Conversation model -> Group creation -> Group membership -> Group message visibility -> Group metadata actions

Community chain:

User account -> Conversation model -> Community conversation -> Community message -> Community mute -> Mention override notification

Discussion chain:

User account -> Discussion post -> Discussion list/history -> Reply depth rules -> Vote -> Sorting -> Edit moderation

Notification chain:

Source action -> Notification record -> Notification list -> Unread count -> Realtime notification -> Settings/mute behavior

Media chain:

Storage policy -> Upload prepare -> Upload content -> File record -> Attachment reference -> File access/download -> Cleanup/retention

Operations chain:

Config -> Health -> Logging -> Backup policy -> Restore policy -> Deployment/TLS -> Production readiness

## B. First Sensible Build Order

This is a practical dependency order, not a release plan:

1. Product rules: decide account status behavior, username mutability, direct-message gating, and production/private operating assumptions.
2. User account and session model.
3. Auth guard and authenticated app shell.
4. Basic authorization/admin guard.
5. Conversation model with the smallest useful chat type.
6. Text message send.
7. Message history with cursor/limit.
8. Basic chat UI that can load history and send messages without realtime.
9. DM request and accept/reject if direct messaging is required early.
10. Admin approval/status enforcement if pending registration remains required.
11. Basic rate limits for login, messages, DM requests, and votes when added.
12. WebSocket authenticated connection.
13. Message delivery events with idempotent client handling.
14. Reconnect/resync behavior using history.
15. Notifications only for concrete events already present, such as DM requests or mentions.
16. Groups after direct membership rules are proven.
17. Community mute and mention override after notifications exist.
18. Discussion posts, then voting, then moderation.
19. Media upload after text/chat security and storage policy are clear.
20. Search, advanced settings, advanced dashboards, and optional poll/media automation last.

## C. Features That Must NOT Be Started Early

- Media upload: It pulls in storage policy, MIME validation, quota, privacy, cleanup, scanning, backup scope, and file-serving security before the core chat product is proven.
- Discussion edit moderation: It creates admin queues, approval state, audit requirements, notifications, and realtime edge cases before basic discussions exist.
- Fine-grained permission management: It can overfit the admin model before real moderation tasks are clear.
- Global search: It is high-risk for privacy leaks and depends on stable access rules for users, messages, discussions, and files.
- Advanced presence: It creates privacy and stale-state issues and is not needed for sending or reading messages.
- Realtime before history: WebSocket delivery without reliable history/resync causes missed messages, duplicates, and fragile UI state.
- Admin dashboard polish: It tends to expand scope into metrics, charts, live events, and configuration before core moderation workflows are stable.
- Configurable rate-limit UI: Rate limits are needed early, but changing them in an admin panel can wait.
- Production captcha integration before owner decision: It requires choosing a provider and accepting provider-specific privacy, UX, and operations tradeoffs.
- Polls: Legacy docs identify this as future-ready; it should not compete with chat/discussion basics.

## D. Open Product Questions

- Should registration remain approval-gated, or should users become active immediately?
- Is the site private/community-only, public internet-facing, or invite-only?
- Which chat type should prove the rebuild first: direct, group, or community?
- Must direct messaging require accepted friendship, or can users message directly by username?
- Should usernames be immutable?
- Are `login_username` and public display username separate concepts in the rebuild?
- What is the password policy owner wants, given the legacy `a-z0-9` policy is weak?
- Should suspended users be allowed to read and connect realtime, or fully blocked?
- Should deleted message bodies be permanently discarded, retained for audit, or retained only for admin review?
- Should message edit/delete be included early, or postponed until basic chat stabilizes?
- What is the expected behavior for users added to a group: can they see prior messages or only messages after join?
- Is community chat required at the start, or can it wait until moderation rules are clearer?
- Which notification events are essential: DM requests, mentions, admin decisions, discussion replies, or all of them?
- Should WebSocket events include event ids and schema versions from the first realtime implementation?
- Is discussion a core site feature or a later expansion after chat?
- Should voting include downvotes, upvotes only, or reactions?
- Who can moderate discussions and users on day one?
- Is fine-grained permission management required, or is a simple admin capability enough initially?
- Should file uploads exist at all in the rebuild's first usable product?
- If media uploads are kept, what file types, size limits, quota, and malware-scanning expectations apply?
- What retention policy applies to messages, logs, backups, uploaded files, and deleted content?
- What production deployment target should be assumed before architecture work begins?
- Which legacy UI is the product reference, if any: `legacy/web/app.html`, `legacy/web/raw-app.html`, `legacy/UI/App.html`, or `legacy/UI/App-Fixed.html`?
- Should the API contract keep an envelope format, or should a new response contract be defined?
