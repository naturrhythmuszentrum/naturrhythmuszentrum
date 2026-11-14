// SPDX-License-Identifier: AGPL-3.0-only
//! Generation-tagged blocking-channel workers, joined on every exit path.

use crate::{CancellationToken, CaseRequest, Seed, kernels::Kernel, recipe::RecipeError};
use std::{
    hint::black_box,
    sync::mpsc::{self, Receiver, RecvTimeoutError, SyncSender},
    thread::{self, JoinHandle},
    time::Duration,
};

/// One bounded command per worker; no global state or detached work.
#[derive(Clone, Copy, Debug)]
enum Command {
    /// Restore private state and optionally dry-validate a new length.
    Reset(u64, u64),
    /// One aggregate-item generation, including the empty control protocol.
    Execute(u64, Option<u64>),
}

/// Fixed-size worker response includes both worker identity and generation.
#[derive(Clone, Copy, Debug)]
struct Completion {
    /// Assigned worker index.
    worker: usize,
    /// Parent-issued generation; zero denotes preparation.
    generation: u64,
    /// Ordinary operation/preparation outcome.
    result: Result<(), RecipeError>,
    /// Startup-only storage evidence.
    layout: Option<crate::layout::WorkerLayout>,
}

/// One live worker's bounded transport and join obligation.
#[derive(Debug)]
struct Worker {
    /// At most one pending control command.
    commands: Option<SyncSender<Command>>,
    /// At most one outstanding completion.
    completions: Receiver<Completion>,
    /// Owned join obligation, never detached.
    handle: Option<JoinHandle<()>>,
    /// Startup evidence retained without another allocation.
    layout: std::cell::Cell<Option<crate::layout::WorkerLayout>>,
}

/// Parallel active workers with a separately timed coordinator.
#[derive(Debug)]
#[expect(
    clippy::redundant_pub_crate,
    reason = "Shared by crate siblings; private module prevents public reachability."
)]
pub(super) struct Workers {
    /// Every created worker, including any whose startup failed.
    entries: Vec<Worker>,
    /// Monotonically increasing command identity.
    generation: u64,
    /// Explicit cancellation token.
    cancellation: CancellationToken,
    /// Bounded blocking receive interval between cancellation checkpoints.
    poll: Duration,
}

/// Bounded transport payload: retained handles plus one command and completion slot per worker.
#[expect(
    clippy::redundant_pub_crate,
    reason = "Shared by crate siblings; private module prevents public reachability."
)]
pub(super) fn transport_bytes(count: u64) -> Result<u64, RecipeError> {
    let per_worker = size_of::<Worker>()
        .checked_add(size_of::<Command>())
        .and_then(|n| n.checked_add(size_of::<Completion>()))
        .ok_or(RecipeError::Overflow)?;
    count
        .checked_mul(u64::try_from(per_worker).map_err(|_| RecipeError::Overflow)?)
        .ok_or(RecipeError::Overflow)
}

impl Drop for Workers {
    fn drop(&mut self) {
        for worker in &mut self.entries {
            // Closing command admission cannot block behind a pending generation.
            drop(worker.commands.take());
        }
        for worker in &mut self.entries {
            if let Some(handle) = worker.handle.take() {
                let _joined = handle.join();
            }
        }
    }
}

