import { useState, useEffect, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import "./index.css";

// ── Profile display name mapping ──
const PROFILE_DISPLAY: Record<string, { name: string; desc: string }> = {
  "turkey-default": {
    name: "Türkiye — Varsayılan",
    desc: "Genel Türkiye ISS altyapıları için optimize edilmiş ayarlar.",
  },
  "superonline-default": {
    name: "Turkcell Superonline",
    desc: "Superonline DPI ekipmanlarına özel yapılandırma.",
  },
  "byedpi-kaspersky-mode": {
    name: "Uyumluluk Modu",
    desc: "Üçüncü taraf güvenlik yazılımlarıyla tam uyumlu çalışma modu.",
  },
};

function profileDisplayName(id: string): string {
  return PROFILE_DISPLAY[id]?.name ?? id;
}

interface LogEntry {
  timestamp: string;
  line: string;
}

function normalizeLogs(rawLogs: LogEntry[]): LogEntry[] {
  const result: LogEntry[] = [];
  const seenMilestones = new Set<string>();

  for (const log of rawLogs) {
    const lower = log.line.toLowerCase();
    let normalizedText: string | null = null;

    if (lower.includes("valdikss") || lower.includes("github.com") || lower.includes("goodbyedpi v")) {
      normalizedText = "ZonDPI ağ motoru başlatılıyor...";
    } else if (lower.includes("opening filter")) {
      normalizedText = "Paket filtresi hazırlanıyor...";
    } else if (lower.includes("filter activated") || lower.includes("goodbyedpi is now running")) {
      normalizedText = "ZonDPI paket filtresi etkinleştirildi.";
    } else if (lower.includes("dns redirect: 1") || lower.includes("dns redirect: on")) {
      normalizedText = "DNS uyumluluk katmanı: Etkin";
    } else if (lower.includes("dnsv6 redirect: 1") || lower.includes("dnsv6 redirect: on")) {
      normalizedText = "IPv6 DNS uyumluluk katmanı: Etkin";
    } else if (lower.includes("native fragmentation")) {
      normalizedText = "Paket işleme: Etkin";
    } else if (lower.includes("auto ttl")) {
      normalizedText = "Gecikme ve TTL optimizasyonu: Etkin";
    } else if (lower.includes("fragments sending in reverse")) {
      normalizedText = "Gelişmiş paket sırası: Etkin";
    } else if (lower.includes("ciadpi") || lower.includes("byedpi")) {
      normalizedText = "ZonDPI uyumluluk motoru hazırlandı.";
    } else if (lower.includes("socks5")) {
      normalizedText = "Yerel uyumluluk proxy katmanı hazırlandı.";
    } else if (
      lower.includes("block passive") ||
      lower.includes("block quic") ||
      lower.includes("fragment http") ||
      lower.includes("host no space") ||
      lower.includes("additional space") ||
      lower.includes("mix host") ||
      lower.includes("http allports") ||
      lower.includes("allow missing sni") ||
      lower.includes("fake requests")
    ) {
      normalizedText = null;
    } else if (lower.includes("error") || lower.includes("failed")) {
      normalizedText = log.line
        .replace(/GoodbyeDPI/gi, "ZonDPI Motoru")
        .replace(/ByeDPI/gi, "ZonDPI Uyumluluk")
        .replace(/ciadpi/gi, "ZonDPI İş parçacığı")
        .replace(/WinDivert/gi, "Paket Filtre Sürücüsü");
    }

    if (normalizedText && !seenMilestones.has(normalizedText)) {
      seenMilestones.add(normalizedText);
      result.push({
        timestamp: log.timestamp || new Date().toLocaleTimeString(),
        line: normalizedText,
      });
    }
  }

  if (result.length === 0) {
    result.push({
      timestamp: new Date().toLocaleTimeString(),
      line: "ZonDPI koruması aktif ve çalışıyor.",
    });
  }

  return result;
}

type Page = "overview" | "profiles" | "diagnostics" | "settings" | "about";

function App() {
  const [page, setPage] = useState<Page>("overview");
  const [status, setStatus] = useState<string>("connecting");
  const [isRunning, setIsRunning] = useState(false);
  const [activeEngine, setActiveEngine] = useState("");
  const [activeProfile, setActiveProfile] = useState("turkey-default");
  const [selectedProfile, setSelectedProfile] = useState("turkey-default");
  const [autoMode, setAutoMode] = useState(true);
  const [logs, setLogs] = useState<LogEntry[]>([]);
  const [recommendation, setRecommendation] = useState<any>(null);
  const [showAdvanced, setShowAdvanced] = useState(false);
  const [developerMode, setDeveloperMode] = useState<boolean>(() => {
    return localStorage.getItem("zondpi_developer_mode") === "true";
  });
  const [showRawLogs, setShowRawLogs] = useState(false);
  const [autoProtect, setAutoProtect] = useState<boolean>(() => {
    return localStorage.getItem("zondpi_auto_protect") === "true";
  });
  const autoProtectTriggered = useRef(false);
  const [startOnBoot, setStartOnBoot] = useState<boolean>(() => {
    return localStorage.getItem("zondpi_autostart") === "true";
  });
  const [startMinimized, setStartMinimized] = useState<boolean>(() => {
    return localStorage.getItem("zondpi_minimized") === "true";
  });

  // ── Status polling ──
  useEffect(() => {
    const check = async () => {
      try {
        const res: any = await invoke("ipc_status");
        if (res?.success && res.result) {
          const r = res.result;
          const running = Boolean(r.active_engine);
          setIsRunning(running);
          setActiveEngine(r.active_engine || "");
          if (r.active_profile) setActiveProfile(r.active_profile);
          setStatus(running ? "active" : "idle");

          // Auto-protection preference on startup
          if (autoProtect && !autoProtectTriggered.current && !running && r.service_state === "Running") {
            autoProtectTriggered.current = true;
            invoke("ipc_start_auto", { profile: selectedProfile || "turkey-default" }).catch(console.error);
          }
        } else {
          setStatus("error");
          setIsRunning(false);
        }
      } catch {
        setStatus("disconnected");
        setIsRunning(false);
      }
    };
    check();
    const id = setInterval(check, 2500);
    return () => clearInterval(id);
  }, [autoProtect, selectedProfile]);

  // ── Fetch recommendation ──
  useEffect(() => {
    (async () => {
      try {
        const r: any = await invoke("ipc_recommendation");
        if (r?.success && r.result) setRecommendation(r.result);
      } catch { /* silent */ }
    })();
  }, []);

  // ── Fetch logs when on diagnostics page ──
  useEffect(() => {
    if (page !== "diagnostics") return;
    const fetch = async () => {
      try {
        const r: any = await invoke("ipc_logs", { lines: 80 });
        if (r?.success && r.result) setLogs(r.result);
      } catch { /* silent */ }
    };
    fetch();
    const id = setInterval(fetch, 3000);
    return () => clearInterval(id);
  }, [page]);

  const toggleProtection = async () => {
    if (status === "connecting" || status === "starting" || status === "stopping") return;
    try {
      if (isRunning) {
        setStatus("stopping");
        await invoke("ipc_stop");
      } else {
        setStatus("starting");
        if (autoMode) {
          await invoke("ipc_start_auto", { profile: selectedProfile });
        } else {
          const engine = selectedProfile.includes("byedpi") ? "byedpi" : "goodbye";
          await invoke("ipc_start", { engine, profile: selectedProfile });
        }
      }
    } catch (e: any) {
      console.error(e);
      setStatus("error");
    }
  };

  // ── Status helpers ──
  const statusLabel = (): string => {
    switch (status) {
      case "connecting": return "Bağlanıyor…";
      case "active": return "ZonDPI Aktif";
      case "idle": return "ZonDPI Kapalı";
      case "starting": return "Başlatılıyor…";
      case "stopping": return "Durduruluyor…";
      case "disconnected": return "Servis Bağlantısı Yok";
      case "error": return "Servis Hatası";
      default: return "Bilinmiyor";
    }
  };

  const statusRingClass = (): string => {
    if (status === "active") return "active";
    if (status === "starting" || status === "stopping" || status === "connecting") return "transitioning";
    if (status === "disconnected" || status === "error") return "error";
    return "inactive";
  };

  const statusEmoji = (): string => {
    if (status === "active") return "🛡️";
    if (status === "starting" || status === "stopping" || status === "connecting") return "⏳";
    if (status === "disconnected" || status === "error") return "⚠️";
    return "○";
  };

  const isTransitioning = status === "connecting" || status === "starting" || status === "stopping";

  // ── Render ──
  return (
    <div className="app-layout">
      {/* ───────── SIDEBAR ───────── */}
      <aside className="sidebar">
        <div className="sidebar-brand">
          <div className="brand-icon">Z</div>
          <span className="brand-text">ZonDPI</span>
        </div>

        <nav className="sidebar-nav">
          <button className={`nav-btn ${page === "overview" ? "active" : ""}`} onClick={() => setPage("overview")}>
            <span className="nav-icon">🏠</span> Genel Bakış
          </button>
          <button className={`nav-btn ${page === "profiles" ? "active" : ""}`} onClick={() => setPage("profiles")}>
            <span className="nav-icon">📋</span> Profiller
          </button>
          <button className={`nav-btn ${page === "diagnostics" ? "active" : ""}`} onClick={() => setPage("diagnostics")}>
            <span className="nav-icon">🔍</span> Tanılama
          </button>
          <button className={`nav-btn ${page === "settings" ? "active" : ""}`} onClick={() => setPage("settings")}>
            <span className="nav-icon">⚙️</span> Ayarlar
          </button>
        </nav>

        <div className="sidebar-footer">
          <button className="about-link" onClick={() => setPage("about")}>Hakkında</button>
          <span className="version-text">v1.0.5</span>
        </div>
      </aside>

      {/* ───────── MAIN ───────── */}
      <main className="main-area">
        {/* ── DASHBOARD ── */}
        {page === "overview" && (
          <div className="animate-in">
            <div className="card dashboard-hero">
              <div className="hero-status">
                <div className={`status-ring ${statusRingClass()}`}>
                  {statusEmoji()}
                </div>
                <div className="status-label">{statusLabel()}</div>
                {status === "active" && (
                  <div className="status-sublabel">
                    Bağlantı korumanız etkin. İnternet trafiğiniz ZonDPI tarafından güvence altında.
                  </div>
                )}
                {status === "idle" && (
                  <div className="status-sublabel">
                    Koruma şu anda durdurulmuş durumda. Başlatmak için aşağıdaki düğmeyi kullanın.
                  </div>
                )}
                {status === "disconnected" && (
                  <div className="status-sublabel">
                    ZonDPI arka plan servisine ulaşılamıyor. Servisin kurulu ve çalışır durumda olduğundan emin olun.
                  </div>
                )}

                <button
                  className={`primary-action ${isRunning ? "stop" : "start"}`}
                  onClick={toggleProtection}
                  disabled={isTransitioning}
                >
                  {status === "starting" ? "Başlatılıyor…"
                    : status === "stopping" ? "Durduruluyor…"
                    : isRunning ? "Korumayı Durdur"
                    : "Korumayı Başlat"}
                </button>
              </div>
            </div>

            <div className="card-row">
              <div className="info-card">
                <div className="info-label">Çalışma Yöntemi</div>
                <div className="info-value">{autoMode ? "Otomatik" : "Manuel"}</div>
              </div>
              <div className="info-card">
                <div className="info-label">Profil</div>
                <div className="info-value">{profileDisplayName(activeProfile)}</div>
              </div>
              <div className="info-card">
                <div className="info-label">Durum</div>
                <div className="info-value">{isRunning ? "Aktif" : "Kapalı"}</div>
              </div>
              <div className="info-card">
                <div className="info-label">Uyumluluk</div>
                <div className="info-value">{recommendation ? "Uyumlu" : "Denetleniyor…"}</div>
              </div>
            </div>

            {recommendation && status !== "disconnected" && (
              <div className="alert-banner success" style={{ marginTop: 16 }}>
                ✓ Sisteminiz ZonDPI ile uyumlu. {recommendation.compatibility_rating === "excellent" ? "Mükemmel uyumluluk." : ""}
              </div>
            )}

            {status === "disconnected" && (
              <div className="alert-banner error" style={{ marginTop: 16 }}>
                ⚠ ZonDPI arka plan servisine bağlanılamadı. Lütfen Windows Hizmetleri'nden ZonDPI servisinin çalıştığını doğrulayın.
              </div>
            )}
          </div>
        )}

        {/* ── PROFILES ── */}
        {page === "profiles" && (
          <div className="animate-in">
            <div className="page-title">Bağlantı Profilleri</div>
            <div className="page-subtitle">
              Ağ ortamınıza en uygun profili seçin. Profil değişikliği bir sonraki başlatmada geçerli olur.
            </div>

            <div className="profile-list">
              {Object.entries(PROFILE_DISPLAY).map(([id, meta]) => (
                <div
                  key={id}
                  className={`profile-item ${selectedProfile === id ? "selected" : ""}`}
                  onClick={() => setSelectedProfile(id)}
                >
                  <div className="profile-radio">
                    <div className="profile-radio-inner" />
                  </div>
                  <div className="profile-info">
                    <div className="profile-name">{meta.name}</div>
                    <div className="profile-desc">{meta.desc}</div>
                  </div>
                </div>
              ))}
            </div>
          </div>
        )}

        {/* ── DIAGNOSTICS ── */}
        {page === "diagnostics" && (
          <div className="animate-in">
            <div className="page-title">Sistem Durumu</div>
            <div className="page-subtitle">Servis durumu ve ağ bileşenleri bilgileri.</div>

            <div className="diag-grid">
              <div className="diag-item">
                <span className="diag-label">Hizmet</span>
                <span className={`diag-value ${status !== "disconnected" ? "ok" : "err"}`}>
                  {status !== "disconnected" ? "Çalışıyor" : "Bağlantı Yok"}
                </span>
              </div>
              <div className="diag-item">
                <span className="diag-label">ZonDPI</span>
                <span className={`diag-value ${isRunning ? "ok" : ""}`}>
                  {isRunning ? "Aktif" : "Kapalı"}
                </span>
              </div>
              <div className="diag-item">
                <span className="diag-label">Profil</span>
                <span className="diag-value">{profileDisplayName(activeProfile)}</span>
              </div>
              <div className="diag-item">
                <span className="diag-label">Paket Filtresi</span>
                <span className={`diag-value ${isRunning ? "ok" : ""}`}>
                  {isRunning ? "Etkin" : "Devre Dışı"}
                </span>
              </div>
              <div className="diag-item">
                <span className="diag-label">DNS Uyumluluğu</span>
                <span className={`diag-value ${isRunning ? "ok" : ""}`}>
                  {isRunning ? "Etkin" : "Devre Dışı"}
                </span>
              </div>
              <div className="diag-item">
                <span className="diag-label">Ağ Etkinliği</span>
                <span className="diag-value">Bilinmiyor</span>
              </div>
            </div>

            {/* Developer Mode Technical Details */}
            {developerMode && (
              <div style={{ marginTop: 16 }}>
                <button className="details-toggle" onClick={() => setShowAdvanced(!showAdvanced)}>
                  {showAdvanced ? "Gelişmiş teknik ayrıntıları gizle" : "Gelişmiş teknik ayrıntıları göster (Geliştirici)"}
                </button>

                {showAdvanced && (
                  <div className="advanced-details">
                    <div>Teknik Motor: {activeEngine || "(yok)"}</div>
                    <div>Profil ID: {activeProfile}</div>
                    <div>IPC Borusu: \\.\pipe\zondpi-service-ipc</div>
                    {recommendation && (
                      <>
                        <div>Algılanan Güvenlik: {recommendation.detected_security_products?.join(", ") || "Yok"}</div>
                        <div>Önerilen Motor: {recommendation.recommended_engine}</div>
                        <div>Gerekçe: {recommendation.rationale}</div>
                      </>
                    )}
                  </div>
                )}
              </div>
            )}

            <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center", marginTop: 24 }}>
              <div className="page-title" style={{ margin: 0 }}>Günlükler</div>
              {developerMode && (
                <label style={{ fontSize: 12, display: "flex", alignItems: "center", gap: 6, color: "var(--text-dim)", cursor: "pointer" }}>
                  <input
                    type="checkbox"
                    checked={showRawLogs}
                    onChange={(e) => setShowRawLogs(e.target.checked)}
                  />
                  Ham Motor Günlüğünü Göster
                </label>
              )}
            </div>

            <div className="log-container">
              {logs.length === 0 ? (
                <div className="log-empty">Henüz kaydedilmiş günlük bulunmuyor.</div>
              ) : (showRawLogs && developerMode ? logs : normalizeLogs(logs)).map((log, i) => (
                <div key={i} className="log-line">
                  <span className="log-ts">[{log.timestamp}]</span> {log.line}
                </div>
              ))}
            </div>
          </div>
        )}

        {/* ── SETTINGS ── */}
        {page === "settings" && (
          <div className="animate-in">
            <div className="page-title">Ayarlar</div>
            <div className="page-subtitle">Uygulama ve koruma tercihlerinizi yönetin.</div>

            <div className="settings-section">
              <div className="settings-section-title">Koruma</div>
              <div className="setting-row">
                <div>
                  <div className="setting-label">Otomatik Çalışma Yöntemi</div>
                  <div className="setting-hint">Sistem uyumluluğuna göre en iyi yöntemi otomatik seçer.</div>
                </div>
                <label className="toggle">
                  <input type="checkbox" checked={autoMode} onChange={(e) => setAutoMode(e.target.checked)} />
                  <span className="toggle-track" />
                </label>
              </div>
              <div className="setting-row">
                <div>
                  <div className="setting-label">Windows başladığında korumayı otomatik etkinleştir</div>
                  <div className="setting-hint">Uygulama veya Windows açıldığında korumayı otomatik olarak devreye alır.</div>
                </div>
                <label className="toggle">
                  <input
                    type="checkbox"
                    checked={autoProtect}
                    onChange={(e) => {
                      setAutoProtect(e.target.checked);
                      localStorage.setItem("zondpi_auto_protect", String(e.target.checked));
                    }}
                  />
                  <span className="toggle-track" />
                </label>
              </div>
            </div>

            <div className="settings-section">
              <div className="settings-section-title">Başlangıç ve Sistem Tepsisi</div>
              <div className="setting-row">
                <div>
                  <div className="setting-label">Windows ile Birlikte Başlat</div>
                  <div className="setting-hint">Windows açıldığında ZonDPI uygulamasını otomatik çalıştırır.</div>
                </div>
                <label className="toggle">
                  <input
                    type="checkbox"
                    checked={startOnBoot}
                    onChange={(e) => {
                      setStartOnBoot(e.target.checked);
                      localStorage.setItem("zondpi_autostart", String(e.target.checked));
                    }}
                  />
                  <span className="toggle-track" />
                </label>
              </div>
              <div className="setting-row">
                <div>
                  <div className="setting-label">Başlangıçta Sistem Tepsisinde Gizle</div>
                  <div className="setting-hint">Uygulama açılışında ana pencereyi tepside gizli tutar.</div>
                </div>
                <label className="toggle">
                  <input
                    type="checkbox"
                    checked={startMinimized}
                    onChange={(e) => {
                      setStartMinimized(e.target.checked);
                      localStorage.setItem("zondpi_minimized", String(e.target.checked));
                    }}
                  />
                  <span className="toggle-track" />
                </label>
              </div>
            </div>

            <div className="settings-section">
              <div className="settings-section-title">Gelişmiş</div>
              <div className="setting-row">
                <div>
                  <div className="setting-label">Geliştirici Modu</div>
                  <div className="setting-hint">Teknik ayrıntıları ve ham motor günlüklerini erişilebilir kılar.</div>
                </div>
                <label className="toggle">
                  <input
                    type="checkbox"
                    checked={developerMode}
                    onChange={(e) => {
                      setDeveloperMode(e.target.checked);
                      localStorage.setItem("zondpi_developer_mode", String(e.target.checked));
                      if (!e.target.checked) setShowRawLogs(false);
                    }}
                  />
                  <span className="toggle-track" />
                </label>
              </div>
            </div>

            <div className="settings-section">
              <div className="settings-section-title">Gizlilik ve DNS</div>
              <div className="privacy-box">
                ZonDPI telemetri veya analiz verisi toplamaz; merkezi bir DNS veya günlük sunucusu barındırmaz.
                Ziyaret edilen siteler, alan adları ve DNS geçmişi ZonDPI tarafından kaydedilmez.
                Varsayılan uyumluluk profilinde ISS DNS zehirlemesini önlemek için DNS çözümlemesi seçilen temiz üçüncü taraf sağlayıcıya (Cloudflare 1.1.1.1) şeffaf olarak yönlendirilir.
              </div>
            </div>
          </div>
        )}

        {/* ── ABOUT ── */}
        {page === "about" && (
          <div className="animate-in">
            <div className="about-header">
              <div className="about-icon">Z</div>
              <div className="page-title">ZonDPI</div>
              <div className="about-version">Sürüm 1.0.5</div>
              <div className="about-desc">
                Açık kaynak Windows bağlantı yardımcı aracı. Türkiye'deki DPI kaynaklı erişim kısıtlamalarını aşmak için geliştirilmiştir.
              </div>
            </div>

            <div className="page-title" style={{ fontSize: 14 }}>Açık Kaynak Bileşenleri</div>
            <div className="component-list">
              <div className="component-item">
                <span className="component-name">GoodbyeDPI</span>
                <span className="component-license">Apache-2.0</span>
              </div>
              <div className="component-item">
                <span className="component-name">ByeDPI</span>
                <span className="component-license">MIT</span>
              </div>
              <div className="component-item">
                <span className="component-name">WinDivert</span>
                <span className="component-license">LGPL-3.0</span>
              </div>
              <div className="component-item">
                <span className="component-name">Tauri</span>
                <span className="component-license">MIT / Apache-2.0</span>
              </div>
              <div className="component-item">
                <span className="component-name">React</span>
                <span className="component-license">MIT</span>
              </div>
              <div className="component-item">
                <span className="component-name">Tokio</span>
                <span className="component-license">MIT</span>
              </div>
            </div>
          </div>
        )}
      </main>
    </div>
  );
}

export default App;
