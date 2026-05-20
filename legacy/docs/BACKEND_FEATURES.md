# Novastrum — Backend Özellikleri & API + Frontend Entegrasyon Özeti

Bu döküman backend tarafından sağlanan özelliklerin, HTTP API uç noktalarının ve WebSocket olaylarının özetini ile frontend entegrasyon notlarını içerir. Amaç, frontend geliştiricilerin hangi endpoint'leri, payload formatlarını ve çalışma şekillerini beklemesi gerektiğini açıkça belirtmektir.

---

## Genel mimari özet
- HTTP API: JSON ApiEnvelope sarmalayıcısı kullanır; başarılı/başarısız sonuçlar `result.ok` ve `result.body.ok` ile kontrol edilir.
- Oturum: HttpOnly, signed cookie ile yönetilir. Frontend `fetch` çağrılarında `credentials: "same-origin"` kullanmalıdır.
- WebSocket: Tek bağlantı noktası `/ws`. Bağlantı yalnızca imzalı (authenticated) oturumlara izin verir. Sunucu yayınları (pub/sub) aracılığıyla gerçek zamanlı olaylar gönderilir.
- Statik sayfalar: `index.html` (captcha + giriş/ yönlendirme), `app.html` (kullanıcı arayüzü), `admin.html` (yönetici paneli).

---

## Önemli HTTP endpoint'leri (kısa liste)

Not: Aşağıdaki endpoint isimleri ve yöntemleri router tanımlarından derlenmiştir (`src/main.rs` + dokümantasyon).

Authentication / Session
- `POST /api/captcha/verify`  
  - Captcha token doğrulama (dev secret örneği: `i-am-human`)  
  - Dönen payload: `next_page` (ör. `app.html` veya `auth.html`)
- `POST /api/auth/login`  
  - Body: `{ login_username, password, device_label? }`  
  - Başarı: HttpOnly `Set-Cookie` ile oturum oluşturur.
- `POST /api/auth/register`  
- `POST /api/auth/reset`  
- `POST /api/auth/pin`  
- `POST /api/auth/logout`  
- `POST /api/auth/logout-all`  

Bootstrap / Health
- `GET /api/bootstrap/state`  
  - Döner: `{ current_user_id: string|null, users: [...], chats: [...] }`  
  - 401/403 durumunda frontend `index.html`'e yönlendirir.
- `GET /health` — sağlık kontrolü

Chat & Messaging
- `POST /api/chats/message`  
  - Gönderme: `{ user_id, chat_id, chat_type, body }`  
  - Rate limit: 1 mesaj / 3s
- `POST /api/chats/message/edit`  
  - Sadece gönderici, gönderimden sonraki 300s içinde düzenleme. Payload: `{ user_id, message_id, chat_type, new_body }`
  - `edited_at` alanı broadcast edilir.
- `POST /api/chats/message/delete`  
  - Sadece gönderici; soft delete. Body `"Mesaj silindi"`, `deleted=true`, `mentions=[]`.
- `GET /api/chats/message/history` — (mesaj geçmişi, sayfalama vs.)

DM / Group flows
- `POST /api/chats/dm-request`  
  - `{ from_user_id, to_user_id }` — eğer her iki yönde bekleyen istek varsa yeni istek 409 ile reddedilir.
- `GET /api/chats/dm-request/list`  
- `POST /api/chats/dm-request/accept`  
  - `{ user_id, request_id }` — kabul => arkadaşlık + DM chat oluşturulur.
- `POST /api/chats/dm-request/reject`
- `POST /api/chats/group` — grup oluşturma (limit: kullanıcı başına max 3)
- `POST /api/chats/group/rename`
- `POST /api/chats/group/member/add`
- `POST /api/chats/group/member/remove`
- `POST /api/chats/group/delete`

Discussion (Forum benzeri)
- `POST /api/discussion/post`  
  - `{ user_id, thread_id?, parent_id?, title, body }`  
  - Ağaç derinliği <= 3
- `POST /api/discussion/vote`  
  - `{ user_id, post_id, value(-1|1), undo? }`  
  - Rate: ~1 vote / dakika (dokümante edildi)
- `GET /api/discussion/list?sort=hot|top|new&cursor=`  
  - Dönüş: `{ items: DiscussionPost[], next_cursor }` (cursor pagination)

