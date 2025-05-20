use super::{
    cigar::{CigarElem, CigarOp},
    cigar_error::CigarError,
};

enum ValidationState {
    Start,
    AfterInitialHardClip,
    AfterInitialSoftClip,
    AfterHardClip,
    AfterSoftClip,
    Interior,
}

pub(super) fn valid_elem_slice(v: &[CigarElem]) -> Result<(), CigarError> {
    let mut state = ValidationState::Start;
    for op in v.iter().map(|o| o.op()) {
        state = match state {
            ValidationState::Start => handle_start(op)?,
            ValidationState::AfterInitialHardClip => handle_after_init_hard_clip(op)?,
            ValidationState::AfterInitialSoftClip => handle_after_init_soft_clip(op)?,
            ValidationState::AfterSoftClip => handle_after_soft_clip(op)?,
            ValidationState::AfterHardClip => handle_after_hard_clip(op)?,
            ValidationState::Interior => handle_interior(op)?,
        }
    }
    Ok(())
}

fn handle_start(op: CigarOp) -> Result<ValidationState, CigarError> {
    match op {
        CigarOp::SoftClip => Ok(ValidationState::AfterInitialSoftClip),
        CigarOp::HardClip => Ok(ValidationState::AfterInitialHardClip),
        x if !x.is_valid() => Err(CigarError::UnknownOperator),
        _ => Ok(ValidationState::Interior),
    }
}

fn handle_after_init_hard_clip(op: CigarOp) -> Result<ValidationState, CigarError> {
    match op {
        CigarOp::SoftClip => Ok(ValidationState::AfterInitialSoftClip),
        CigarOp::HardClip => Err(CigarError::MultipleAdjacentHardClips),
        x if !x.is_valid() => Err(CigarError::UnknownOperator),
        _ => Ok(ValidationState::Interior),
    }
}

fn handle_after_init_soft_clip(op: CigarOp) -> Result<ValidationState, CigarError> {
    match op {
        CigarOp::SoftClip => Err(CigarError::MultipleAdjacentSoftClips),
        CigarOp::HardClip => Ok(ValidationState::AfterHardClip),
        x if !x.is_valid() => Err(CigarError::UnknownOperator),
        _ => Ok(ValidationState::Interior),
    }
}

fn handle_after_soft_clip(op: CigarOp) -> Result<ValidationState, CigarError> {
    match op {
        CigarOp::SoftClip => Err(CigarError::MultipleAdjacentSoftClips),
        CigarOp::HardClip => Ok(ValidationState::AfterHardClip),
        x if !x.is_valid() => Err(CigarError::UnknownOperator),
        _ => Err(CigarError::InteriorSoftClip),
    }
}

fn handle_after_hard_clip(op: CigarOp) -> Result<ValidationState, CigarError> {
    match op {
        CigarOp::HardClip => Err(CigarError::MultipleAdjacentHardClips),
        x if !x.is_valid() => Err(CigarError::UnknownOperator),
        _ => Err(CigarError::InteriorHardClip),
    }
}

fn handle_interior(op: CigarOp) -> Result<ValidationState, CigarError> {
    match op {
        CigarOp::SoftClip => Ok(ValidationState::AfterSoftClip),
        CigarOp::HardClip => Ok(ValidationState::AfterHardClip),
        x if !x.is_valid() => Err(CigarError::UnknownOperator),
        _ => Ok(ValidationState::Interior),
    }
}
