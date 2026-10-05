//! Presença DDS do nó Studio (T-890 — 19º tópico canônico
//! `Studio.NodePresence`, decisão de 2026-10-05: descoberta DDS-nativa das
//! instalações do Studio; mDNS eliminado — DDS é requisito mínimo por
//! máquina).
//!
//! Atrás da feature `dds` (como no Studio desktop): o publicador sobe um
//! `DataSpace` real e publica um heartbeat a cada [`HEARTBEAT`] (5 s) na
//! instância `node_id` (@key) — os pares veem a instalação viva e o Liveliness
//! ManualByTopic (lease 10 s) a dá por morta em ≤ 10 s sem `dispose`.
//! Sem `STUDIO_NODE_DDS_DOMAIN` a presença fica desativada (nó HTTP-only,
//! descobrível só pelo catálogo local do Studio) — falha de DDS NUNCA derruba
//! o nó administrativo.

/// Período do heartbeat de presença (5 s) — o lease ManualByTopic do tópico é
/// de 10 s (`dds_dataspace::qos::profiles::studio_node_presence`): dois beats
/// perdidos ainda cabem no lease.
pub const HEARTBEAT: std::time::Duration = std::time::Duration::from_secs(5);

/// Variável de ambiente que ativa a presença (nº do domínio DDS).
pub const ENV_DDS_DOMAIN: &str = "STUDIO_NODE_DDS_DOMAIN";

/// `now` em ns UNIX — `last_seen_ns` é comparado ENTRE máquinas (wall clock;
/// skew de NTP aparece no campo, não na entrega: quem ordena a entrega é o
/// DDS).
#[must_use]
pub fn now_unix_ns() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or_default()
}

/// Resolve o domínio DDS da presença a partir do valor cru da env.
///
/// Função pura (testável sem DDS): `None` = presença DESATIVADA (env ausente
/// ou vazia — normal, é o modo HTTP-only); `Some(dominio)` = publicar. Valor
/// não-numérico também desativa (com log no chamador) — um erro de digitação
/// nunca deve derrubar o nó administrativo.
#[must_use]
pub fn dds_domain_from_env(raw: Option<String>) -> Option<u32> {
    let raw = raw?;
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }
    trimmed.parse::<u32>().ok()
}

#[cfg(feature = "dds")]
pub use dds_impl::{presence_sample, spawn_from_env, PresencePublisher};

#[cfg(feature = "dds")]
mod dds_impl {
    //! Publicador de presença (requer a feature `dds`).

    use super::{dds_domain_from_env, now_unix_ns, ENV_DDS_DOMAIN, HEARTBEAT};
    use dds_contract::generated::dds_llm_orchestrator::StudioNodePresence;
    use dds_dataspace::DataSpace;

    /// Versão do protocolo administrativo anunciada na presença (a presença
    /// descreve ESTE servidor HTTP — reuso da fonte única do `protocol`).
    fn protocol_version() -> (i32, i32) {
        let v = crate::protocol::NODE_PROTOCOL_VERSION;
        (i32::from(v.major), i32::from(v.minor))
    }

    /// Registro puro do wire (sem entidade DDS — testável com a feature
    /// ligada): monta a amostra do heartbeat com `last_seen_ns` explícito.
    #[must_use]
    pub fn presence_sample(
        now_ns: u64,
        node_id: &str,
        url: &str,
        token_required: bool,
        services_hint: &str,
    ) -> StudioNodePresence {
        let (protocol_major, protocol_minor) = protocol_version();
        StudioNodePresence {
            node_id: node_id.to_owned(),
            url: url.to_owned(),
            protocol_major,
            protocol_minor,
            token_required,
            services_hint: services_hint.to_owned(),
            last_seen_ns: now_ns,
        }
    }

    /// Publicador de `Studio.NodePresence`: um `DataSpace` dedicado e um
    /// heartbeat por instância `node_id` (@key — o DDS sobrescreve a amostra
    /// anterior; KeepLast(1) no tópico).
    pub struct PresencePublisher {
        space: DataSpace,
        node_id: String,
        url: String,
        token_required: bool,
        services_hint: String,
    }

    impl PresencePublisher {
        /// Sobe o publicador no domínio. `ownership 0` de propósito: o nó
        /// Studio NÃO disputa `Tasks` — só publica presença (mesmo padrão do
        /// `dds_observe` do Studio desktop).
        ///
        /// Falível: criação de participant/tópicos DDS pode falhar — `Err`
        /// em vez de panic; o chamador registra e segue HTTP-only.
        pub fn new(
            domain: u32,
            node_id: String,
            url: String,
            token_required: bool,
            services_hint: String,
        ) -> Result<Self, dds_dataspace::api::DataSpaceError> {
            let space = DataSpace::new(domain, 0)?;
            Ok(Self {
                space,
                node_id,
                url,
                token_required,
                services_hint,
            })
        }

        /// Amostra do heartbeat com `last_seen_ns` carimbado agora.
        #[must_use]
        pub fn beat(&self, now_ns: u64) -> StudioNodePresence {
            presence_sample(
                now_ns,
                &self.node_id,
                &self.url,
                self.token_required,
                &self.services_hint,
            )
        }

