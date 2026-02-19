use thiserror::Error;

#[derive(Error, Debug)]
pub enum FaidxError {
    #[error("Illegal input parameters ({0}, {1})")]
    IllegalInput(usize, usize),
    #[error("Unknown sequence")]
    UnknownSequence,
    #[error("Error loading sequence")]
    ErrorLoadingSequence,
    #[error("Error loading FASTA/FASTQ index")]
    ErrorLoadingFaidx,
    #[error("Error building FASTA/FASTQ index")]
    ErrorBuildingFaidx,
}