# Novastrum TODO / Eksikler

## Kritik
- [ ] Raw UI: Mesaj geçmişi için server-side pagination/scroll API’si ekle veya (geçici) limit/temizleme koy.
- [ ] Raw UI: DM listesinde isim tespiti, üyelik/veritabanı verisine dayalı hale getir (şu an gönderilen mesajlardan çıkarıyor).
- [ ] DM istekleri: accept/reject sonrası WS notification/admin event push ekle; frontend’ler poll’a bağımlı olmasın.
- [ ] app.html & App-Fixed: Yeni DM kabul/ret akışını ve mesaj DTO (edited_at/deleted) alanlarını işleyen UI’ler ekle veya tamamen raw yönlendirmesine karar ver.

## Önerilen geliştirmeler
- [ ] Presence/unread counters’ı raw UI’ya ekle; `/api/notifications/list` ve mute kontrolü için basit panel.
- [ ] File upload (prepare + upload) akışını raw UI’ya ekle.
- [ ] Tailwind CDN uyarısı (App-Fixed) çözümü: build pipeline veya yerel CSS.
- [ ] Mesaj feed’inde eski mesajları sunucudan çekecek endpoint tasarla (chat history API).
- [ ] Discussion detayında yanıtları ve oy skorlarını göster; mevcut list endpoint’ini kullanarak thread görünümleri inşa et.

## Temizlik / DX
- [ ] Bootstrap payload’a (opsiyonel) friendships ve pending_dm_requests ekleyip round-trip sayısını düşür.
- [ ] app.html / App-Fixed yedeklerini gözden geçir; ihtiyaç yoksa kaldır ya da geliştirme branch’ine al.