impl Workers {
    /// Each worker allocates and first-touches its own disjoint equally sized buffers.
    pub(super) fn prepare(
        case: &CaseRequest,
        seed: Seed,
        byte_cap: u64,
        stack: usize,
        cancellation: CancellationToken,
        poll: Duration,
        transport_cap: u64,
    ) -> Result<Self, RecipeError> {
        let count = usize::try_from(case.workers).map_err(|_| RecipeError::Overflow)?;
        let mut out = Self {
            entries: Vec::new(),
            generation: 0,
            cancellation,
            poll,
        };
        let mut transport = crate::memory::Budget::new(transport_cap);
        let slots = count
            .checked_mul(size_of::<Command>() + size_of::<Completion>())
            .ok_or(RecipeError::Overflow)?;
        transport.charge(slots)?;
        out.entries = transport.reserve(count)?;
        for index in 0..count {
            if out.cancellation.is_cancelled() {
                return Err(RecipeError::Cancelled);
            }
            let (commands, receive) = mpsc::sync_channel(1);
            let (completion_sender, completions) = mpsc::sync_channel(1);
            let mut budget = crate::memory::Budget::new(byte_cap);
            budget.charge(size_of::<CaseRequest>())?;
            let case = crate::memory::clone_case(case, &mut budget)?;
            let byte_cap = budget.remaining();
            let handle = thread::Builder::new()
                .stack_size(stack)
                .spawn(move || worker(index, &case, seed, byte_cap, &receive, &completion_sender))
                .map_err(|_| RecipeError::Allocation)?;
            out.entries.push(Worker {
                commands: Some(commands),
                completions,
                handle: Some(handle),
                layout: std::cell::Cell::new(None),
            });
            out.receive(index, 0)?;
        }
        Ok(out)
    }

    /// Collect exactly one matching response; cancellation never fabricates completion.
    fn receive(&self, index: usize, generation: u64) -> Result<(), RecipeError> {
        let worker = self.entries.get(index).ok_or(RecipeError::Worker)?;
        loop {
            match worker.completions.recv_timeout(self.poll) {
                Ok(completion) => {
                    if completion.worker != index || completion.generation != generation {
                        return Err(RecipeError::Worker);
                    }
                    if generation == 0 {
                        worker.layout.set(completion.layout);
                    }
                    return completion.result;
                }
                Err(RecvTimeoutError::Timeout) => {
                    if self.cancellation.is_cancelled() {
                        return Err(RecipeError::Cancelled);
                    }
                }
                Err(RecvTimeoutError::Disconnected) => return Err(RecipeError::Worker),
            }
        }
    }

    /// Dispatch one generation to every active worker, then consume every completion.
    fn dispatch(&mut self, length: Option<u64>, reset: bool) -> Result<(), RecipeError> {
        if self.cancellation.is_cancelled() {
            return Err(RecipeError::Cancelled);
        }
        self.generation = self
            .generation
            .checked_add(1)
            .ok_or(RecipeError::Overflow)?;
        let command = if reset {
            Command::Reset(self.generation, length.unwrap_or(0))
        } else {
            Command::Execute(self.generation, length)
        };
        for worker in &self.entries {
            worker
                .commands
                .as_ref()
                .ok_or(RecipeError::Worker)?
                .send(command)
                .map_err(|_| RecipeError::Worker)?;
        }
        let mut first_error = None;
        for index in 0..self.entries.len() {
            if let Err(error) = self.receive(index, self.generation) {
                first_error.get_or_insert(error);
            }
        }
        first_error.map_or(Ok(()), Err)
    }

    /// Reset/dry-validation acknowledgement precedes the host timer.
    pub(super) fn reset(&mut self, length: u64) -> Result<(), RecipeError> {
        self.dispatch(Some(length), true)
    }

    /// Timed dispatch includes channel wait, skew, computation and completion collection.
    pub(super) fn execute(&mut self, length: Option<u64>) -> Result<(), RecipeError> {
        self.dispatch(length, false)
    }
}

/// Worker owns all preparation, reset and boundary consumption for its fixed role.
fn worker(
    index: usize,
    case: &CaseRequest,
    seed: Seed,
    byte_cap: u64,
    receive: &Receiver<Command>,
    completion_sender: &SyncSender<Completion>,
) {
    let prepared = Kernel::prepare(case, seed, byte_cap);
    let layout = prepared
        .as_ref()
        .ok()
        .and_then(|kernel| kernel.layout(u64::try_from(index).ok()?).ok());
    let result = prepared.as_ref().map(|_| ()).map_err(|error| *error);
    if completion_sender
        .send(Completion {
            worker: index,
            generation: 0,
            result,
            layout,
        })
        .is_err()
    {
        return;
    }
    let Ok(mut kernel) = prepared else {
        return;
    };
    let mut validated = None;
    while let Ok(command) = receive.recv() {
        let (generation, result) = match command {
            Command::Reset(generation, length) => {
                let result = if length > 0 && validated != Some(length) {
                    kernel.validate(length).inspect(|()| {
                        validated = Some(length);
                    })
                } else {
                    Ok(())
                };
                kernel.reset();
                (generation, result)
            }
            Command::Execute(generation, length) => {
                let result = length.map_or_else(
                    || {
                        black_box(());
                        Ok(())
                    },
                    |length| kernel.execute(length),
                );
                (generation, result)
            }
        };
        if completion_sender
            .send(Completion {
                worker: index,
                generation,
                result,
                layout: None,
            })
            .is_err()
        {
            break;
        }
    }
}

