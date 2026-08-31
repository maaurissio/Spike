use std::{
    collections::VecDeque,
    time::{Duration, Instant},
};

const SAMPLE_INTERVAL: Duration = Duration::from_secs(1);
const HISTORY_LIMIT: usize = 60;

#[derive(Clone, Copy, Debug, Default)]
pub(super) struct MetricSample {
    pub cpu_percent: Option<f64>,
    pub memory_bytes: Option<u64>,
}

pub(super) struct ProcessMetrics {
    started_at: Instant,
    last_sample_at: Instant,
    last_cpu_time: Option<u64>,
    current: MetricSample,
    history: VecDeque<MetricSample>,
    peak_cpu: Option<(f64, Duration)>,
    peak_memory: Option<(u64, Duration)>,
}

impl ProcessMetrics {
    pub fn new() -> Self {
        let now = Instant::now();
        let current = MetricSample {
            cpu_percent: None,
            memory_bytes: process_memory_bytes(),
        };
        Self {
            started_at: now,
            last_sample_at: now,
            last_cpu_time: process_cpu_time(),
            current,
            history: VecDeque::from([current]),
            peak_cpu: None,
            peak_memory: current.memory_bytes.map(|value| (value, Duration::ZERO)),
        }
    }

    pub fn tick(&mut self) -> bool {
        let elapsed = self.last_sample_at.elapsed();
        if elapsed < SAMPLE_INTERVAL {
            return false;
        }
        let cpu_time = process_cpu_time();
        let cpu_percent = match (self.last_cpu_time, cpu_time) {
            (Some(previous), Some(current)) => {
                let cpu_seconds = current.saturating_sub(previous) as f64 / 10_000_000.0;
                let cores =
                    std::thread::available_parallelism().map_or(1.0, |value| value.get() as f64);
                Some((cpu_seconds / elapsed.as_secs_f64() / cores * 100.0).clamp(0.0, 100.0))
            }
            _ => None,
        };
        self.current = MetricSample {
            cpu_percent,
            memory_bytes: process_memory_bytes(),
        };
        let at = self.uptime();
        if let Some(cpu) = self.current.cpu_percent
            && self.peak_cpu.is_none_or(|(peak, _)| cpu > peak)
        {
            self.peak_cpu = Some((cpu, at));
        }
        if let Some(memory) = self.current.memory_bytes
            && self.peak_memory.is_none_or(|(peak, _)| memory > peak)
        {
            self.peak_memory = Some((memory, at));
        }
        self.last_cpu_time = cpu_time;
        self.last_sample_at = Instant::now();
        self.history.push_back(self.current);
        while self.history.len() > HISTORY_LIMIT {
            self.history.pop_front();
        }
        true
    }

    pub fn current(&self) -> MetricSample {
        self.current
    }

    pub fn uptime(&self) -> Duration {
        self.started_at.elapsed()
    }

    pub fn history(&self) -> &VecDeque<MetricSample> {
        &self.history
    }

    pub fn peak_cpu(&self) -> Option<(f64, Duration)> {
        self.peak_cpu
    }

    pub fn peak_memory(&self) -> Option<(u64, Duration)> {
        self.peak_memory
    }
}

#[cfg(target_os = "windows")]
fn process_memory_bytes() -> Option<u64> {
    use windows_sys::Win32::System::{
        ProcessStatus::{K32GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS},
        Threading::GetCurrentProcess,
    };

    // SAFETY: GetCurrentProcess devuelve un pseudo-handle válido para el
    // proceso actual y `counters` vive durante toda la llamada.
    unsafe {
        let mut counters = PROCESS_MEMORY_COUNTERS::default();
        let size = u32::try_from(std::mem::size_of::<PROCESS_MEMORY_COUNTERS>()).ok()?;
        (K32GetProcessMemoryInfo(GetCurrentProcess(), &mut counters, size) != 0)
            .then_some(counters.WorkingSetSize as u64)
    }
}

#[cfg(not(target_os = "windows"))]
fn process_memory_bytes() -> Option<u64> {
    None
}

#[cfg(target_os = "windows")]
fn process_cpu_time() -> Option<u64> {
    use windows_sys::Win32::{
        Foundation::FILETIME,
        System::Threading::{GetCurrentProcess, GetProcessTimes},
    };

    fn ticks(value: FILETIME) -> u64 {
        (u64::from(value.dwHighDateTime) << 32) | u64::from(value.dwLowDateTime)
    }

    // SAFETY: todos los punteros apuntan a FILETIME inicializados y válidos
    // durante la llamada; el pseudo-handle pertenece al proceso actual.
    unsafe {
        let mut creation = FILETIME::default();
        let mut exit = FILETIME::default();
        let mut kernel = FILETIME::default();
        let mut user = FILETIME::default();
        (GetProcessTimes(
            GetCurrentProcess(),
            &mut creation,
            &mut exit,
            &mut kernel,
            &mut user,
        ) != 0)
            .then(|| ticks(kernel).saturating_add(ticks(user)))
    }
}

#[cfg(not(target_os = "windows"))]
fn process_cpu_time() -> Option<u64> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn starts_with_a_bounded_history_and_monotonic_uptime() {
        let metrics = ProcessMetrics::new();
        assert_eq!(metrics.history().len(), 1);
        assert!(metrics.uptime() <= Duration::from_secs(1));
    }
}
