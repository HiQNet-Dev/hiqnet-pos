use super::RestoreStep;
use std::sync::atomic::{AtomicU32, Ordering};

static INJECTED: AtomicU32 = AtomicU32::new(0);

fn mask(step: RestoreStep) -> u32 {
    match step {
        RestoreStep::RecoveryCopy => 1 << 0,
        RestoreStep::StagedCopy => 1 << 1,
        RestoreStep::StagedValidation => 1 << 2,
        RestoreStep::Promotion => 1 << 3,
        RestoreStep::FinalVerification => 1 << 4,
        RestoreStep::Rollback => 1 << 5,
    }
}

/// Permite encadenar fallos (por ejemplo, verificación final y reversión).
pub(super) fn inject(steps: &[RestoreStep]) {
    let mut bits = 0;
    for step in steps {
        bits |= mask(*step);
    }
    INJECTED.store(bits, Ordering::SeqCst);
}

pub(super) fn take(step: RestoreStep) -> bool {
    let bit = mask(step);
    let mut current = INJECTED.load(Ordering::SeqCst);
    loop {
        if current & bit == 0 {
            return false;
        }
        match INJECTED.compare_exchange(current, current & !bit, Ordering::SeqCst, Ordering::SeqCst)
        {
            Ok(_) => return true,
            Err(observed) => current = observed,
        }
    }
}
