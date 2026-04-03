# API & WebSocket Guide (Concise)

Bu döküman frontend geliştiricileri ve entegrasyon ekipleri için kısa, uygulanabilir bir referans sağlar. Amaç: hangi HTTP endpoint'lerinin ve WebSocket event'lerinin mevcut olduğunu hızlıca görmek ve frontend entegrasyonunda dikkat edilmesi gereken temel kuralları hatırlatmak.

---

## Hızlı Özet
- Base HTTP API prefix: `/api/*`  
- WebSocket endpoint: `GET /ws` (session cookie ile bağlanır)  
- API yanıtları `ApiEnvelope` içinde döner — önce HTTP status, sonra `result.ok` ve `result.body.ok` kontrolü yapılmalı.  
- Frontend helper'ları: `Melt.api`, `Melt.startPresenceSocket`, `Melt.onWebSocketEvent`, `Melt.uploadFile`.

---

## Önemli HTTP Endpoint'leri (kısa)
- Authentication / Session
  - `POST /api/captcha/verify`
  - `POST /api/auth/login`
  - `POST /api/auth/register`
  - `POST /api/auth/reset`
  - `POST /api/auth/pin`
  - `POST /api/auth/logout`
  - `POST /api/auth/logout-all`

- Bootstrap / Health
  - `GET /api/bootstrap/state`
  - `GET /health`

- Chat & Messaging
  - `POST /api/chats/message` — Gönderme (rate: 1/msg per 3s)
  - `POST /api/chats/message/edit` — Sender-only, <= 300s
  - `POST /api/chats/message/delete` — Soft delete (body -> "Mesaj silindi")
  - `GET /api/chats/message/history`
  - `POST /api/chats/dm-request`
  - `GET /api/chats/dm-request/list`
  - `POST /api/chats/dm-request/accept`
  - `POST /api/chats/dm-request/reject`
  - `POST /api/chats/group` and related group member endpoints

- Discussions
  - `POST /api/discussion/post`
  - `POST /api/discussion/vote`
  - `GET /api/discussion/list?sort=&cursor=`
  - `POST /api/discussion/edit-request`

- Files
  - `POST /api/files/prepare`
  - `POST /api/files/upload` (multipart/form-data)

- Notifications & Presence
  - `GET /api/notifications/list?user_id=&cursor=&limit=`
  - `GET /api/notifications/unread`
  - `POST /api/community/mute`
  - `GET /api/presence/online`

- Admin
  - `GET /api/admin/overview`
  - `GET /api/admin/users`
  - `POST /api/admin/users/approve`
  - `POST /api/admin/users/reject`
  - `POST /api/admin/users/status`
  - `POST /api/admin/users/permission`
  - `GET /api/admin/rate-config` / `POST /api/admin/rate-config`
  - `GET /api/admin/audit`
  - `GET /api/admin/discussion/edits`
  - `POST /api/admin/discussion/edits/decide`

- Operations
  - `GET /api/backup/plan`

---

## WebSocket - Olaylar ve Beklenen Davranış
- Bağlantı noktası: `GET /ws` — sunucu HttpOnly signed session cookie kontrolü yapar.
- Ana event tipleri:
  - `chat.message`
  - `discussion.post`
  - `discussion.vote`
  - `notification.created`
  - `admin.event`
- İstemci davranışı:
  - `Melt.startPresenceSocket(elementId)` ile bağlantı yönetimi (ping, backoff, reconnect).
  - `Melt.onWebSocketEvent` ile event'leri dinle; `payload.type` ve gerekli kimlikleri kontrol etmeden UI'ı değiştirme.
  - UI güncellemeleri idempotent olmalı (aynı event tekrar gelirse çoğaltma).

Örnek `chat.message` payload:
```/dev/null/example.json#L1-14
{
  "type": "chat.message",
  "payload": {
    "chat_id": "<uuid>",
    "chat_type": 1,
    "message": {
      "id": "<uuid>",
      "body": "Merhaba dünya",
      "sender_id": "<uuid>",
      "created_at": "2026-02-16T12:00:00Z",
      "edited_at": null,
      "deleted": false,
      "mentions": []
    }
  }
}
```

---

## Frontend Entegrasyon Kuralları (pratik)
- Tüm fetch çağrılarında `credentials: "same-origin"` kullanılmalı (helper: `Melt.api`).
- İlk açılış: `GET /api/bootstrap/state`. Eğer 401/403 gelirse `window.location.href = "/index.html"` ile yönlendir.
- Api yanıtı kontrol sırası:
  1. HTTP status (401/403 treat as auth failure)  
  2. `result.ok`  
  3. `result.body.ok`  
  4. `result.body.data` kullan
- Hata gösterimi: `Melt.toast` veya `Melt.showNotice`.
- Mesaj gönderme UX:
  - Gönderme sırasında optimistic UI gösterimi yapılabilir; fakat sunucu onayı veya WS broadcast'ı gelince kesinleştir.
  - Gönderme rate-limit (1/msg per 3s) aşıldığında backend 429 veya uygun hata dönecek; frontend bunu kullanıcı dostu şekilde yakalamalı.
- Dosya yükleme:
  - Önce `POST /api/files/prepare` çağır, `file_id` al, sonra `Melt.uploadFile(file, { user_id, file_id })` kullan.
- WebSocket reconnect sonrası:
  - Kısa veri yeniden senkronizasyonu (ör. unread counts veya tartışma cursor) gerekebilir; reconnect politika­sı dokümante edilmeli.

---

## Kısıtlar & Önemli Notlar
- Mesaj düzenleme sadece göndericiye ve 300s ile sınırlı.
- Discussion reply depth <= 3.
- DM request: çift yönlü bekleyen istek varsa yeni istek 409 döner.
- `files/upload` server-side `storage_root` altında dosya yazar; path traversal ve MIME/size kontrolleri backend tarafından yapılmalı.
- EKSIK: production captcha doğrulaması (hCaptcha/recaptcha) ve deployment (Caddy/TLS) konuları hâlâ açık — bu iki konu ops/infra adımlarında çözülmeli.

---

## Öneriler (kısa)
- API için minimal OpenAPI/Swagger şeması oluşturulması frontend-backend uyumunu hızlandırır.
- WebSocket event'lerine `event_id` ve (opsiyonel) `version` eklenmesi idempotency ve schema migration'ı kolaylaştırır.
- `ApiEnvelope` örnekleri ve ortak hata kodları (`401`, `403`, `409`, `429`) bir referans dosyasında toplanmalı.

---

Bu döküman kısa, okunabilir ve frontend geliştiricinin hızlıca entegrasyon yapmasına yetmelidir. Daha ayrıntılı örnek request/response blokları, OpenAPI çıktısı veya deployment talimatlarını istersen bir sonraki adım olarak ekleyebilirim.