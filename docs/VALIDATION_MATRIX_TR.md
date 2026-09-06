# ZonDPI v1.0.0 — Türkiye Doğrulama ve Uyumluluk Matrisi

Bu doküman, ZonDPI'nin Türkiye ağlarındaki hedef servisler, İnternet Servis Sağlayıcıları (İSS) ve güvenlik yazılımları üzerindeki gerçek test ve doğrulama durumunu kayıt altına alır.

---

## 1. Hedef Servis ve Protokol Matrisi

Aşağıdaki ölçümler yerel test ortamında doğrudan TCP/TLS ve DNS seviyesinde doğrulanmıştır. Erişim engeli bulunmayan veya test anında engellenmemiş olan hedefler dürüstçe **NOT CURRENTLY BLOCKED** olarak işaretlenmiştir.

| Hedef Kategori | Test Hedefi | Port/Protokol | Temel Durum (ZonDPI Kapalı) | ZonDPI ile Durum | Doğrulama Notu |
| :--- | :--- | :--- | :--- | :--- | :--- |
| **Discord** | `discord.com` | TCP/443 (HTTPS) | REACHABLE | REACHABLE | Test ortamında doğrudan erişilebilir durumdadır. |
| **X / Twitter** | `x.com` | TCP/443 (HTTPS) | REACHABLE | REACHABLE | Test anında bant daraltma veya erişim engeli saptanmamıştır. |
| **YouTube** | `www.youtube.com` | TCP/443 (HTTPS) | REACHABLE | REACHABLE | Doğrudan erişilebilir. SNI manipülasyonu bağlantıyı bozmaz. |
| **Cloudflare CDN** | `www.cloudflare.com` | TCP/443 (HTTPS) | REACHABLE | REACHABLE | ECH / TLS 1.3 bağlantıları stabil. |
| **Generic HTTPS** | `example.com` | TCP/443 (HTTPS) | REACHABLE | REACHABLE | Temel TCP handshake ve TLS doğrulandı. |
| **HTTP/3 (QUIC)** | UDP 443 | UDP/443 | ALLOWED / BLOCKED BY POLICY | HANDLED (`-q` flag) | GoodbyeDPI `-q` bayrağı ile QUIC bloklanarak güvenli HTTP/2-TCP fallback sağlanır. |

---

## 2. İnternet Servis Sağlayıcısı (İSS) Matrisi

Aşağıdaki tabloda "Profil Tanımı Mevcut" ile "Gerçek İSS Üzerinde Doğrulandı" kavramları kesin olarak ayrılmıştır:

| Servis Sağlayıcı | Profil Adı | Profil Yapılandırması | Yerel Şema & Derleme | Canlı İSS Test Durumu |
| :--- | :--- | :--- | :--- | :--- |
| **Türk Telekom** | `turkey-default` | Modeset -5, TTL 5, DNS 1253 Yandex | **PASS** | **NOT TESTED** (Fiziksel TT fiber/DSL test hattı mevcut değil) |
| **Turkcell Superonline** | `superonline-default` | TTL 3, Modeset -5, DNS 1253 Yandex | **PASS** | **NOT TESTED** (Fiziksel Superonline test hattı mevcut değil) |
| **Diğer / Yerel Ağ** | `byedpi-kaspersky-mode` | SOCKS5 Proxy 127.0.0.1:1080 | **PASS** | **PASS** (Yerel ağ bağlantısı üzerinde SOCKS5 handshake ve trafik doğrulandı) |

---

## 3. Güvenlik Yazılımları (AV / EDR) Matrisi

Sistemdeki `root/SecurityCenter2` WMI sorgusu ile yalnızca yüklü ve aktif ürünler test edilmiştir.

| Güvenlik Yazılımı | Kurulu Durum | Tespit / Karantina | Sürücü / Motor Uyumluluğu | Doğrulama Sonucu |
| :--- | :--- | :--- | :--- | :--- |
| **Windows Defender** | **KURULU & AKTİF** | YOK (0 Tespit) | WinDivert ve ciadpi sorunsuz çalıştı | **PASS** |
| **Kaspersky** | Fiziksel Test Ortamı | YOK (0 Tespit) | GoodbyeDPI -5 + Temiz Bağdaştırıcı DNS | **Uyumluluk düzeltmesi uygulandı, doğrulama bekleniyor** |
| **ESET NOD32** | YÜKLÜ DEĞİL | - | - | **NOT TESTED** (Ortamda yüklü değil) |
| **Bitdefender** | YÜKLÜ DEĞİL | - | - | **NOT TESTED** (Ortamda yüklü değil) |

### Windows Defender Doğrulama Raporu:
- Defender Antivirus aktif durumdadır.
- `goodbyedpi.exe`, `ciadpi.exe`, `WinDivert.dll`, `WinDivert64.sys`, `zondpi-service.exe`, `zondpi-gui.exe` dosyaları taranmış ve hiçbir tehdit uyarısı veya karantina işlemi gerçekleşmemiştir.
- Defender ayarları değiştirilmemiş ve exclusion eklenmemiştir.

---

## 4. Tanılama ve Doğrulama Komutu

Ağ durumunu bağımsız olarak doğrulamak için `zondpi-cli` aracı kullanılabilir:

```powershell
.\dist\zondpi-cli.exe test-connectivity discord.com --port 443
.\dist\zondpi-cli.exe test-connectivity www.cloudflare.com --port 443
```
