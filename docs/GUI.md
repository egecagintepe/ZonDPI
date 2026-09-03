# ZonDPI v1.0.0 — Masaüstü Kullanıcı Arayüzü (GUI Guide)

ZonDPI masaüstü kullanıcı arayüzü, Tauri v2, React 19 ve modern Vanilla CSS kullanılarak geliştirilmiş, yerel Windows performansına sahip bir Fluent karanlık tema uygulamasıdır.

---

## 1. Mimari ve İletişim

- **Hafif Bellek Tüketimi:** Arayüz, Windows Evergreen WebView2 bileşenini kullanır ve arka plan motoruyla doğrudan Windows Named Pipe (`\\.\pipe\zondpi-service-ipc`) üzerinden haberleşir.
- **Güvenli IPC:** Tüm işlemler uzunluk önekli (length-prefixed) asenkron JSON protokolü üzerinden yürütülür.
- **Kullanıcı İzinleri:** Standart kullanıcı haklarıyla açılabilir. Arka plandaki ayrıcalıklı servis işlemlerini IPC komutları ile yönlendirir.

---

## 2. Arayüz Bölümleri

1. **Genel Bakış (Overview):**
   - Canlı koruma durumu (Korumada / Korumasız / Servis Bağlantısı Kesildi).
   - Tek dokunuşla koruma başlatma / durdurma anahtarı.
   - Aktif motor, profil ve akıllı uyumluluk analizi kartları.
2. **Aşım Profilleri (Profiles):**
   - Türkiye Varsayılan (`turkey-default`) ve Kaspersky Uyumlu (`byedpi-kaspersky-mode`) profilleri arasında anında geçiş.
3. **Günlükler (Logs):**
   - Servis ve motor süreçlerinin gerçek zamanlı çıktılarını görüntüleme.
4. **Ayarlar (Settings):**
   - Otomatik mod (Auto Mode) açma/kapama.
   - Windows başlangıç servis entegrasyonu.
   - Gizlilik ve telemetri durum doğrulaması.
