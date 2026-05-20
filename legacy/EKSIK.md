# EKSIK.md

Bu dosya SPECS’teki açık maddeleri listeler. Şu anda yalnızca 2.1 (gerçek captcha) ve 2.7 (Caddy/TLS deployment) kalıyor; diğer tüm senaryolar uygulamaya taşındı.

## 1. Tamamlanan maddeler (2.2–2.10)
1. **WebSocket pub/sub + kontrol:** `WebSocketBroadcaster` artık `chat.message`, `discussion.post`, `discussion.vote`, `notification.created` ve `admin.event` olaylarını yayınlıyor, `Melt.onWebSocketEvent` dinleyicileri app/admin tarafında aktifleştirildi.
2. **Dosya upload uç noktası:** `/api/files/prepare` metadata kontrolü yapıyor, `/api/files/upload` multipart isteğiyle içerik yüklüyor, disk yolu `storage_root` altına yazılıyor, UI `Melt.uploadFile` helper’ı kullanıyor.
3. **Discussion edit moderasyonu:** Kullanıcılar `/api/discussion/edit-request` ile düzenleme talebi oluşturabiliyor, `admin` panelinde talepler listeleniyor ve `/api/admin/discussion/edits/decide` ile onay/red yapılıyor; WebSocket yayınları admin panelini canlı güncelliyor.
4. **Cursor pagination:** `discussion::list_posts_handler` ve `notifications::list_notifications_handler` `cursor`/`limit` parametreleri alıyor, `CursorPage` döndürüyor, frontend `state.*Cursor` ile takibi yapıyor.
5. **Backup scheduler:** Yeni `BackupService` 12 saatte bir `pg_dump` çalıştırıyor, tablo başına `.sql` dosyalarını üretip zip’lemeye çalışıyor ve `/api/backup/plan` artık gerçek planı döndürüyor.
6. **UI/UX iyileştirmeleri:** `app.html` içinde olay bazlı refresh, daha kullanıcı dostu bildirimler, gerçek zamanlı WebSocket dinlemeleri ve dosya yükleme akışı; `admin.html` içinde edit talepleri tablosu, gerçek zamanlı admin event handling.
7. **Otomaik veritabanı oluşturma:** `Database::ensure_database_exists` `DATABASE_URL` tarafından açıklanan veritabanı yoksa `postgres` bazında yaratıyor; `Cargo.lock` ve env dosyasında `axum` multipart desteği eklendi.
8. **Mesaj düzenleme/silme:** `/api/chats/message/edit` ve `/api/chats/message/delete` eklendi; 300 sn içinde edit, soft delete “Mesaj silindi” metniyle; WebSocket `chat.message` DTO’su `edited_at` ve `deleted` alanlarını taşıyor.

## 2. Hâlâ açık kalan maddeler
1. **2.1 – Gerçek captcha entegrasyonu:** `auth::verify_captcha_handler` hâlâ sadece `CAPTCHA_DEV_SECRET`’ı kontrol ediyor; hCaptcha/reCAPTCHA server-side doğrulaması eklenmeli.
2. **2.7 – Deployment/Caddy/TLS:** Repo hâlâ Caddyfile, systemd servisi veya TLS otomasyonu içermiyor; bu bölümler prod dağıtım sırasında dışarıda bırakıldı.
