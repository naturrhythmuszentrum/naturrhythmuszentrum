// SPDX-License-Identifier: AGPL-3.0-only
//! Scripted transport tests exercise generation and partial failure without timing assertions.

use super::*;

/// Scripted worker state and retained receiving ends.
type Scripted = (Workers, Vec<Receiver<Command>>);

/// Bounded scripted endpoints contain no live operating-system worker.
fn scripted(completions: &[Completion]) -> Result<Scripted, Box<dyn std::error::Error>> {
    let mut workers = Workers {
        entries: Vec::new(),
        generation: 0,
        cancellation: CancellationToken::default(),
        poll: Duration::from_nanos(1),
    };
    let mut commands = Vec::new();
    for completion in completions {
        let (send_command, receive_command) = mpsc::sync_channel(1);
        let (send_completion, receive_completion) = mpsc::sync_channel(1);
        send_completion.send(*completion)?;
        workers.entries.push(Worker {
            commands: Some(send_command),
            completions: receive_completion,
            handle: None,
            layout: std::cell::Cell::new(None),
        });
        commands.push(receive_command);
    }
    Ok((workers, commands))
}

#[test]
fn matching_generation_is_required_before_accepting_completion()
-> Result<(), Box<dyn std::error::Error>> {
    for completion in [
        Completion {
            worker: 1,
            generation: 0,
            result: Ok(()),
            layout: None,
        },
        Completion {
            worker: 0,
            generation: 1,
            result: Ok(()),
            layout: None,
        },
    ] {
        let (workers, _commands) = scripted(&[completion])?;
        assert_eq!(workers.receive(0, 0), Err(RecipeError::Worker));
    }
    Ok(())
}

#[test]
fn partial_failure_preserves_first_error_and_collects_other_workers()
-> Result<(), Box<dyn std::error::Error>> {
    let (mut workers, commands) = scripted(&[
        Completion {
            worker: 0,
            generation: 1,
            result: Err(RecipeError::Domain),
            layout: None,
        },
        Completion {
            worker: 1,
            generation: 1,
            result: Ok(()),
            layout: None,
        },
    ])?;
    assert_eq!(workers.dispatch(Some(1), false), Err(RecipeError::Domain));
    for command in &commands {
        assert!(matches!(command.try_recv()?, Command::Execute(1, Some(1))));
    }
    for worker in &workers.entries {
        assert!(worker.completions.try_recv().is_err());
    }
    Ok(())
}

#[test]
fn cancellation_prevents_a_new_dispatch_and_drop_closes_admission()
-> Result<(), Box<dyn std::error::Error>> {
    let (mut workers, commands) = scripted(&[Completion {
        worker: 0,
        generation: 1,
        result: Ok(()),
        layout: None,
    }])?;
    workers.cancellation.cancel();
    assert_eq!(
        workers.dispatch(Some(1), false),
        Err(RecipeError::Cancelled)
    );
    assert!(commands[0].try_recv().is_err());
    drop(workers);
    assert!(matches!(commands[0].recv(), Err(mpsc::RecvError)));
    Ok(())
}

#[test]
fn disconnected_transport_is_a_worker_failure() -> Result<(), Box<dyn std::error::Error>> {
    let (workers, _commands) = scripted(&[Completion {
        worker: 0,
        generation: 0,
        result: Ok(()),
        layout: None,
    }])?;
    workers.receive(0, 0)?;
    assert_eq!(workers.receive(0, 1), Err(RecipeError::Worker));
    Ok(())
}

/// Dropping the coordinator closes admission and joins every created worker.
#[test]
fn finalization_joins_all_created_workers() {
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };
    let finished = Arc::new(AtomicUsize::new(0));
    let mut workers = Workers {
        entries: Vec::new(),
        generation: 0,
        cancellation: CancellationToken::default(),
        poll: Duration::from_nanos(1),
    };
    for _ in 0..3 {
        let (send_command, receive_command) = mpsc::sync_channel::<Command>(1);
        let (send_completion, receive_completion) = mpsc::sync_channel(1);
        let count = Arc::clone(&finished);
        let handle = thread::spawn(move || {
            assert!(matches!(receive_command.recv(), Err(mpsc::RecvError)));
            drop(send_completion);
            count.fetch_add(1, Ordering::Release);
        });
        workers.entries.push(Worker {
            commands: Some(send_command),
            completions: receive_completion,
            handle: Some(handle),
            layout: std::cell::Cell::new(None),
        });
    }
    drop(workers);
    assert_eq!(finished.load(Ordering::Acquire), 3);
}
