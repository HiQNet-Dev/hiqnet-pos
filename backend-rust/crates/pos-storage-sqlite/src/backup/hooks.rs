//! Puntos de fallo del procedimiento de restauracion.

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum RestoreStep {
    RecoveryCopy,
    StagedCopy,
    StagedValidation,
    Promotion,
    FinalVerification,
    Rollback,
}

/// En compilaciones normales los puntos de fallo no existen.
#[inline]
pub(super) fn fault(_step: RestoreStep) -> bool {
    #[cfg(test)]
    {
        super::fault_injector::take(_step)
    }
    #[cfg(not(test))]
    {
        false
    }
}
