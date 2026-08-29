//! Un único trabajador limita la concurrencia de red; la TUI nunca espera una respuesta.
use std::{
    io,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, SyncSender, TryRecvError},
    },
    thread,
};

use crate::{
    config::{self, Config},
    providers::{
        GameStateSource, HistorySource, LiveMatchSource, LocalClientSource, MatchDetailSource,
        PlayerProfileSource, ProcessGameStateSource, StateInfo,
        capabilities::GamePhase,
        history::HistoryEntry,
        live_match::LiveMatchContext,
        profile::{CompetitiveProfile, CompetitiveUpdate, OwnProfile},
        resolve_with_fallback,
    },
    watch::{Watcher, log_transition},
};

pub(super) enum Request {
    Observe { log: bool },
    Context { phase: GamePhase, generation: u64 },
    History { epoch: u64 },
    Save(Config),
}

pub(super) enum Context {
    Live(LiveMatchContext),
    Profile(
        OwnProfile,
        Option<CompetitiveProfile>,
        Vec<CompetitiveUpdate>,
    ),
    /// Texto normalizado: sin IDs ni roster en el estado de pantalla.
    Completed(String),
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
    History {
        epoch: u64,
        data: Result<Vec<HistoryEntry>, ()>,
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
            Request::History { epoch } => Reply::History {
                epoch,
                data: Err(()),
            },
        })
    }

    pub fn start() -> io::Result<Self> {
        let mut sources = None;
        Self::spawn(move |request, stop| {
            sources
                .get_or_insert_with(Sources::new)
                .handle(request, stop)
        })
    }

    pub(super) fn spawn(
        mut handle: impl FnMut(Request, &AtomicBool) -> Reply + Send + 'static,
    ) -> io::Result<Self> {
        let (requests, incoming) = mpsc::sync_channel(4);
        let (outgoing, replies) = mpsc::sync_channel(4);
        let stop = Arc::new(AtomicBool::new(false));
        let stopped = Arc::clone(&stop);
        thread::Builder::new()
            .name("vtracker-tui-data".into())
            .spawn(move || {
                while let Ok(request) = incoming.recv() {
                    if stopped.load(Ordering::Acquire) {
                        break;
                    }
                    let reply = handle(request, &stopped);
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
        let simulation = std::env::var_os("VTRACKER_STATE").is_some();
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

    fn handle(&mut self, request: Request, stop: &AtomicBool) -> Reply {
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
                data: self.context(phase, stop),
            },
            Request::History { epoch } => Reply::History {
                epoch,
                data: if self.simulation {
                    Err(())
                } else {
                    self.local
                        .history_request(10)
                        .and_then(|request| self.history.fetch_own(&request))
                        .map_err(|_| ())
                },
            },
            Request::Save(config) => {
                let result = config::save(&config).map(|_| config).map_err(|_| ());
                Reply::Saved(result)
            }
        }
    }

    fn context(&self, phase: GamePhase, stop: &AtomicBool) -> Result<Context, ()> {
        if self.simulation || stop.load(Ordering::Acquire) {
            return Err(());
        }
        match phase {
            GamePhase::InMatch => self
                .local
                .live_match_request()
                .and_then(|request| self.live.fetch(&request))
                .map(Context::Live)
                .map_err(|_| ()),
            GamePhase::PostMatch => self
                .local
                .match_detail_request()
                .and_then(|request| self.details.fetch_completed(&request))
                .map(|completed| Context::Completed(super::completed_match_content(&completed)))
                .map_err(|_| ()),
            GamePhase::Idle => {
                let request = self.local.profile_request().map_err(|_| ())?;
                if stop.load(Ordering::Acquire) {
                    return Err(());
                }
                let profile = self.profile.fetch_own(&request).map_err(|_| ())?;
                if stop.load(Ordering::Acquire) {
                    return Err(());
                }
                let competitive = self.profile.fetch_own_competitive(&request).ok().flatten();
                if stop.load(Ordering::Acquire) {
                    return Err(());
                }
                let updates = self
                    .profile
                    .fetch_own_competitive_updates(&request, 5)
                    .unwrap_or_default();
                Ok(Context::Profile(profile, competitive, updates))
            }
            _ => Err(()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

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
