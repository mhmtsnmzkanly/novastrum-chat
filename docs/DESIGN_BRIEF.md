# Novastrum — Design Brief (Condensed)

Kapsam
- Tek sayfa (SPA) UI: DM/Group/Community chat, tartışma (threads), bildirimler, DM istek akışı, yönetici paneli için temel görüntüleme.
- Mevcut backend API'leri ve WebSocket event'lerini kullan; backend kontratlarını değiştirme.

Hızlı Başlangıç
- `GET /api/bootstrap/state` → `{ current_user_id, users[], chats[] }`. 401/403 → `/index.html`.
- Oturum: HttpOnly signed cookie. İsteklerde `credentials: "same-origin"` (kullan: `Melt.api`).

Ana Kullanıcı Akışları
- Chat
  - Gönder: `POST /api/chats/message` { user_id, chat_id, chat_type, body } (rate: 1/msg/3s).
  - Düzenle: `POST /api/chats/message/edit` (sender-only, <=300s).
  - Sil: `POST /api/chats/message/delete` (soft delete; body = "Mesaj silindi").
  - Geçmiş: `GET /api/chats/message/history`.
  - DM request / group: `POST /api/chats/dm-request`, `GET /api/chats/dm-request/list`, accept/reject, `POST /api/chats/group` + member endpoints.

- Discussions
  - `POST /api/discussion/post`, `POST /api/discussion/vote`, `GET /api/discussion/list?sort=&cursor=`.
  - Depth <= 3; vote rate ≈ 1/dk.

- Files
  - Prepare: `POST /api/files/prepare` → al metadata, file_id.
  - Upload: `POST /api/files/upload` (multipart). Helper: `Melt.uploadFile`.

- Notifications & Presence
  - `GET /api/notifications/list?user_id=&cursor=&limit=`
  - `POST /api/community/mute`, `GET /api/presence/online`

- Admin & Ops
  - `/api/admin/*` (overview, users, rate-config, audit, discussion edits), `GET /api/backup/plan`.

WebSocket (Gerçek zamanlı)
- Bağlantı: `GET /ws` (session cookie). Event tipleri: `chat.message`, `discussion.post`, `discussion.vote`, `notification.created`, `admin.event`.
- İstemci: `Melt.startPresenceSocket`, `Melt.onWebSocketEvent`.
- Kural: event'ler idempotent işlenmeli; payload alanları (type, chat_id, message) doğrulanmalı.

Veri Şeması (frontend-relevant, kısa)
- ChatMessage: `{ id, chat_id, chat_type, sender_id, body, mentions[], deleted, created_at, edited_at? }`
- ChatRoom: `{ id, chat_type, created_by, label? }`
- DiscussionPost: `{ id, thread_id?, parent_id?, depth, author_id, title?, body, created_at }`
- NotificationItem: `{ id, user_id, chat_id?, kind, body, unread, created_at }`

UX / Güvenlik Notları
- 401/403 → redirect. Diğer hatalar → `Melt.toast`/notice.
- Mesaj gönderiminde optimistic UI kullanılabilir; sunucu onayı veya WS broadcast ile kesinleştir.
- DM istekleri için duplicate kontrolü backend tarafında var (409). Frontend hata durumunu kullanıcıya anlamlı göster.
- Dosya yükleme: önce `prepare`, sonra `upload`. MIME/size kontrolleri backend tarafından yapılır.

Kısaltılmış Gereksinimler / Açık Maddeler
- Production captcha doğrulaması (hCaptcha/recaptcha) gerekli.
- Deployment/TLS (Caddy/systemd) ve backup restore prosedürleri dokümante edilmeli.
- Öneri: OpenAPI / kısa request/response örnekleri ekle; WebSocket event'lerine `event_id`/version eklemesi değerlendirilsin.

Minimum UI State
- `currentUserId`, `users`, `chats`, `dmRequests`, `messagesByChatId`, `discussionItems`, `discussionCursor`.

Bu özet, öncelikli frontend entegrasyon kararlarını hızla almak ve geliştirmeyi başlatmak için yeterli olmalıdır. Detay (örnek JSON, OpenAPI, deployment playbook) istersen sırayla ekleyebilirim.