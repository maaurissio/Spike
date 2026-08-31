//! Un único trabajador limita la concurrencia de red; la TUI nunca espera una respuesta.
use std::{
    fs, io,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, SyncSender, TryRecvError},
    },
    thread,
    time::{SystemTime, UNIX_EPOCH},
};

use crate::{
    config::{self, Config},
    providers::{
        GameStateSource, HistorySource, LiveMatchSource, LocalClientSource, MatchDetailSource,
        PlayerProfileSource, ProcessGameStateSource, StateInfo,
        capabilities::GamePhase,
        live_match::LiveMatchContext,
        profile::{CompetitiveProfile, CompetitiveUpdate, OwnProfile},
        resolve_with_fallback,
    },
    watch::{Watcher, log_transition},
};

pub(super) enum Request {
    Observe { log: bool },
    Context { phase: GamePhase, generation: u64 },
    Profile { epoch: u64 },
    History { epoch: u64 },
    Save(Config),
}

pub(super) enum Context {
    Live(LiveMatchContext),
    Completed(super::PostMatch),
}

pub(super) enum Reply {
    Observed {
        state: Result<StateInfo, ()>,
        log_failed: bool,
    },
    Context {
        generation: u64,
        data: Result<Context, ()>,
    },
    ContextProgress {
        generation: u64,
        percent: u16,
        label: &'static str,
    },
    Profile {
        epoch: u64,
        data: Result<
            (
                OwnProfile,
                Option<CompetitiveProfile>,
                Vec<CompetitiveUpdate>,
            ),
            (),
        >,
    },
    History {
        epoch: u64,
        data: Result<Vec<super::HistoryItem>, ()>,
    },
    Saved(Result<Config, ()>),
}

pub(super) struct Worker {
    requests: SyncSender<Request>,
    replies: Receiver<Reply>,
    stop: Arc<AtomicBool>,
}

impl Worker {
    /// No construye Sources ni clientes HTTP, y jamás escribe en disco.
    pub fn demo() -> io::Result<Self> {
        Self::spawn(|request, _| match request {
            Request::Save(config) => Reply::Saved(Ok(config)),
            Request::Observe { .. } => Reply::Observed {
                state: Err(()),
                log_failed: false,
            },
            Request::Context { generation, .. } => Reply::Context {
                generation,
                data: Err(()),
            },
            Request::Profile { epoch } => Reply::Profile {
                epoch,
                data: Err(()),
            },
            Request::History { epoch } => Reply::History {
                epoch,
                data: Err(()),
            },
        })
    }

    pub fn start() -> io::Result<Self> {
        let mut sources = None;
        Self::spawn_with_progress(move |request, stop, progress| {
            sources
                .get_or_insert_with(Sources::new)
                .handle(request, stop, progress)
        })
    }

    pub(super) fn spawn(
        mut handle: impl FnMut(Request, &AtomicBool) -> Reply + Send + 'static,
    ) -> io::Result<Self> {
        Self::spawn_with_progress(move |request, stop, _| handle(request, stop))
    }

    fn spawn_with_progress(
        mut handle: impl FnMut(Request, &AtomicBool, &mut dyn FnMut(Reply)) -> Reply + Send + 'static,
    ) -> io::Result<Self> {
        let (requests, incoming) = mpsc::sync_channel(4);
        let (outgoing, replies) = mpsc::sync_channel(8);
        let stop = Arc::new(AtomicBool::new(false));
        let stopped = Arc::clone(&stop);
        thread::Builder::new()
            .name("spike-tui-data".into())
            .spawn(move || {
                while let Ok(request) = incoming.recv() {
                    if stopped.load(Ordering::Acquire) {
                        break;
                    }
                    let mut progress = |reply| {
                        let _ = outgoing.send(reply);
                    };
                    let reply = handle(request, &stopped, &mut progress);
                    if stopped.load(Ordering::Acquire) || outgoing.send(reply).is_err() {
                        break;
                    }
                }
            })?;
        Ok(Self {
            requests,
            replies,
            stop,
        })
    }

    /// Nunca bloquea la lectura de teclas, incluso si se llena la cola.
    pub fn submit(&self, request: Request) -> bool {
        self.requests.try_send(request).is_ok()
    }

    pub fn receive(&self) -> Result<Reply, TryRecvError> {
        self.replies.try_recv()
    }
}

impl Drop for Worker {
    fn drop(&mut self) {
        // No iniciar trabajos pendientes después de salir. Un GET en curso
        // conserva su timeout; no esperamos su terminación para cerrar la TUI.
        self.stop.store(true, Ordering::Release);
    }
}

struct Sources {
    local: LocalClientSource,
    process: ProcessGameStateSource,
    history: HistorySource,
    live: LiveMatchSource,
    details: MatchDetailSource,
    profile: PlayerProfileSource,
    watcher: Watcher,
    simulation: bool,
}