/// One direct-call kernel or explicitly requested active workers.
#[derive(Debug)]
#[expect(
    clippy::redundant_pub_crate,
    reason = "Shared by crate siblings; private module prevents public reachability."
)]
pub(super) enum Runner {
    /// Serial source path has no channel protocol.
    Direct {
        /// Prepared private kernel storage.
        kernel: Kernel,
        /// Last fully dry-checked item length.
        validated: Option<u64>,
    },
    /// Parallel source path uses blocking channels.
    Parallel(Workers),
}

impl Runner {
    /// Prepare within the supplied buffer, stack and worker bounds.
    pub(super) fn prepare(
        case: &CaseRequest,
        seed: Seed,
        request: &crate::RunRequest,
        cancellation: CancellationToken,
        bounds: crate::ExecutionBounds,
    ) -> Result<Self, RecipeError> {
        if case.workers == 1 {
            return Ok(Self::Direct {
                kernel: Kernel::prepare(case, seed, request.resources.buffer_bytes)?,
                validated: None,
            });
        }
        let bytes = request
            .resources
            .buffer_bytes
            .checked_div(case.workers)
            .ok_or(RecipeError::Domain)?;
        let stack = usize::try_from(request.resources.worker_stack_bytes)
            .map_err(|_| RecipeError::Overflow)?;
        Workers::prepare(
            case,
            seed,
            bytes,
            stack,
            cancellation,
            Duration::from_nanos(bounds.wait_poll_ns),
            transport_bytes(case.workers)?,
        )
        .map(Self::Parallel)
    }

    /// Fallibly reserve retained per-worker metadata before pilot or measurement.
    pub(super) fn layouts(&self) -> Result<Vec<crate::layout::WorkerLayout>, RecipeError> {
        let count = match self {
            Self::Direct { .. } => 1,
            Self::Parallel(workers) => workers.entries.len(),
        };
        let bytes = count
            .checked_mul(size_of::<crate::layout::WorkerLayout>())
            .ok_or(RecipeError::Overflow)?;
        let mut budget =
            crate::memory::Budget::new(u64::try_from(bytes).map_err(|_| RecipeError::Overflow)?);
        let mut out = budget.reserve(count)?;
        match self {
            Self::Direct { kernel, .. } => out.push(kernel.layout(0)?),
            Self::Parallel(workers) => {
                for worker in &workers.entries {
                    out.push(worker.layout.get().ok_or(RecipeError::Worker)?);
                }
            }
        }
        Ok(out)
    }

    /// Reset and acknowledge all active workers before timing starts.
    pub(super) fn reset(&mut self, length: u64) -> Result<(), RecipeError> {
        match self {
            Self::Direct { kernel, validated } => {
                if length > 0 && *validated != Some(length) {
                    kernel.validate(length)?;
                    *validated = Some(length);
                }
                kernel.reset();
                Ok(())
            }
            Self::Parallel(workers) => workers.reset(length),
        }
    }

    /// Execute and consume one direct or aggregate item using its declared protocol.
    pub(super) fn execute(&mut self, length: Option<u64>) -> Result<(), RecipeError> {
        match self {
            Self::Direct { kernel, .. } => length.map_or_else(
                || {
                    black_box(());
                    Ok(())
                },
                |length| kernel.execute(length),
            ),
            Self::Parallel(workers) => workers.execute(length),
        }
    }
}

#[cfg(test)]
mod tests;
