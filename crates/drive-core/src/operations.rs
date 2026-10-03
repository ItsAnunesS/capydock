//! Bounded lanes keep navigation and indexing independent of long transfers.
//! Authentication and executable replacement are exclusive barriers.
use crate::{now, Result};
use serde::Serialize;
use std::{
    collections::VecDeque,
    future::Future,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
};
use tokio::sync::Notify;

pub const CANCELLED: &str = "Operação cancelada.";
pub const PREEMPTED: &str = "Consulta interrompida para manutenção; será retomada automaticamente.";
const HISTORY_LIMIT: usize = 30;
const WAITING_LIMIT: usize = 256;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    Queued,
    Running,
    Cancelling,
    Completed,
    Failed,
    Cancelled,
}
impl Status {
    fn active(self) -> bool {
        matches!(self, Self::Running | Self::Cancelling)
    }
    fn pending(self) -> bool {
        self == Self::Queued || self.active()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Lane {
    Transfer,
    Interactive,
    Background,
    Exclusive,
}
impl Lane {
    fn capacity(self) -> usize {
        match self {
            Self::Interactive => 2,
            _ => 1,
        }
    }
    fn interruptible(self) -> bool {
        matches!(self, Self::Interactive | Self::Background)
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Operation {
    pub id: String,
    pub kind: String,
    pub lane: Lane,
    pub title: String,
    pub detail: String,
    pub pair_id: Option<String>,
    pub automatic: bool,
    pub status: Status,
    pub created_at: u64,
    pub started_at: Option<u64>,
    pub finished_at: Option<u64>,
    pub error: Option<String>,
    pub can_cancel_running: bool,
}

pub struct Spec {
    pub kind: String,
    pub lane: Lane,
    pub title: String,
    pub detail: String,
    pub pair_id: Option<String>,
    pub automatic: bool,
    pub can_cancel_running: bool,
}
impl Spec {
    pub fn new(kind: &str, title: impl Into<String>, detail: impl Into<String>) -> Self {
        Self {
            kind: kind.into(),
            lane: Lane::Transfer,
            title: title.into(),
            detail: detail.into(),
            pair_id: None,
            automatic: false,
            can_cancel_running: false,
        }
    }
    pub fn interactive(mut self) -> Self {
        self.lane = Lane::Interactive;
        self.can_cancel_running = true;
        self
    }
    pub fn background(mut self) -> Self {
        self.lane = Lane::Background;
        self.can_cancel_running = true;
        self.automatic = true;
        self
    }
    pub fn exclusive(mut self) -> Self {
        self.lane = Lane::Exclusive;
        self
    }
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QueueView {
    pub paused: bool,
    pub items: Vec<Operation>,
}
struct Entry {
    view: Operation,
    cancel: Arc<AtomicBool>,
    preempted: Arc<AtomicBool>,
}
#[derive(Default)]
struct Data {
    paused: bool,
    entries: VecDeque<Entry>,
}
struct Inner {
    data: Mutex<Data>,
    changed: Notify,
    on_change: Box<dyn Fn() + Send + Sync>,
}
#[derive(Clone)]
pub struct OperationQueue(Arc<Inner>);

impl OperationQueue {
    pub fn new(on_change: impl Fn() + Send + Sync + 'static) -> Self {
        Self(Arc::new(Inner {
            data: Mutex::new(Data::default()),
            changed: Notify::new(),
            on_change: Box::new(on_change),
        }))
    }
    fn changed(&self) {
        self.0.changed.notify_waiters();
        (self.0.on_change)();
    }
    pub fn snapshot(&self) -> QueueView {
        let data = self.0.data.lock().unwrap();
        QueueView {
            paused: data.paused,
            items: data.entries.iter().map(|e| e.view.clone()).collect(),
        }
    }
    pub fn pause(&self, paused: bool) {
        self.0.data.lock().unwrap().paused = paused;
        self.changed();
    }
    pub fn clear_finished(&self) {
        self.0
            .data
            .lock()
            .unwrap()
            .entries
            .retain(|e| e.view.status.pending());
        self.changed();
    }
    pub fn cancel(&self, id: &str) -> Result<()> {
        {
            let mut data = self.0.data.lock().unwrap();
            let entry = data
                .entries
                .iter_mut()
                .find(|e| e.view.id == id)
                .ok_or("Operação não encontrada.")?;
            match entry.view.status {
                Status::Queued => {
                    entry.view.status = Status::Cancelled;
                    entry.view.finished_at = Some(now());
                }
                Status::Running if entry.view.can_cancel_running => {
                    entry.view.status = Status::Cancelling
                }
                Status::Running => {
                    return Err(
                        "Esta operação já começou e precisa terminar para preservar os dados."
                            .into(),
                    )
                }
                _ => return Ok(()),
            }
            entry.cancel.store(true, Ordering::Relaxed);
        }
        self.changed();
        Ok(())
    }
    pub fn promote(&self, detail: &str) {
        let mut changed = false;
        {
            let mut data = self.0.data.lock().unwrap();
            for entry in &mut data.entries {
                if entry.view.status == Status::Queued
                    && entry.view.automatic
                    && entry.view.detail == detail
                    && entry.view.lane == Lane::Interactive
                {
                    entry.view.automatic = false;
                    changed = true;
                }
            }
        }
        if changed {
            self.changed();
        }
    }
    pub fn cancel_reads(&self) {
        for item in self
            .snapshot()
            .items
            .into_iter()
            .filter(|i| i.lane.interruptible() && i.status.pending())
        {
            let _ = self.cancel(&item.id);
        }
    }
    pub fn cancel_syncs(&self, include_queued: bool) {
        let ids: Vec<_> = self
            .snapshot()
            .items
            .into_iter()
            .filter(|e| {
                e.kind == "sync"
                    && (e.status.active() || (include_queued && e.status == Status::Queued))
            })
            .map(|e| e.id)
            .collect();
        for id in ids {
            let _ = self.cancel(&id);
        }
    }
    pub fn cancel_waiting_account_operations(&self) {
        // Logout invalidates requests prepared under the previous account.
        // Authentication and local CLI maintenance can still run afterwards.
        for item in self.snapshot().items.into_iter().filter(|item| {
            item.status == Status::Queued
                && !matches!(item.kind.as_str(), "login" | "connection" | "update")
        }) {
            let _ = self.cancel(&item.id);
        }
    }
    pub async fn execute<T, F, Fut>(&self, spec: Spec, work: F) -> Result<T>
    where
        F: FnOnce(Context) -> Fut,
        Fut: Future<Output = Result<T>>,
    {
        let id = uuid::Uuid::new_v4().to_string();
        let cancel = Arc::new(AtomicBool::new(false));
        let preempted = Arc::new(AtomicBool::new(false));
        {
            let mut data = self.0.data.lock().unwrap();
            if data
                .entries
                .iter()
                .filter(|e| e.view.status.pending())
                .count()
                >= WAITING_LIMIT
            {
                return Err("A fila atingiu 256 operações. Cancele itens pendentes ou espere a fila avançar.".into());
            }
            if spec.lane == Lane::Exclusive {
                // Read-only futures can be dropped safely; do not wait for an entire
                // account index before logout, login or an executable replacement.
                for entry in &mut data.entries {
                    if entry.view.status.active() && entry.view.lane.interruptible() {
                        entry.preempted.store(true, Ordering::Relaxed);
                        entry.cancel.store(true, Ordering::Relaxed);
                        entry.view.status = Status::Cancelling;
                    }
                }
            }
            data.entries.push_back(Entry {
                cancel: cancel.clone(),
                preempted: preempted.clone(),
                view: Operation {
                    id: id.clone(),
                    kind: spec.kind,
                    lane: spec.lane,
                    title: spec.title,
                    detail: spec.detail,
                    pair_id: spec.pair_id,
                    automatic: spec.automatic,
                    can_cancel_running: spec.can_cancel_running,
                    status: Status::Queued,
                    created_at: now(),
                    started_at: None,
                    finished_at: None,
                    error: None,
                },
            });
        }
        let mut lease = Lease {
            queue: self.clone(),
            id: id.clone(),
            finished: false,
        };
        self.changed();
        loop {
            // Subscribe before checking admission/cancellation to avoid lost wakeups.
            let notified = self.0.changed.notified();
            tokio::pin!(notified);
            notified.as_mut().enable();
            if cancel.load(Ordering::Relaxed) {
                return Err(CANCELLED.into());
            }
            let admitted = {
                let mut data = self.0.data.lock().unwrap();
                let barrier = data
                    .entries
                    .iter()
                    .find(|e| e.view.lane == Lane::Exclusive && e.view.status.pending())
                    .map(|e| e.view.id.clone());
                let active: Vec<_> = data
                    .entries
                    .iter()
                    .filter(|e| e.view.status.active())
                    .map(|e| e.view.lane)
                    .collect();
                let next = if data.paused {
                    None
                } else if spec.lane == Lane::Exclusive {
                    if active.is_empty() {
                        barrier
                    } else {
                        None
                    }
                } else if barrier.is_some()
                    || active.contains(&Lane::Exclusive)
                    || active.iter().filter(|lane| **lane == spec.lane).count()
                        >= spec.lane.capacity()
                {
                    None
                } else {
                    data.entries
                        .iter()
                        .filter(|e| e.view.status == Status::Queued && e.view.lane == spec.lane)
                        .min_by_key(|e| e.view.automatic)
                        .map(|e| e.view.id.clone())
                };
                if next.as_deref() == Some(&id) {
                    let entry = data.entries.iter_mut().find(|e| e.view.id == id).unwrap();
                    entry.view.status = Status::Running;
                    entry.view.started_at = Some(now());
                    true
                } else {
                    false
                }
            };
            if admitted {
                break;
            }
            notified.await;
        }
        self.changed();
        let context = Context {
            queue: self.clone(),
            id,
            cancel: cancel.clone(),
        };
        let cancellation = context.clone();
        let future = work(context);
        let result = if spec.lane.interruptible() {
            tokio::select! {
                biased;
                _ = cancellation.cancelled() => Err(CANCELLED.into()),
                result = future => result,
            }
        } else {
            future.await
        };
        let result = if preempted.load(Ordering::Relaxed) {
            Err(PREEMPTED.into())
        } else if cancel.load(Ordering::Relaxed) {
            Err(CANCELLED.into())
        } else {
            result
        };
        lease.finish(&result);
        result
    }
}

#[derive(Clone)]
pub struct Context {
    queue: OperationQueue,
    id: String,
    pub cancel: Arc<AtomicBool>,
}
impl Context {
    pub async fn cancelled(&self) {
        loop {
            let notified = self.queue.0.changed.notified();
            tokio::pin!(notified);
            notified.as_mut().enable();
            if self.cancel.load(Ordering::Relaxed) {
                return;
            }
            notified.await;
        }
    }
    pub fn progress(&self, pair_id: Option<String>, detail: impl Into<String>) {
        {
            let mut data = self.queue.0.data.lock().unwrap();
            if let Some(entry) = data.entries.iter_mut().find(|e| e.view.id == self.id) {
                entry.view.detail = detail.into();
                if pair_id.is_some() {
                    entry.view.pair_id = pair_id;
                }
            }
        }
        self.queue.changed();
    }
}
struct Lease {
    queue: OperationQueue,
    id: String,
    finished: bool,
}
impl Lease {
    fn finish<T>(&mut self, result: &Result<T>) {
        {
            let mut data = self.queue.0.data.lock().unwrap();
            if let Some(entry) = data.entries.iter_mut().find(|e| e.view.id == self.id) {
                entry.view.status = match result {
                    Ok(_) => Status::Completed,
                    Err(e) if e == CANCELLED || e == PREEMPTED => Status::Cancelled,
                    Err(_) => Status::Failed,
                };
                entry.view.error = result
                    .as_ref()
                    .err()
                    .filter(|e| e.as_str() != CANCELLED)
                    .cloned();
                entry.view.finished_at = Some(now());
            }
            let mut excess = data
                .entries
                .iter()
                .filter(|e| !e.view.status.pending())
                .count()
                .saturating_sub(HISTORY_LIMIT);
            data.entries.retain(|e| {
                if excess > 0 && !e.view.status.pending() {
                    excess -= 1;
                    false
                } else {
                    true
                }
            });
        }
        self.finished = true;
        self.queue.changed();
    }
}
impl Drop for Lease {
    fn drop(&mut self) {
        if !self.finished {
            self.finish::<()>(&Err(CANCELLED.into()));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    use tokio::sync::oneshot;

    async fn until(queue: &OperationQueue, count: usize) {
        tokio::time::timeout(Duration::from_secs(2), async {
            while queue.snapshot().items.len() < count {
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
    }
    #[tokio::test]
    async fn independent_lanes_bound_reads_and_cancel_a_stuck_index() {
        let queue = OperationQueue::new(|| {});
        let mut tasks = vec![];
        for lane in [
            Lane::Background,
            Lane::Interactive,
            Lane::Interactive,
            Lane::Interactive,
            Lane::Transfer,
        ] {
            let q = queue.clone();
            tasks.push(tokio::spawn(async move {
                let mut spec = Spec::new("test", "job", "");
                spec.lane = lane;
                spec.can_cancel_running = true;
                q.execute(spec, |_| std::future::pending::<Result<()>>())
                    .await
            }));
            until(&queue, tasks.len()).await;
        }
        let snapshot = queue.snapshot();
        assert_eq!(
            snapshot
                .items
                .iter()
                .filter(|i| i.status == Status::Running)
                .count(),
            4
        );
        assert_eq!(snapshot.items[3].status, Status::Queued);
        assert_eq!(snapshot.items[4].status, Status::Running);
        queue.cancel(&snapshot.items[0].id).unwrap();
        let index = tasks.remove(0);
        assert_eq!(
            tokio::time::timeout(Duration::from_secs(1), index)
                .await
                .unwrap()
                .unwrap(),
            Err(CANCELLED.into())
        );
        for task in tasks {
            task.abort();
        }
    }

    #[tokio::test]
    async fn foreground_actions_precede_automatic_jobs_and_exclusive_work_preempts_reads() {
        let queue = OperationQueue::new(|| {});
        queue.pause(true);
        let calls = Arc::new(Mutex::new(vec![]));
        let mut tasks = vec![];
        for (name, automatic) in [("automatic", true), ("user", false)] {
            let q = queue.clone();
            let calls = calls.clone();
            tasks.push(tokio::spawn(async move {
                let mut spec = Spec::new("sync", name, "");
                spec.automatic = automatic;
                q.execute(spec, |_| async move {
                    calls.lock().unwrap().push(name);
                    Ok(())
                })
                .await
            }));
            until(&queue, tasks.len()).await;
        }
        queue.pause(false);
        for task in tasks {
            task.await.unwrap().unwrap();
        }
        assert_eq!(*calls.lock().unwrap(), vec!["user", "automatic"]);
        let q = queue.clone();
        let index = tokio::spawn(async move {
            q.execute(Spec::new("index", "index", "").background(), |_| {
                std::future::pending::<Result<()>>()
            })
            .await
        });
        until(&queue, 3).await;
        let result = tokio::time::timeout(
            Duration::from_secs(1),
            queue.execute(Spec::new("update", "update", "").exclusive(), |_| async {
                Ok(())
            }),
        )
        .await
        .unwrap();
        assert_eq!(result, Ok(()));
        assert_eq!(index.await.unwrap(), Err(PREEMPTED.into()));
    }

    #[tokio::test]
    async fn fifo_visible_waiting_failure_and_cancellation_do_not_block_following_work() {
        let queue = OperationQueue::new(|| {});
        let (release, wait) = oneshot::channel();
        let q = queue.clone();
        let first = tokio::spawn(async move {
            q.execute(Spec::new("sync", "first", ""), |_| async {
                wait.await.unwrap();
                Ok(1)
            })
            .await
        });
        until(&queue, 1).await;
        let called = Arc::new(Mutex::new(vec![]));
        let mut handles = vec![];
        for i in 2..=4 {
            let q = queue.clone();
            let called = called.clone();
            handles.push(tokio::spawn(async move {
                q.execute(Spec::new("download", i.to_string(), ""), |_| async move {
                    called.lock().unwrap().push(i);
                    if i == 3 {
                        Err("Falha de rede".into())
                    } else {
                        Ok(i)
                    }
                })
                .await
            }));
            until(&queue, i).await;
        }
        let view = queue.snapshot();
        assert_eq!(view.items[0].status, Status::Running);
        assert!(view.items[1..].iter().all(|e| e.status == Status::Queued));
        queue.cancel(&view.items[1].id).unwrap();
        release.send(()).unwrap();
        assert_eq!(first.await.unwrap().unwrap(), 1);
        let mut results = vec![];
        for h in handles {
            results.push(h.await.unwrap());
        }
        assert_eq!(
            results,
            vec![Err(CANCELLED.into()), Err("Falha de rede".into()), Ok(4)]
        );
        assert_eq!(*called.lock().unwrap(), vec![3, 4]);
        assert_eq!(
            queue
                .snapshot()
                .items
                .iter()
                .map(|e| e.status)
                .collect::<Vec<_>>(),
            vec![
                Status::Completed,
                Status::Cancelled,
                Status::Failed,
                Status::Completed
            ]
        );
    }
    #[tokio::test]
    async fn pause_cancel_while_paused_and_resume_are_immediate() {
        let queue = OperationQueue::new(|| {});
        queue.pause(true);
        let q = queue.clone();
        let cancelled = tokio::spawn(async move {
            q.execute(Spec::new("download", "cancel", ""), |_| async {
                panic!("cancelled work must not run");
                #[allow(unreachable_code)]
                Ok(())
            })
            .await
        });
        until(&queue, 1).await;
        queue.cancel(&queue.snapshot().items[0].id).unwrap();
        assert_eq!(
            tokio::time::timeout(Duration::from_secs(1), cancelled)
                .await
                .unwrap()
                .unwrap(),
            Err(CANCELLED.into())
        );
        let q = queue.clone();
        let resumed = tokio::spawn(async move {
            q.execute(Spec::new("browse", "next", ""), |_| async { Ok(7) })
                .await
        });
        until(&queue, 2).await;
        assert_eq!(queue.snapshot().items[1].status, Status::Queued);
        queue.pause(false);
        assert_eq!(resumed.await.unwrap().unwrap(), 7);
    }
    #[tokio::test]
    async fn running_sync_cancels_cooperatively_and_next_job_waits_for_safe_completion() {
        let queue = OperationQueue::new(|| {});
        let (release, wait) = oneshot::channel();
        let (context_tx, context_rx) = oneshot::channel();
        let q = queue.clone();
        let first = tokio::spawn(async move {
            q.execute(
                Spec {
                    can_cancel_running: true,
                    ..Spec::new("sync", "Sync", "")
                },
                |ctx| async move {
                    ctx.progress(Some("pair".into()), "Enviando arquivo.txt");
                    context_tx.send(ctx.cancel).unwrap();
                    wait.await.unwrap();
                    Ok(())
                },
            )
            .await
        });
        let flag = context_rx.await.unwrap();
        let first_id = queue.snapshot().items[0].id.clone();
        queue.cancel(&first_id).unwrap();
        assert!(flag.load(Ordering::Relaxed));
        assert_eq!(queue.snapshot().items[0].status, Status::Cancelling);
        assert_eq!(queue.snapshot().items[0].detail, "Enviando arquivo.txt");
        let q = queue.clone();
        let second = tokio::spawn(async move {
            q.execute(Spec::new("download", "next", ""), |_| async { Ok(()) })
                .await
        });
        until(&queue, 2).await;
        assert_eq!(queue.snapshot().items[1].status, Status::Queued);
        release.send(()).unwrap();
        assert_eq!(first.await.unwrap(), Err(CANCELLED.into()));
        second.await.unwrap().unwrap();
    }
    #[tokio::test]
    async fn dropped_waiter_and_dropped_running_job_release_the_fifo() {
        let queue = OperationQueue::new(|| {});
        let q = queue.clone();
        let first = tokio::spawn(async move {
            q.execute(Spec::new("browse", "active", ""), |_| async {
                std::future::pending::<Result<()>>().await
            })
            .await
        });
        until(&queue, 1).await;
        let q = queue.clone();
        let waiting = tokio::spawn(async move {
            q.execute(Spec::new("browse", "waiting", ""), |_| async { Ok(()) })
                .await
        });
        until(&queue, 2).await;
        waiting.abort();
        let _ = waiting.await;
        first.abort();
        let _ = first.await;
        assert!(queue
            .snapshot()
            .items
            .iter()
            .all(|e| e.status == Status::Cancelled));
        queue
            .execute(Spec::new("browse", "next", ""), |_| async { Ok(()) })
            .await
            .unwrap();
    }
    #[tokio::test]
    async fn non_interruptible_operations_finish_normally_and_history_is_bounded() {
        let queue = OperationQueue::new(|| {});
        let q = queue.clone();
        queue
            .execute(Spec::new("update", "Update", ""), |_| async move {
                assert!(q.cancel(&q.snapshot().items[0].id).is_err());
                Ok(())
            })
            .await
            .unwrap();
        for _ in 0..40 {
            queue
                .execute(Spec::new("browse", "Read", ""), |_| async { Ok(()) })
                .await
                .unwrap();
        }
        assert_eq!(queue.snapshot().items.len(), HISTORY_LIMIT);
        queue.clear_finished();
        assert!(queue.snapshot().items.is_empty());
    }
}

#[cfg(test)]
mod account_tests {
    use super::*;
    #[tokio::test]
    async fn logout_cancels_pending_account_actions_without_cancelling_the_next_login() {
        let queue = OperationQueue::new(|| {});
        queue.pause(true);
        let mut handles = Vec::new();
        for kind in ["download", "sync", "computer", "login"] {
            let q = queue.clone();
            handles.push(tokio::spawn(async move {
                q.execute(Spec::new(kind, kind, ""), |_| async { Ok(()) })
                    .await
            }));
        }
        tokio::time::timeout(std::time::Duration::from_secs(2), async {
            while queue.snapshot().items.len() < 4 {
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
        queue.cancel_waiting_account_operations();
        let view = queue.snapshot();
        assert!(view
            .items
            .iter()
            .filter(|i| i.kind != "login")
            .all(|i| i.status == Status::Cancelled));
        assert_eq!(
            view.items
                .iter()
                .find(|i| i.kind == "login")
                .unwrap()
                .status,
            Status::Queued
        );
        queue.pause(false);
        for task in handles {
            let _ = task.await.unwrap();
        }
    }
}
