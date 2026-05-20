# EKSIK — Kısa ve Güncel Özet

Bu dosya, projedeki eksik (veya dikkat gerektiren) maddeleri kısa, önceliklendirilmiş biçimde listeler. Amaç geliştiricilerin ve operasyon ekiplerinin hızlıca hangi konuların hala açık olduğunu görmesi ve bir sonraki adımı seçebilmesidir.

## Durum (kısa)
- Pek çok SPECS maddesi implemente edildi: WebSocket pub/sub, dosya yükleme akışı, discussion edit moderasyonu, cursor pagination, backup scheduler, otomatik DB oluşturma, mesaj düzenleme/silme vb.
- Mevcut eksikler aşağıda listelenmiştir; önceliklendirme ve önerilen çözüm adımları eklenmiştir.

## Açık Maddeler (Önceliklendirilmiş)

1) 2.1 — Production captcha doğrulaması (Yüksek)
- Mevcut: `auth::verify_captcha_handler` geliştirme modunda `CAPTCHA_DEV_SECRET` ile çalışıyor.
- Risk: Bot/spam koruması yetersiz; prod ortamında gerçek captcha doğrulaması yok.
- Öneri: hCaptcha veya reCAPTCHA entegrasyonu ekle — sunucu tarafı doğrulama, configurable secret via env, başarısız doğrulama için uygun hata dönüşü (HTTP 400/401).

2) 2.7 — Deployment / TLS / Reverse proxy (Yüksek-Orta)
- Mevcut: Repo içinde Caddyfile, systemd unit veya TLS otomasyon playbook yok.
- Risk: Prod dağıtım adımları belirsiz; TLS/HTTP proxy konfigürasyonu eksik.
- Öneri: Basit Caddyfile örneği, systemd servis şablonu ve / veya Docker + Traefik/Caddy örneği ekle. Ayrıca environment vars ve secret yönetimi (DATABASE_URL, COOKIE_SECRET, CAPTCHA_SECRET vb.) için doküman hazırla.

3) Ops: Backup restore ve retention açıklığı (Orta)
- Mevcut: `BackupService` düzenli `pg_dump` üretiyor; dump dosyaları repo/dump dizininde örneklerle var.
- Risk: Restore adımları, retention politikası ve erişim kontrolü dokümante değil.
- Öneri: Restore playbook, retention (kaç gün), şifreleme/erişim, ve otomatik temizleme politika­sı ekle.

4) WebSocket event versioning / idempotency (Orta)
- Mevcut: `chat.message`, `discussion.post` vb. event'ler var; ancak event_id/versiyon yok.
- Risk: Schema değişiklikleri veya duplicate event'ler istemci tarafında sorun çıkarabilir.
- Öneri: Event payload'larına `event_id` ve `version` (opsiyonel) ekleme; reconnect sonrası minimal resync stratejisi tanımla.

5) API dökümantasyonu (Orta)
- Mevcut: Endpoint listeleri var; fakat örnek request/response ve `ApiEnvelope` şeması kısıtlı.
- Risk: Frontend-backend entegrasyonu yavaşlayabilir, yanlış anlaşılmalar olur.
- Öneri: Kritik endpoint'ler için örnek JSON blokları ekle ve mümkünse OpenAPI minimal şeması üret.

6) Dosya upload güvenliği ve limitler (Düşük-Orta)
- Mevcut: `files/prepare` ve `files/upload` mevcut; MIME/size kontroller backend'de.
- Risk: Upload izinleri, content-scan (virus), storage quota, temp->committed akışı net değil.
- Öneri: MIME whitelist, max size, scan pipeline (opsiyonel) ve storage cleanup/retention dokümanına ekle.

## Kısa Eylem Önerileri (Hızlı kazanımlar)
- Öncelik 1: 2.1 captcha üretim entegrasyonu planı ve kısa PR (handler + env değişkeni).
- Öncelik 2: Basit `Caddyfile` ve `systemd` örneği eklenmesi; README'ye deploy adımları.
- Öncelik 3: `ApiEnvelope` örnekleri ve 5–10 ana endpoint için örnek request/response blokları ekle.
- Orta vadede: WebSocket `event_id` ekleme ve reconnect sonrası resync endpoint taslağı.

## Notlar / İletişim
- Hangi maddeyle başlamak istersiniz? İsterseniz ben sırasıyla (1) captcha handler PR taslağı, (2) Caddy/systemd örnekleri ve (3) OpenAPI minimal şeması oluşturabilirim.
- Metrik/monitoring ve üretim testleri (load/abuse) eklenecekse, önceliklendirmeyi yeniden yapalım.