        /// Loop do heartbeat: publica imediatamente (o primeiro tick do
        /// `interval` não espera) e a cada [`HEARTBEAT`]. Roda até a task ser
        /// cancelada (o processo é quem morre); erro de escrita é registrado
        /// e o loop continua — um domínio DDS instável não pode silenciar o
        /// nó mais do que o lease já faria.
        pub async fn run(self) {
            let mut ticker = tokio::time::interval(HEARTBEAT);
            ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
            loop {
                ticker.tick().await;
                let sample = self.beat(now_unix_ns());
                if let Err(e) = self.space.write_studio_node_presence(&sample) {
                    eprintln!("studio-noded: aviso: heartbeat Studio.NodePresence falhou: {e}");
                }
            }
        }
    }

    /// Deriva `node_id`/`url`/`token_required`/`services_hint` do ambiente do
    /// nó e faz o spawn da task de presença — ponto único de acoplamento com
    /// o `main` (chamado só com a feature `dds`).
    pub fn spawn_from_env(
        bind: std::net::IpAddr,
        port: u16,
        token_present: bool,
        services: &[String],
    ) {
        let Some(domain) = dds_domain_from_env(std::env::var(ENV_DDS_DOMAIN).ok()) else {
            eprintln!(
                "studio-noded: info: presenca DDS desativada ({ENV_DDS_DOMAIN} ausente) — \
                 descoberta apenas pelo catalogo local"
            );
            return;
        };
        let url = format!("http://{bind}:{port}");
        let node_id = std::env::var("STUDIO_NODE_ID")
            .ok()
            .filter(|id| !id.trim().is_empty())
            .unwrap_or_else(|| format!("{}:{port}", hostname_or_bind(bind)));
        let publisher =
            match PresencePublisher::new(domain, node_id, url, token_present, services.join(",")) {
                Ok(publisher) => publisher,
                Err(e) => {
                    eprintln!(
                        "studio-noded: aviso: presenca DDS nao subiu (dominio {domain}): {e} — \
                     no segue HTTP-only"
                    );
                    return;
                }
            };
        eprintln!("studio-noded: presença DDS ativa (domínio {domain})");
        tokio::spawn(publisher.run());
    }

    /// Identidade padrão do nó: hostname do ambiente quando existe (a porta
    /// desambigua múltiplos nós na mesma máquina), `bind` caso contrário.
    fn hostname_or_bind(bind: std::net::IpAddr) -> String {
        std::env::var("HOSTNAME")
            .ok()
            .filter(|h| !h.trim().is_empty())
            .unwrap_or_else(|| bind.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::{dds_domain_from_env, ENV_DDS_DOMAIN, HEARTBEAT};

    // T-890: a env decide a presença — ausente/vazia/lixo desativam (nó
    // segue HTTP-only), inteiro válido ativa. Função pura: sem DDS.
    #[test]
    fn dds_domain_env_resolves_presence_on_off() {
        assert_eq!(dds_domain_from_env(None), None, "env ausente desativa");
        assert_eq!(
            dds_domain_from_env(Some(String::new())),
            None,
            "env vazia desativa"
        );
        assert_eq!(
            dds_domain_from_env(Some("   ".into())),
            None,
            "env só de espaço desativa"
        );
        assert_eq!(
            dds_domain_from_env(Some("nao-e-numero".into())),
            None,
            "lixo desativa (não derruba o nó)"
        );
        assert_eq!(dds_domain_from_env(Some("0".into())), Some(0));
        assert_eq!(
            dds_domain_from_env(Some(" 42 ".into())),
            Some(42),
            "espaços em volta são tolerados"
        );
    }

    #[test]
    fn heartbeat_is_five_seconds_under_ten_second_lease() {
        // Duas omissões de beat ainda cabem no lease ManualByTopic de 10 s.
        assert_eq!(HEARTBEAT.as_secs(), 5);
        assert!(HEARTBEAT.as_nanos() < 10_000_000_000);
    }

    #[test]
    fn env_constant_is_stable() {
        // Contrato operacional documentado: renomear quebra deploy (Ansible)
        // e GUI.
        assert_eq!(ENV_DDS_DOMAIN, "STUDIO_NODE_DDS_DOMAIN");
    }
}

#[cfg(all(test, feature = "dds"))]
mod dds_tests {
    use super::dds_impl::presence_sample;
    use super::now_unix_ns;

    // T-890: construção da amostra de presença — a forma do wire (typename,
    // @key, roundtrip XCDR) é gate da dds-contract; aqui vale o conteúdo que
    // o nó publica. Função pura: nenhuma entidade DDS no teste.
    #[test]
    fn presence_sample_stamps_identity_and_fresh_last_seen_ns() {
        let before = now_unix_ns();
        let sample = presence_sample(
            before,
            "no-lab-1",
            "http://127.0.0.1:4317",
            true,
            "dds-agent,llm-local",
        );
        assert_eq!(sample.node_id, "no-lab-1");
        assert_eq!(sample.url, "http://127.0.0.1:4317");
        // Versão do protocolo vem da fonte única do `protocol` do nó (1.0).
        assert_eq!(sample.protocol_major, 1);
        assert_eq!(sample.protocol_minor, 0);
        assert!(sample.token_required);
        assert_eq!(sample.services_hint, "dds-agent,llm-local");
        assert_eq!(sample.last_seen_ns, before);
        assert!(sample.last_seen_ns > 0, "carimbo de agora não pode ser 0");
    }
}
