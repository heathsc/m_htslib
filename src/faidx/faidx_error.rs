use thiserror::Error;

#[derive(Error, Debug)]
pub enum FaidxError {
    #[error("Illegal input parameters")]
    IllegalInput,
    #[error("Unknown sequence")]
    UnknownSequence,
    #[error("Error loading sequence")]
    ErrorLoadingSequence,
    #[error("Error loading FASTA/FASTQ index")]
    ErrorLoadingFaidx,
}