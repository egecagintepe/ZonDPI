# ZonDPI 1.0.1 — Sürüm Notları (Release Notes)

Türkiye'deki DPI (Derin Paket İnceleme) kaynaklı bağlantı engellemelerini ve bant daraltmalarını aşmak için tasarlanmış, modern ve açık kaynaklı Windows sistemi.

---

## 1. v1.0.1 İle Gelen Düzeltmeler ve Yenilikler (What's New in v1.0.1)

- **Kurulumcu Dizin Ağacı Düzeltmesi (Installer Path Hotfix):** 
  - Önceki sürümde `_up_\_up_\_up_\dist` şeklinde oluşan iç içe dizin yerleşimi tamamen giderildi.
  - Artık tüm bileşenler doğrudan `C:\Program Files\ZonDPI\` kök dizinine temiz biçimde kurulur.
  - Derleme aşamasına otomatik `_up_` regresyon tespit kapısı eklendi.
- **Otomatik Windows Servis Kurulumu ve Başlatılması:**
  - NSIS kurulumcusu arka plan servisini (`zondpi-service.exe`) kurulum anında otomatik olarak kaydeder ve başlatır.
  - Kaldırma (uninstall) işlemi sırasında servis otomatik durdurulur ve sistemden temizlenir.
- **Tamamen Yenilenen Fluent/Sidebar Kullanıcı Deneyimi:**
  - Modern Windows 11/Fluent tasarım diline uygun sol kenar çubuğu (Sidebar) navigasyonu.
  - Ana ekrandan teknik motor adları kaldırılarak sadeleştirilmiş durum göstergesi ("ZonDPI Aktif / Kapalı") ve durum rozetleri eklendi.
  - Tanılama sekmesine "Gelişmiş teknik ayrıntıları göster" butonu ve sistem durumu grid'i eklendi.
  - Ayarlar sekmesine "Windows ile birlikte başlat" ve "Sistem tepsisinde küçült" tercihleri eklendi.
  - Hakkında sekmesinde açık kaynaklı bağımlılıkların (GoodbyeDPI, ByeDPI, WinDivert, Tauri, Tokio, React) lisans ve atıf bilgileri korundu.
- **Sistem Tepsisi (System Tray) Entegrasyonu:**
  - Pencere kapatıldığında (`X` butonu) uygulama sonlanmaz, arka planda sistem tepsisinde çalışmaya devam eder.
  - Tepsi ikonu üzerinden tek tıkla pencere açma/gizleme ve sağ tık menüsü ile doğrudan çıkış desteği sağlandı.

---

## 2. Temel Mimari Özellikler (Core Features)

- **Birleşik Windows Mimarisi:** Arka planda güvenli Windows Servisi (`zondpi-service.exe`), Tauri 2 tabanlı modern masaüstü arayüzü (`zondpi.exe`) ve komut satırı aracı (`zondpi-cli.exe`).
- **Çift Motorlu Hibrit Aşım Desteği:**
  - **GoodbyeDPI (Kernel/WinDivert):** Sistem genelinde, vekil sunucu ayarı gerektirmeyen şeffaf paket manipülasyonu.
  - **ByeDPI / ciadpi (Kullanıcı Alanı / SOCKS5):** Yönetici hakları veya çekirdek sürücüsü gerektirmeyen, antivirüs yazılımlarıyla %100 uyumlu yerel SOCKS5 proxy modu.
- **Akıllı Otomatik Mod (Auto Mode):** Sistemde kurulu güvenlik yazılımlarını (Windows Defender, Kaspersky vb.) WMI üzerinden otomatik algılar; en uyumlu motoru seçer.
- **Türkiye'ye Özel Ön Ayarlar:** Türk Telekom ve Superonline altyapıları için test edilmiş `turkey-default` ve `superonline-default` profilleri.
- **Sertleştirilmiş Güvenlik:** Sadece yerel kullanıcılara ve yöneticilere açık, ağa kapalı Named Pipe IPC erişim kontrol listesi (SDDL DACL).
- **Temiz Süreç Yönetimi:** Windows Job Object API ile yetim kalan veya asılı kalan (orphan) süreçlerin %100 engellenmesi.
- **Gizlilik Odaklı:** Sıfır telemetri, sıfır analitik, disk üzerinde kullanıcı geçmişi kaydetmeyen mimari.

---

## 3. Desteklenen Çalışma Modları (Supported Modes)

1. **Auto Mode (Önerilen):** Uyumluluk ve sürücü durumuna göre GoodbyeDPI veya ByeDPI motorunu otomatik yönetir.
2. **GoodbyeDPI Modu:** Tüm sistem trafiğini şeffaf olarak filtreler (Yönetici yetkisi gerektirir).
3. **ByeDPI (Kaspersky Uyumlu Mod):** `127.0.0.1:1080` portunda yerel SOCKS5 proxy çalıştırır (Yönetici yetkisi gerektirmez).

---

## 4. Test Edilen ve Doğrulanan Ortam (Tested Environment)

- **İşletim Sistemi:** Microsoft Windows 11 / Windows 10 (x86_64)
- **Güvenlik Yazılımı:** Windows Defender (Aktif, 0 tespit / karantina)
- **Hedef Doğrulamaları:** Discord, X (Twitter), YouTube, Cloudflare CDN, Generic HTTPS
- **Ağ Motorları:** GoodbyeDPI v0.2.3rc3, ByeDPI v0.15, WinDivert v1.4.3
- **Test Sonucu:** Tüm yerel birim, entegrasyon, Named Pipe IPC, SOCKS5 el sıkışması ve süreç yaşam döngüsü testleri başarıyla tamamlanmıştır.

---

## 5. Bilinen Sınırlamalar ve Doğrulama Durumu (Known Limitations & Status)

- **İmzasız Geliştirici Sürümü (Unsigned Build):** Bu sürüm resmi EV Authenticode sertifikası ile imzalanmamıştır. Windows SmartScreen açılışta uyarı gösterebilir.
- **Fiziksel İSS Doğrulaması:** Türk Telekom ve Turkcell Superonline profilleri derleme ve şema testlerinden geçmiş olup, canlı fiziksel hat doğrulama durumu `NOT TESTED` olarak işaretlenmiştir.
- **Kaspersky / ESET / Bitdefender:** Test makinesinde kurulu olmadığından bu antivirüsler altındaki canlı test durumu dürüstçe `NOT TESTED` olarak kayıt altına alınmıştır.

---

## 6. Güvenlik Notları (Security Notes)

- WinDivert sürücüleri orijinal dijital imzalarını korumaktadır.
- Named Pipe uç noktası (`\\.\pipe\zondpi-service-ipc`) sadece `SYSTEM`, `Builtin Administrators` ve `Authenticated Users` yetkilerine açıktır.
- Ağ üzerinden anonim veya uzaktan IPC erişimi engellenmiştir.