Files
- `POST /api/files/prepare`  
  - Metadata doğrulaması (MIME, boyut, permission) ve `file_id` üretimi
- `POST /api/files/upload` (multipart/form-data)  
  - `user_id`, `file_id` ve binary içeriği alır; disk altına `storage_root` altında kaydeder.

Notifications & Presence
- `GET /api/notifications/list?user_id=&cursor=&limit=`  
  - Cursor/limit ile sayfalama, kronolojik cursor mantığı.
- `GET /api/notifications/unread` (veya benzeri) — unread count endpoint
- `POST /api/community/mute`  
  - `{ user_id, chat_id, mute_minutes }` — mention'lar mute'u geçersiz kılar.
- `GET /api/presence/online` — kullanıcı/üye online durumları

Admin
- `GET /api/admin/overview`
- `GET /api/admin/users`
- `POST /api/admin/users/approve`
- `POST /api/admin/users/reject`
- `POST /api/admin/users/status`
- `POST /api/admin/users/permission` — toggle
- `GET /api/admin/rate-config` / `POST /api/admin/rate-config`
- `GET /api/admin/audit`
- `GET /api/admin/discussion/edits` — list edit requests
- `POST /api/admin/discussion/edits/decide` — approve/reject edit requests

Backup & Ops
- `GET /api/backup/plan` — backup plan (scheduler: `BackupService` ~12 saatte bir `pg_dump` çalıştırır, tablo başına `.sql` üretir).
- Otomatik veritabanı oluşturma: `Database::ensure_database_exists(DATABASE_URL)` (ops içi kolay kurulum).

---

