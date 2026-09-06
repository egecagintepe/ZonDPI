# ZonDPI v1.0.0 — Kurulum Kılavuzu (Installation Guide)

Bu kılavuz, ZonDPI'nin Windows sistemlerine kurulumunu, yapılandırılmasını ve kaldırılmasını açıklar.

---

## 1. Hızlı Kurulum (Son Kullanıcı)

1. **İndirin:** En son `ZonDPI-1.0.0-Setup.exe` dosyasını GitHub Releases sayfasından indirin.
2. **Çalıştırın:** İndirilen kurulum dosyasını çift tıklayarak başlatın.
3. **UAC Onayı:** Windows Kullanıcı Hesabı Denetimi (UAC) penceresi açıldığında *"Evet"* butonuna tıklayarak yönetici onayını verin.
4. **ZonDPI'yi Başlatın:** Masaüstündeki veya Başlat menüsündeki ZonDPI simgesine tıklayın.
5. **Mod Seçimi:** Genel Bakış ekranında *Otomatik Mod (Auto Mode)* varsayılan olarak seçilidir.
6. **Korumayı Başlatın:** Üst kısımdaki anahtarı *"Açık"* konuma getirin. Koruma durumu *"Korumada"* olarak güncellenecektir.

---

## 2. Gelişmiş Kurulum ve Komut Satırı (Advanced / CLI)

Geliştiriciler veya sistem yöneticileri komut satırından kurulum yapabilir:

### Sessiz Kurulum:
```cmd
ZonDPI-1.0.0-Setup.exe /S
```

### Kurulum Konumları:
- **Uygulama ve Yürütülebilir Dosyalar:** `%ProgramFiles%\ZonDPI\`
- **Çalışma Zamanı ve Değişken Veriler:** `%ProgramData%\ZonDPI\`

### Windows Hizmeti Yönetimi:
ZonDPI Windows Servisi arka planda otomatik olarak yapılandırılır. Manuel kontrol:
```cmd
# Servis Durumunu Sorgula
zondpi-service.exe status

# Servisi Başlat / Durdur
zondpi-service.exe start
zondpi-service.exe stop

# Servisi Yeniden Yükle / Kaldır
zondpi-service.exe install
zondpi-service.exe uninstall
```

---

## 3. Taşınabilir (Portable) Kullanım

Kurulum yapmadan çalıştırmak için:
1. `ZonDPI-1.0.0-Windows-x64-portable.zip` arşivini bir klasöre çıkartın.
2. Sürücüsüz vekil sunucu modu için:
   - `zondpi.exe` uygulamasını çalıştırın ve `byedpi-kaspersky-mode` profilini seçin.
3. Çekirdek sürücülü GoodbyeDPI modu için:
   - `zondpi.exe` veya `zondpi-service.exe` dosyasını *"Yönetici Olarak Çalıştır"* seçeneğiyle açın.

---

## 4. Kaldırma (Uninstall)

1. Windows **Ayarlar** -> **Yüklü Uygulamalar** (veya Denetim Masası -> Program Ekle/Kaldır) bölümüne gidin.
2. **ZonDPI** uygulamasını bulun ve **Kaldır** seçeneğini tıklayın.
3. Kaldırıcı (Uninstaller) otomatik olarak:
   - ZonDPI Windows Hizmetini (`ZonDPI`) durdurur. Hizmet durdurulduğunda ZonDPI tarafından başlatılan iş parçacıkları Job Object ve süreç kimliği üzerinden güvenle kapatılır ve aygıt tanıtıcıları serbest bırakılır.
   - Harici süreçlere müdahale etmeksizin yalnızca ZonDPI mülkiyetindeki `WinDivert` sürücü kayıtları güvenle temizlenir (Error 1072 durumunda sınırlandırılmış bekleme ve durum sorgusu yapılır).
   - Ağ bağdaştırıcısı DNS ayarlarını orijinal durumuna geri döndürür.
   - ZonDPI Windows Hizmetini SCM kayıtlarından kaldırır.
   - `%ProgramFiles%\ZonDPI` altındaki tüm program dosyalarını temizler.

### Manuel Sürücü ve Hizmet Temizliği:
Yönetici olarak açılmış PowerShell veya Komut İstemi'nde:
```powershell
# ZonDPI mülkiyetindeki sürücü ve servisleri temizleme:
zondpi-service.exe clean-driver
# veya CLI ile:
zondpi-cli.exe clean-driver
```