impl Sources {
    fn new() -> Self {
        let local = LocalClientSource::new();
        let simulation = std::env::var_os("SPIKE_STATE").is_some();
        if !simulation {
            local.start_event_listener();
        }
        Self {
            local,
            simulation,
            process: ProcessGameStateSource::new(),
            history: HistorySource::new(),
            live: LiveMatchSource::new(),
            details: MatchDetailSource::new(),
            profile: PlayerProfileSource::new(),
            watcher: Watcher::default(),
        }
    }

    fn handle(
        &mut self,
        request: Request,
        stop: &AtomicBool,
        progress: &mut dyn FnMut(Reply),
    ) -> Reply {
        match request {
            Request::Observe { log } => {
                let state = if self.simulation {
                    self.process.fetch()
                } else {
                    resolve_with_fallback(&self.local, &self.process)
                };
                let mut log_failed = false;
                if let Ok(info) = &state
                    && let Some(transition) = self.watcher.observe(info)
                    && log
                    && !self.simulation
                {
                    log_failed = log_transition(&transition).is_err();
                }
                Reply::Observed {
                    state: state.map_err(|_| ()),
                    log_failed,
                }
            }
            Request::Context { phase, generation } => Reply::Context {
                generation,
                data: self.context(phase, generation, stop, progress),
            },
            Request::Profile { epoch } => Reply::Profile {
                epoch,
                data: self.profile(stop),
            },
            Request::History { epoch } => Reply::History {
                epoch,
                data: if self.simulation {
                    Err(())
                } else {
                    self.history(stop)
                },
            },
            Request::Save(config) => {
                let result = config::save(&config).map(|_| config).map_err(|_| ());
                Reply::Saved(result)
            }
        }
    }

    fn context(
        &self,
        phase: GamePhase,
        generation: u64,
        stop: &AtomicBool,
        progress: &mut dyn FnMut(Reply),
    ) -> Result<Context, ()> {
        if self.simulation || stop.load(Ordering::Acquire) {
            return Err(());
        }
        progress(Reply::ContextProgress {
            generation,
            percent: 15,
            label: "Leyendo la sesión local",
        });
        match phase {
            GamePhase::PreGame | GamePhase::AgentSelect | GamePhase::InMatch => {
                let request = self.local.live_match_request(phase).map_err(|_| ())?;
                progress(Reply::ContextProgress {
                    generation,
                    percent: 45,
                    label: "Partida detectada",
                });
                let context = self.live.fetch(&request).map_err(|_| ())?;
                progress(Reply::ContextProgress {
                    generation,
                    percent: 90,
                    label: "Preparando jugadores y estadísticas",
                });
                Ok(Context::Live(context))
            }
            GamePhase::PostMatch => {
                let request = self.local.match_detail_request().map_err(|_| ())?;
                progress(Reply::ContextProgress {
                    generation,
                    percent: 45,
                    label: "Resultado encontrado",
                });
                let completed = self.details.fetch_completed(&request).map_err(|_| ())?;
                progress(Reply::ContextProgress {
                    generation,
                    percent: 90,
                    label: "Preparando el resumen final",
                });
                Ok(Context::Completed(super::PostMatch::from_completed(
                    completed,
                )))
            }
            _ => Err(()),
        }
    }

    fn profile(
        &self,
        stop: &AtomicBool,
    ) -> Result<
        (
            OwnProfile,
            Option<CompetitiveProfile>,
            Vec<CompetitiveUpdate>,
        ),
        (),
    > {
        if self.simulation || stop.load(Ordering::Acquire) {
            return Err(());
        }
        let request = self.local.profile_request().map_err(|_| ())?;
        let profile = self.profile.fetch_own(&request).map_err(|_| ())?;
        if stop.load(Ordering::Acquire) {
            return Err(());
        }
        let mut competitive = self.profile.fetch_own_competitive(&request).ok().flatten();
        let updates = self
            .profile
            .fetch_own_competitive_updates(&request, 5)
            .unwrap_or_default();
        if competitive.is_none() {
            competitive = updates
                .first()
                .and_then(CompetitiveProfile::from_latest_update);
        }
        Ok((profile, competitive, updates))
    }

