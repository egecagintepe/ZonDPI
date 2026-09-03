======================================================================
  ZonDPI v1.0.0 (Windows x64) — Taşınabilir Sürüm (Portable Edition)
======================================================================

Bu paket, kurulum gerektirmeyen (portable) ZonDPI yürütülebilir dosyalarını,
aşım motorlarını ve profillerini içerir.

----------------------------------------------------------------------
1. YÖNETİCİ HAKLARI (UAC) VE ÇALIŞMA MODLARI
----------------------------------------------------------------------
* ÖNEMLİ: "Portable" olmak, yönetici izni gerektirmediği anlamına gelmez!

  a) ByeDPI Modu (Sürücüsüz / SOCKS5):
     - Yönetici (Administrator) izni GEREKTİRMEZ.
     - Standart kullanıcı haklarıyla çalıştırılabilir.
     - 127.0.0.1:1080 portunda yerel SOCKS5 vekillik sunucusu açar.

  b) GoodbyeDPI Modu (WinDivert Sürücüsü):
     - WinDivert çekirdek sürücüsünün (WinDivert64.sys) yüklenmesi için
       YÖNETİCİ (Administrator) yetkisi ZORUNLUDUR.
     - zondpi.exe veya zondpi-service.exe "Yönetici Olarak Çalıştır" seçilmelidir.

  c) Windows Servisi Modu:
     - Windows Hizmetler (SCM) listesine kayıt ve başlatma için
       YÖNETİCİ izni ZORUNLUDUR.

----------------------------------------------------------------------
2. HIZLI BAŞLANGIÇ
----------------------------------------------------------------------
* Grafiksel Arayüz (GUI):
  zondpi.exe (veya zondpi-gui.exe) dosyasına çift tıklayın.

* Konsol Geliştirici / Ön Plan Modu:
  .\zondpi-service.exe --foreground

* Komut Satırı Yönetimi (CLI):
  .\zondpi-cli.exe status
  .\zondpi-cli.exe start-auto turkey-default
  .\zondpi-cli.exe stop
  .\zondpi-cli.exe test-connectivity discord.com

----------------------------------------------------------------------
3. DİZİN YAPISI
----------------------------------------------------------------------
ZonDPI-Portable\
  ├── zondpi.exe             (Grafiksel Kullanıcı Arayüzü)
  ├── zondpi-service.exe     (Windows Arka Plan Servis Motoru)
  ├── zondpi-cli.exe         (Yönetim ve Tanılama Komut Satırı)
  ├── engines\
  │   ├── goodbye\           (GoodbyeDPI ve WinDivert kütüphaneleri)
  │   └── byedpi\            (ciadpi SOCKS5 motoru)
  ├── profiles\              (Türkiye ve genel DPI aşım profilleri)
  ├── third_party\           (Resmi imzalı WinDivert sürücüleri)
  └── docs\                  (Teknik ve kullanıcı dokümanları)

Açık kaynak: https://github.com/zondpi/zondpi
Lisans: Apache-2.0
