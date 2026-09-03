# ZonDPI v1.0.0 — Dijital İmzalama ve Dağıtım Güvenliği (Code Signing Guide)

## 1. Mevcut Sürüm Durumu: UNSIGNED DEVELOPMENT RELEASE

Bu sürüm bir **geliştirici ve açık kaynak topluluk sürümüdür (Unsigned Development Release)**. Binary'ler ticari bir EV (Extended Validation) veya standart Authenticode sertifikası ile imzalanmamıştır.

### Microsoft SmartScreen Uyarısı Hakkında:
> **Önemli Not:** Windows Defender SmartScreen, dijital olarak imzalanmamış veya yeni yayımlanmış kurulum dosyalarında *"Windows kişisel bilgisayarınızı korudu"* uyarısı verebilir. Bu bir kötü amaçlı yazılım (malware) tespiti değil; dijital sertifika ve Microsoft sunucularında yeterli indirme itibarının (reputation) henüz oluşmamış olmasından kaynaklanan standart bir Windows koruma uyarısıdır.
>
> Kuruluma devam etmek için: **"Ek Bilgi"** -> **"Yine de çalıştır"** butonuna tıklanabilir.

---

## 2. Resmi Üretim Dağıtımı İçin İmzalama Yönergeleri

Resmi bir ticari sertifika temin edildiğinde uygulanması gereken imzalama sırası:

### A. Gerekli Araçlar
- Windows SDK `signtool.exe`
- Geçerli Authenticode / EV Kod İmzalama Sertifikası (USB Token veya Azure Trusted Signing)
- Standart RFC 3161 Zaman Damgası Sunucusu (Timestamping)

### B. İmzalama Sırası (Execution Order)

1. **Üçüncü Taraf Sürücü ve Kütüphaneler:**
   - `third_party\windivert\WinDivert64.sys` ve `WinDivert.dll` dosyalarının orijinal upstream imzası **asla bozulmamalıdır**. Bu dosyalar Microsoft tarafından onaylanmış dijital imzaya sahiptir.

2. **C Motorları:**
   ```cmd
   signtool sign /tr http://timestamp.digicert.com /td sha256 /fd sha256 /a dist\engines\goodbye\goodbyedpi.exe
   signtool sign /tr http://timestamp.digicert.com /td sha256 /fd sha256 /a dist\engines\byedpi\ciadpi.exe
   ```

3. **Rust Servisi ve CLI:**
   ```cmd
   signtool sign /tr http://timestamp.digicert.com /td sha256 /fd sha256 /a dist\zondpi-service.exe
   signtool sign /tr http://timestamp.digicert.com /td sha256 /fd sha256 /a dist\zondpi-cli.exe
   signtool sign /tr http://timestamp.digicert.com /td sha256 /fd sha256 /a dist\zondpi.exe
   signtool sign /tr http://timestamp.digicert.com /td sha256 /fd sha256 /a dist\zondpi-gui.exe
   ```

4. **Son Kurulum Paketi (Installer):**
   ```cmd
   signtool sign /tr http://timestamp.digicert.com /td sha256 /fd sha256 /a release\ZonDPI-1.0.0-Setup.exe
   ```

### C. Doğrulama (Verification)
İmzalanmış dosyaların doğrulanması:
```cmd
signtool verify /pa /v release\ZonDPI-1.0.0-Setup.exe
```
