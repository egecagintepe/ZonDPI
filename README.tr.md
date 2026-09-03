# ZonDPI (Türkçe)

[![License](https://img.shields.io/badge/Lisans-Apache_2.0-blue.svg)](LICENSE)
[![Platform](https://img.shields.io/badge/Platform-Windows_10_%2F_11_x64-0078D6.svg)](docs/COMPATIBILITY.md)
[![Sürüm](https://img.shields.io/badge/S%C3%BCr%C3%BCm-v1.0.4-success.svg)](https://github.com/egecagintepe/ZonDPI/releases/latest)
[![CI](https://github.com/egecagintepe/ZonDPI/actions/workflows/ci.yml/badge.svg)](https://github.com/egecagintepe/ZonDPI/actions/workflows/ci.yml)
[![Sıfır Telemetri](https://img.shields.io/badge/Telemetri-S%C4%B1f%C4%B1r-brightgreen.svg)](docs/PRIVACY.md)

[English](README.md) | **Türkçe**

Türkiye'deki İnternet Servis Sağlayıcı (İSS) ağlarında uygulanan Derin Paket İnceleme (DPI) filtrelemelerini, TLS ClientHello SNI engellemelerini ve DNS zehirlenmelerini aşmak için geliştirilmiş açık kaynaklı Windows sistem aracı.

---

> [!NOTE]
> **ZonDPI Ne DEĞİLDİR**: ZonDPI bir **VPN**, ücretli proxy servisi veya anonimlik ağı **değildir**. İnternet trafiğinizi üçüncü taraf sunuculara yönlendirmez ve genel IP adresinizi gizlemez. Bunun yerine, tamamen kendi Windows bilgisayarınızda yerel olarak çalışarak paketleri parçalar ve TCP/TLS el sıkışmalarını yeniden düzenler; böylece aradaki İSS inceleme cihazlarının Discord gibi yasal servislere giden bağlantıları kesmesini engeller.

---

## Temel Özellikler

* **Türkiye İSS Ağlarında Fiziksel Olarak Doğrulandı**: Varsayılan profiller Türk Telekom ve Turkcell Superonline fiber/VDSL hatlarında bizzat test edilip doğrulanmış; Discord web ve masaüstü uygulamasının ping kaybı olmaksızın çalışmasını sağlamıştır.
* **Çift Çalışma Modu**:
  * **Sistem Paket Filtresi (Varsayılan)**: İmzalı WinDivert sürücüsü üzerinden çekirdek düzeyinde şeffaf TCP segmentasyonu ve auto-TTL atlama tekniği uygular. Ek proxy ayarı gerektirmeden tüm tarayıcılar, oyunlar ve masaüstü uygulamalarında sistem genelinde çalışır.
  * **Antivirüs Uyumluluk Modu**: Kaspersky, Bitdefender veya ESET gibi derin paket taraması yapan antivirüs yazılımlarıyla çakışmaları önlemek için tasarlanmış yerel loopback proxy modu.
* **Windows Arka Plan Hizmeti Mimarisi**: Kullanıcı arayüzü, Windows Hizmet Denetim Yöneticisi (SCM) altında çalışan ayrıcalıklı bir Windows Hizmetine (`zondpi-service.exe`) bağlanır. Çökme durumunda otomatik yeniden başlatma ve temiz kapanma güvencesi sunar.
* **DNS Zehirlenmesi Koruması & Otomatik Geri Alma**: Ağ bağdaştırıcılarını standart portlar üzerinden Cloudflare 1.1.1.1 DNS ile korur. Hizmet kapandığında veya sistem yeniden başladığında bağdaştırıcı DNS ayarlarını otomatik olarak önceki orijinal durumuna geri döndürür.
* **Sıfır Telemetri & Mutlak Gizlilik**: Hiçbir kullanıcı verisi, tarama geçmişi, çökme kaydı toplanmaz; ZonDPI sunucularına herhangi bir veri iletimi yapılmaz.
* **Modern Masaüstü Arayüzü & Operatör Komut Satırı (CLI)**: Sistem tepsisi (tray) kontrollü modern masaüstü uygulaması (Tauri v2) ve otomasyon/tanılama için script edilebilir komut satırı aracı (`zondpi-cli.exe`).

---

## Mimari Bakış

```
┌────────────────────────────────────────────────────────┐
│               ZonDPI Masaüstü Arayüzü (Tauri v2)       │
│           veya Script Edilebilir CLI (zondpi-cli)      │
└───────────────────────────┬────────────────────────────┘
                            │ Yerel Named Pipe IPC
                            ▼
┌────────────────────────────────────────────────────────┐
│            ZonDPI Windows Arka Plan Hizmeti            │
│         (SCM Yönetimli • Otomatik İyileşme)            │
├───────────────────────────┬────────────────────────────┤
│   Paket Filtreleme Motoru │   Uyumluluk Motoru         │
│   (WinDivert Sürücüsü •   │   (WMI Antivirüs Tespiti • │
│    TCP/TLS Segmentasyonu) │    Yerel SOCKS5 Loopback)  │
└───────────────────────────┴────────────────────────────┘
```

---

## Hızlı Başlangıç

### 1. İndirme
Resmi [Sürümler (Releases)](https://github.com/egecagintepe/ZonDPI/releases/latest) sayfasından en son doğrulanmış sürümü indirin:
* **Kurulum Dosyası**: `ZonDPI-1.0.4-Setup.exe` (Önerilen)
* **Taşınabilir Sürüm**: `ZonDPI-1.0.4-Windows-x64-portable.zip`

### 2. Kurulum ve SmartScreen Uyarısı
1. `ZonDPI-1.0.4-Setup.exe` dosyasını çalıştırın ve Windows UAC (Kullanıcı Hesabı Denetimi) onayını verin.
2. *Windows SmartScreen Uyarısı*: ZonDPI açık kaynaklı ve ücretsiz bir yazılım olduğu için pahalı ticari kod imzalama sertifikalarına sahip değildir. Windows SmartScreen bilinmeyen yayımcı uyarısı verirse: **"Ek bilgi"** -> **"Yine de çalıştır"** butonuna tıklayın. İndirdiğiniz dosyanın bütünlüğünü [SHA256SUMS.txt](https://github.com/egecagintepe/ZonDPI/releases/latest) dosyasındaki resmi hash ile doğrulayabilirsiniz.

### 3. Kullanım
1. Başlat Menüsünden veya sistem tepsisinden **ZonDPI** uygulamasını açın.
2. **Korumayı Başlat** düğmesine tıklayın.
3. Bağlantınız hemen korunmaya başlar; tarayıcıyı yeniden başlatmanız veya ağ ayarlarını elle değiştirmeniz gerekmez.

---

## Komut Satırı Aracı (CLI)

ZonDPI, ileri düzey kullanıcılar ve otomasyon için `zondpi-cli.exe` aracını içerir:

```powershell
# Hizmet ve koruma durumunu denetle
.\zondpi-cli.exe status

# Kapsamlı uçtan uca ağ tanılamasını çalıştır
.\zondpi-cli.exe diag

# Korumayı başlat / durdur / yeniden başlat
.\zondpi-cli.exe start
.\zondpi-cli.exe stop
.\zondpi-cli.exe restart
```

---

## Doğrulama ve Kriptografik Özetler (SHA-256)

Yayınlanan her sürüm `SHA256SUMS.txt` dosyasında doğrulanabilir özetler sunar:

```powershell
# İndirilen kurulum dosyasının bütünlüğünü doğrulayın
Get-FileHash .\ZonDPI-1.0.4-Setup.exe -Algorithm SHA256
```

v1.0.4 sürümü için beklenen özetler:
* `ZonDPI-1.0.4-Setup.exe`: `A3A537298792B0507EE5C854BC301C35B77533F5932511AB70C633BD2C186FD2`
* `ZonDPI-1.0.4-Windows-x64-portable.zip`: `55A90FCFB485D93B4938679A4AF03E3658328EAB2BB495C72B98BBF51C685036`

---

## Belgeler

* [Donanım & İSS Uyumluluk Matrisi](docs/COMPATIBILITY.md)
* [Sorun Giderme ve Tanılama Kılavuzu](docs/TROUBLESHOOTING.md)
* [Mimari Dokümantasyonu](docs/ARCHITECTURE.md)
* [Kaynak Koddan Derleme Kılavuzu](docs/DEVELOPMENT.md)
* [Bağımlılık ve Tedarik Zinciri Raporu](DEPENDENCIES.md)
* [Gizlilik Bildirimi](docs/PRIVACY.md)
* [Güvenlik Modeli](docs/SECURITY_MODEL.md)

---

## Üçüncü Taraf Açık Kaynak Atıfları

ZonDPI, açık kaynak topluluğunun temel çalışmalarından yararlanmaktadır:

* **[GoodbyeDPI](https://github.com/ValdikSS/GoodbyeDPI)** - ValdikSS (Apache Lisansı 2.0)
* **[ByeDPI](https://github.com/hufrea/byedpi)** - hufrea (MIT Lisansı)
* **[WinDivert](https://github.com/basil00/WinDivert)** - basil00 (LGPL-3.0 / GPL-2.0)
* **[uthash](https://github.com/troydhanson/uthash)** - Troy D. Hanson (BSD 1-Madde)

Tüm açık kaynak lisans metinleri ve tam commit bilgileri için [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md) dosyasına bakınız.

---

## Lisans

ZonDPI, [Apache Lisansı 2.0](LICENSE) altında lisanslanmıştır.