    fn history(&self, stop: &AtomicBool) -> Result<Vec<super::HistoryItem>, ()> {
        if stop.load(Ordering::Acquire) {
            return Err(());
        }
        let request = self.local.history_request(20).map_err(|_| ())?;
        let updates = self
            .profile
            .fetch_own_competitive_updates(&request.profile_request(), 20)
            .unwrap_or_default();
        let matches = self.history.fetch_own_matches(&request).map_err(|_| ())?;
        let mut indexed = Vec::with_capacity(matches.len());
        let mut matches = matches.into_iter().enumerate();
        loop {
            let batch = matches.by_ref().take(5).collect::<Vec<_>>();
            if batch.is_empty() || stop.load(Ordering::Acquire) {
                break;
            }
            let mut results = thread::scope(|scope| {
                batch
                    .into_iter()
                    .map(|(index, item)| {
                        let detail_request = request.match_detail_request(item.match_id);
                        let rr_change = updates.get(index).map(|update| update.rr_earned);
                        let rr_after = updates
                            .get(index)
                            .and_then(|update| update.ranked_rating_after);
                        scope.spawn(move || {
                            let details =
                                self.details
                                    .fetch_own_totals(&detail_request)
                                    .ok()
                                    .map(|totals| super::HistoryDetails {
                                        map: totals.map,
                                        agent: totals.agent,
                                        outcome: totals.stats.outcome,
                                        rounds_played: totals.stats.rounds_played,
                                        stats: totals.stats.stats,
                                        own_score: totals.own_score,
                                        opponent_score: totals.opponent_score,
                                    });
                            (
                                index,
                                super::HistoryItem {
                                    entry: item.entry,
                                    details,
                                    rr_change,
                                    rr_after,
                                },
                            )
                        })
                    })
                    .collect::<Vec<_>>()
                    .into_iter()
                    .map(|handle| handle.join().expect("trabajador de historial"))
                    .collect::<Vec<_>>()
            });
            indexed.append(&mut results);
        }
        indexed.sort_by_key(|(index, _)| *index);
        let items = indexed
            .into_iter()
            .map(|(_, item)| item)
            .collect::<Vec<_>>();
        let _ = save_cached_history(&items);
        Ok(items)
    }
}

fn history_cache_path() -> Option<std::path::PathBuf> {
    config::config_path().map(|path| path.with_file_name("history-cache.json"))
}

fn save_cached_history(items: &[super::HistoryItem]) -> Result<(), ()> {
    let path = history_cache_path().ok_or(())?;
    let parent = path.parent().ok_or(())?;
    fs::create_dir_all(parent).map_err(|_| ())?;
    let cached = super::CachedHistory {
        schema: 1,
        saved_at_ms: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| ())?
            .as_millis() as u64,
        items: items.iter().take(20).cloned().collect(),
    };
    let bytes = serde_json::to_vec(&cached).map_err(|_| ())?;
    let temporary = path.with_extension("json.tmp");
    fs::write(&temporary, bytes).map_err(|_| ())?;
    fs::rename(&temporary, &path).map_err(|_| {
        let _ = fs::remove_file(&temporary);
    })
}

pub(super) fn load_cached_history() -> Option<super::CachedHistory> {
    let bytes = fs::read(history_cache_path()?).ok()?;
    let mut cached: super::CachedHistory = serde_json::from_slice(&bytes).ok()?;
    if cached.schema != 1 || cached.saved_at_ms == 0 {
        return None;
    }
    cached.items.truncate(20);
    Some(cached)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn persisted_history_schema_contains_no_session_identifiers_or_tokens() {
        let cached = super::super::CachedHistory {
            schema: 1,
            saved_at_ms: 1,
            items: vec![super::super::HistoryItem {
                entry: crate::providers::history::HistoryEntry {
                    queue: "competitivo".into(),
                    started_at_ms: 2,
                },
                details: None,
                rr_change: Some(18),
                rr_after: Some(64),
            }],
        };

        let json = serde_json::to_string(&cached).unwrap();

        for forbidden in ["MatchID", "puuid", "access_token", "entitlement"] {
            assert!(!json.contains(forbidden), "{json}");
        }
    }

    #[test]
    fn slow_work_is_bounded_and_does_not_block_submission_or_shutdown() {
        let (started_tx, started_rx) = mpsc::channel();
        let (release_tx, release_rx) = mpsc::channel();
        let (ended_tx, ended_rx) = mpsc::channel();
        let worker = Worker::spawn(move |_, _| {
            started_tx.send(()).unwrap();
            release_rx.recv_timeout(Duration::from_secs(2)).unwrap();
            ended_tx.send(()).unwrap();
            Reply::History {
                epoch: 0,
                data: Ok(vec![]),
            }
        })
        .unwrap();
        assert!(worker.submit(Request::History { epoch: 0 }));
        started_rx.recv_timeout(Duration::from_secs(2)).unwrap();
        for _ in 0..4 {
            assert!(worker.submit(Request::History { epoch: 0 }));
        }
        assert!(!worker.submit(Request::History { epoch: 0 }));
        assert!(matches!(worker.receive(), Err(TryRecvError::Empty)));
        drop(worker);
        release_tx.send(()).unwrap();
        ended_rx.recv_timeout(Duration::from_secs(2)).unwrap();
        // El manejador se destruye sin ejecutar las cuatro consultas pendientes.
        assert!(started_rx.recv_timeout(Duration::from_secs(2)).is_err());
    }
}