## WebSocket — Olay tipi & akışı
- Bağlantı: `ws://<host>/ws` (HTTP oturum cookie'si gönderilerek; yalnızca authenticated oturumlar bağlanabilir).
- Yayınlanan önemli olaylar:
  - `chat.message` — `{ chat_id, chat_type, message: ChatMessageDTO }`  
    - `edited_at`, `deleted`, `mentions` gibi alanları içerebilir.
  - `discussion.post`
  - `discussion.vote` — `{ post_id, score }`
  - `notification.created` — yeni bildirimler
  - `admin.event` — admin panel güncellemeleri
- Client tarafı:
  - `Melt.startPresenceSocket(elementId)` — bağlantı yönetimi, ping, exponential backoff reconnection.
  - `Melt.onWebSocketEvent` — event listener map. Frontend bu handler'ları genişleterek UI güncellemesi yapar.
- Öneri: WebSocket payload'larında `payload.type`, `payload.chat_id` kontrolü yapılmalı; idempotent UI güncellemeleri tercih edilmeli (aynı yayının birden fazla kez gelmesi durumunda çoğaltmama).

---

## Frontend entegrasyon notları (Kısa rehber)
- Başlangıç akışı:
  1. `index.html` üzerinde captcha: `POST /api/captcha/verify` → `next_page` ile `app.html` veya `auth.html`.
  2. `app.html` ve `admin.html` açıldığında `Melt.startPresenceSocket` çalıştırılmalıdır.
  3. Tüm API çağrılarında `Melt.api(path, method, payload)` yardımcı fonksiyonunu kullan; bu helper `credentials: "same-origin"` kullanır, ApiEnvelope yapılarını işler.
- Oturum yönetimi:
  - Sunucu `Set-Cookie` ile HttpOnly oturum başlatır; frontend cookie'ye erişemese de istekler cookie ile gönderilir.
  - 401 / 403 durumunda frontend `index.html`'e yönlendirme uygulamalı.
- WebSocket:
  - `Melt.onWebSocketEvent(type, handler)` ile olayları dinle. Örnek: `discussion.post` geldiğinde tartışma listesi yenilenir.
- Dosya yükleme:
  - Aşamalar: `POST /api/files/prepare` → sunucudan `file_id` al → `Melt.uploadFile(file, { user_id, file_id })` helper'ı kullanarak `multipart/form-data` ile `POST /api/files/upload`.
  - `files/prepare` MIME ve boyut kontrolü yapar; frontend göstergeyi buna göre günceller.
- Pagination:
  - `discussion::list_posts_handler` ve `notifications::list_notifications_handler` cursor/limit destekler. Frontend `state.*Cursor` ile sonraki sayfaları ister.
- Hata yönetimi:
  - API'ler ApiEnvelope döndürür. Frontend önce HTTP status kontrolü (401/403 => redirect), sonra `result.ok` ve `result.body.ok` kontrollerini yapmalı. Hata mesajları için `Melt.toast`/`Melt.showNotice` kullanılmalı.
- Helper'lar (mevcut):
  - `web/melt.js` içinde: `Melt.api`, `Melt.uploadFile`, `Melt.highlightMentions`, `Melt.toast`, `Melt.showNotice/hideNotice`.

---

## Kısıtlar, kurallar ve rate-limitler (belirtilenler)
- Mesaj gönderme: 1 mesaj / 3 saniye
- Mesaj düzenleme: gönderici tarafından, oluşturulmadan sonraki 300 saniye içerisinde izinli
- Discussion vote rate: ~1 oy / dakika
- Discussion reply depth <= 3
- Grup sayısı: kullanıcı başına max 3 grup
- DM request: aynı yön veya karşı yönünde bekleyen istek varsa yeni istek 409 ile reddedilir
- Mute: community mute uygulanır; mention olursa mute override edilir

---

## Veri ve paging
- Cursor-based pagination kullanılır (örn. `discussion/list` ve `notifications/list`).
- Chat history endpoint mevcut fakat frontend için ana mesaj akışı WebSocket + gönderme cevapları ile çalışır (ilk bootstrap'ta geçmiş özet alınabilir).

---

## Admin iş akışları
- Yönetici için özel uç noktalar: kullanıcı onaylama/reddetme, izin toggle'ları, rate-config görüntü/güncelleme, audit logları, tartışma edit taleplerinin listeleme/karar verme.
- Discussion edit moderasyonu: kullanıcılar `POST /api/discussion/edit-request` ile talep açar; admin `GET /api/admin/discussion/edits` ve `POST /api/admin/discussion/edits/decide` ile karar verir. Bu olaylar WebSocket ile admin UI'ya canlı yansır.

---

## Operasyonel notlar
- BackupService: 12 saatte bir `pg_dump`, tablo başına `.sql` dosyaları üretme ve zip'leme denemesi; `GET /api/backup/plan` endpoint'i plan bilgisini döner.
- Uygulama başlatılırken `Database::ensure_database_exists` ile DATABASE_URL’de belirtilen DB yoksa oluşturuluyor (lokal geliştirici kolaylığı).
- Dosyalar disk üzerine `storage_root` altında tutuluyor; `files/upload` endpoint buna yazıyor.

---

## Bilinen eksikler / geliştirme önerileri (kaynaklarda işaretlenmiş)
- Presence ve unread counter'ların raw UI'ya daha iyi entegrasyonu (TASKLIST.md içinde önerildi).
- Raw UI'da file upload UI akışının tam entegrasyonu hâlâ kısmi.
- WebSocket event seti gerektiğinde genişletilmeli; idempotency ve istemci tarafı yeniden dengelemeleri gözden geçirilmeli.

---

## Hızlı referans — sık kullanılan endpoint'ler
- `GET /api/bootstrap/state`
- `POST /api/chats/message`
- `POST /api/chats/message/edit`
- `POST /api/chats/message/delete`
- `POST /api/chats/dm-request`
- `POST /api/chats/group`
- `POST /api/discussion/post`
- `POST /api/discussion/vote`
- `POST /api/files/prepare`
- `POST /api/files/upload`
- `GET /api/notifications/list`
- `GET /api/backup/plan`
- `GET /ws` (WebSocket)

---

Bu döküman, mevcut kaynaklardaki (`API_WEBSOCKET_GUIDE.md`, `DESIGN_BRIEF.md`, `SPECS.md`, `EKSIK.md`, router tanımları vb.) bilgiler temel alınarak hazırlanmıştır. Yeni uç nokta eklemeleri, payload değişiklikleri veya davranış güncellemeleri yapıldıysa bu döküman güncellenmelidir. Frontend geliştirenlere shortcut: `web/melt.js` içindeki helper'lar ve `Melt.startPresenceSocket` + `Melt.onWebSocketEvent` kombinasyonu, entegrasyonun en hızlı yoludur.