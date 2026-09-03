# ZonDPI v1.0.0 — Gizlilik ve Güvenlik Modeli (Privacy Policy)

ZonDPI, sansür ve DPI filtrelemelerini aşmak amacıyla yerel makinenizde çalışan açık kaynaklı bir Windows yazılımıdır.

Bu doküman, ZonDPI'nin ağ gizliliği ve veri işleme ilkelerini tanımlar.

---

## 1. Kesin Telemetri ve İzleme Karşıtı İlke (No Telemetry)

- **Telemetri Yok:** ZonDPI hiçbir kullanıcı etkileşimi, donanım kimliği, işletim sistemi detayı veya kullanım sıklığı verisi toplamaz ve uzaktaki herhangi bir sunucuya iletmez.
- **Analitik Yok:** Uygulama içinde Google Analytics, Mixpanel, Sentry veya benzeri üçüncü taraf analitik kütüphaneleri bulunmaz.
- **Merkezi Sunucu Yok:** ZonDPI merkezi bir DNS, proxy veya günlük (logging) sunucusu işletmez.
- **Geçmiş Kaydı Yok:** Kullanıcının ziyaret ettiği web siteleri, alan adı geçmişi veya DNS sorguları ZonDPI tarafından disk üzerinde kalıcı olarak saklanmaz veya kaydedilmez.
- **Paket İçeriği İnceleme Yok:** ZonDPI bir DPI aşım aracıdır; kullanıcı verilerini incelemez, paket yüklerini (payload) kaydetmez veya araya girerek (MITM) şifre çözme yapmaz.
- **DNS Çözümleme ve Üçüncü Taraflar:** Varsayılan uyumluluk profilinde (`turkey-default`), yerel ISS'lerin DNS engelleme ve zehirlemesini (örneğin Discord için uygulanan 195.175.254.2 yönlendirmesi) aşmak amacıyla DNS sorguları seçilen üçüncü taraf temiz çözümleyiciye (Cloudflare: 1.1.1.1:53 ve 2606:4700:4700::1111:53) şeffaf olarak yönlendirilir. Bu modda DNS sorguları seçilen DNS sağlayıcısı tarafından çözülür. ZonDPI'nin kendisi hiçbir DNS geçmişi toplamaz ve tutmaz.

---

## 2. Ağ Verisi Nasıl İşlenir?

1. **GoodbyeDPI Modu (WinDivert):**
   - Ağ paketleri çekirdek (kernel) seviyesinde WinDivert sürücüsü tarafından yakalanır.
   - Yalnızca HTTP/HTTPS isteklerindeki SNI ve başlık alanları standart aşım algoritmalarına (TCP segmentasyon, out-of-order paketleme, sahte paket enjeksiyonu) tabi tutulur.
   - Paketler bellekte geçici olarak manipüle edilir ve anında ağ kartına geri enjekte edilir; diske yazılmaz.

2. **ByeDPI Modu (Yerel SOCKS5 Proxy):**
   - Sadece yerel arayüze (`127.0.0.1`) bağlanır. Dış ağlardan gelen bağlantılara kapalıdır (`0.0.0.0` dinleme kesinlikle yasaktır).
   - İstemci ile hedef sunucu arasında TCP paketlerini bölerek doğrudan köprüleme yapar.
   - Trafik şifresi çözülmez; uçtan uca TLS şifrelemesi korunur.

---

## 3. Günlük Kayıtları (Diagnostics & Logs)

- ZonDPI arka plan motorlarının stdout/stderr çıktıları **yalnızca RAM üzerinde çalışan sınırlı kapasiteli dairesel bir kuyrukta (Ring Buffer - 100 satır)** tutulur.
- Bu kayıtlar servis yeniden başlatıldığında veya kapatıldığında tamamen silinir.
- Kalıcı dosya loglaması kullanıcı tarafından bilhassa açıkça ayarlanmadıkça dosya sistemine yazılmaz.

---

## 4. Kaynak Kodu Denetlenebilirliği

ZonDPI tamamen açık kaynaklıdır (Apache-2.0 lisansı altında). Uygulamanın ağ istekleri `zondpi-cli` veya üçüncü taraf ağ izleme araçları (Wireshark vb.) ile her an bağımsız olarak doğrulanabilir.
